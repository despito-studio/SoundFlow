/**
 * SoundFlow Audio Playback Engine
 * Uses native HTML5 Audio element with direct progressive streaming,
 * dedicated fresh Audio instance per track transition to prevent WASAPI endpoint detachment,
 * robust error-recovery, and smart background prefetching.
 */
class SoundFlowPlayer {
    constructor() {
        this.audio = null;

        this.queue = [];
        this.currentIndex = -1;
        this.currentTrack = null;

        this.shuffle = false;
        this.repeat = 'off'; // 'off' | 'all' | 'one'
        this.autoplay = true; // Auto-chain similar tracks by theme/genre
        this.isFetchingSimilar = false;
        this.isMuted = false;
        this.lastVolume = 0.8;
        this.isLoading = false;

        this.prefetchedUrl = null;
        this.prefetchedTrackId = null;
        this._prefetchedSimilarTrackId = null;

        this.currentRequestId = 0;
        this._lastErrorRetried = false;

        this.bassBoostEnabled = true;
        this.bassGain = 6;
        this.hqAudioEnabled = true;

        this.visualizer = new WaveformVisualizer('visualizer-canvas');

        // Initialize persistent DOM audio element
        this.audio = document.getElementById('soundflow-audio');
        if (!this.audio) {
            this.audio = document.createElement('audio');
            this.audio.id = 'soundflow-audio';
            this.audio.preload = 'auto';
            document.body.appendChild(this.audio);
        }
        this.audio.volume = this.lastVolume;

        this.setupAudioListeners();
        this.setupMediaSession();
    }

    setBassBoost(enabled, gain = 6) {
        this.bassBoostEnabled = enabled;
        this.bassGain = gain;
    }

    setupAudioListeners() {
        if (!this.audio) return;

        this.audio.addEventListener('play', () => {
            this.visualizer.start();
            this.updatePlayPauseUI(true);
        });

        this.audio.addEventListener('pause', () => {
            this.visualizer.stop();
            this.updatePlayPauseUI(false);
        });

        this.audio.addEventListener('timeupdate', () => {
            this.onTimeUpdate();
        });

        this.audio.addEventListener('progress', () => {
            this.onProgressUpdate();
        });

        this.audio.addEventListener('ended', () => {
            this.onTrackEnded();
        });

        this.audio.addEventListener('error', (e) => {
            const src = this.audio.currentSrc || this.audio.src || '';
            if (!src || src.endsWith('/about:blank') || src === window.location.href) {
                return;
            }

            console.error('Audio playback error:', e);
            this.showLoading(false);

            if (this.currentTrack && !this._lastErrorRetried && this.currentIndex >= 0) {
                this._lastErrorRetried = true;
                console.log('Stream error encountered. Retrying with fresh URL...');
                setTimeout(() => {
                    this.playTrackAtIndex(this.currentIndex, true);
                }, 400);
                return;
            }

            this._lastErrorRetried = false;
            if (window.soundFlowApp) {
                window.soundFlowApp.showToast('Помилка відтворення. Перехід до наступного треку...');
            }
            setTimeout(() => this.next(), 800);
        });
    }

    setupMediaSession() {
        if ('mediaSession' in navigator) {
            navigator.mediaSession.setActionHandler('play', () => this.play());
            navigator.mediaSession.setActionHandler('pause', () => this.pause());
            navigator.mediaSession.setActionHandler('previoustrack', () => this.prev());
            navigator.mediaSession.setActionHandler('nexttrack', () => this.next());
            navigator.mediaSession.setActionHandler('seekto', (details) => {
                if (details.seekTime !== undefined) this.seek(details.seekTime);
            });
        }
    }

    updateMediaSession(track) {
        if ('mediaSession' in navigator && track) {
            navigator.mediaSession.metadata = new MediaMetadata({
                title: track.title || 'SoundFlow Track',
                artist: track.artist || 'Unknown Artist',
                artwork: [
                    { src: track.thumbnail || '', sizes: '512x512', type: 'image/jpeg' }
                ]
            });
        }
    }

    setQueue(tracks, startIndex = 0) {
        this.queue = Array.isArray(tracks) ? [...tracks] : [];
        if (this.queue.length > 0) {
            const validIndex = Math.max(0, Math.min(startIndex, this.queue.length - 1));
            this.playTrackAtIndex(validIndex);
        }
        if (window.soundFlowApp) {
            window.soundFlowApp.updateQueueDrawer();
        }
    }

    addToQueue(track) {
        if (!track) return;
        this.queue.push(track);
        if (this.currentIndex === -1) {
            this.playTrackAtIndex(0);
        }
        if (window.soundFlowApp) {
            window.soundFlowApp.updateQueueDrawer();
            window.soundFlowApp.showToast(`Додано до черги: ${track.title}`);
        }
    }

    async playTrackAtIndex(index, forceRefresh = false) {
        if (index < 0 || index >= this.queue.length) return;

        const reqId = ++this.currentRequestId;
        this.currentIndex = index;
        const track = this.queue[index];
        this.currentTrack = track;
        this.isFetchingSimilar = false;
        if (forceRefresh) {
            this._lastErrorRetried = true;
        }

        this.updateNowPlayingUI(track);
        this.showLoading(true);

        try {
            let streamUrl = null;

            // Check if prefetched
            if (!forceRefresh && this.prefetchedTrackId === track.id && this.prefetchedUrl) {
                streamUrl = this.prefetchedUrl;
                this.prefetchedUrl = null;
                this.prefetchedTrackId = null;
            } else if (window.pywebview && window.pywebview.api) {
                streamUrl = await window.pywebview.api.get_stream(track, forceRefresh);
            }

            // Check if superseded while fetching stream URL
            if (this.currentRequestId !== reqId) {
                return;
            }

            if (!streamUrl) {
                throw new Error('Не вдалося отримати аудіопотік');
            }

            // Use persistent DOM audio element with guaranteed continuous WASAPI binding
            if (!this.audio) {
                this.audio = document.getElementById('soundflow-audio') || new Audio();
                this.setupAudioListeners();
            }

            // Ensure volume is applied properly
            this.audio.volume = this.isMuted ? 0 : (this.lastVolume || 0.8);

            // If same stream URL and not forceRefresh, simply replay from start
            if (this.audio.src === streamUrl && !forceRefresh) {
                this.audio.currentTime = 0;
            } else {
                this.audio.src = streamUrl;
            }

            try {
                const playPromise = this.audio.play();
                if (playPromise !== undefined) {
                    await playPromise;
                }
            } catch (playErr) {
                if (this.currentRequestId !== reqId) {
                    return;
                }
                if (playErr.name === 'AbortError') {
                    return;
                }
                throw playErr;
            }

            if (this.currentRequestId !== reqId) {
                return;
            }

            this._lastErrorRetried = false;
            this.showLoading(false);
            this.updateMediaSession(track);

            // Record history & notify app
            if (window.pywebview && window.pywebview.api) {
                window.pywebview.api.record_history(track);
            }
            if (window.soundFlowApp) {
                window.soundFlowApp.onTrackChange(track);
            }

        } catch (err) {
            if (this.currentRequestId !== reqId) {
                return;
            }
            console.error('Play error:', err);

            if (!forceRefresh && window.pywebview && window.pywebview.api) {
                console.warn('Retrying playTrackAtIndex with force-refreshed stream...');
                return this.playTrackAtIndex(index, true);
            }

            this.showLoading(false);
            if (window.soundFlowApp) {
                window.soundFlowApp.showToast('Не вдалося відтворити трек');
            }
        }
    }

    async play() {
        if (this.audio && this.audio.src && !this.audio.src.endsWith('/about:blank') && this.audio.src !== window.location.href) {
            try {
                if (this.audio.ended) {
                    this.audio.currentTime = 0;
                }
                const playPromise = this.audio.play();
                if (playPromise !== undefined) {
                    await playPromise;
                }
            } catch (e) {
                if (e.name !== 'AbortError') {
                    console.warn('Play resume error:', e);
                    if (this.currentIndex >= 0 && this.currentIndex < this.queue.length) {
                        this.playTrackAtIndex(this.currentIndex, true);
                    }
                }
            }
        } else if (this.currentIndex >= 0 && this.currentIndex < this.queue.length) {
            this.playTrackAtIndex(this.currentIndex);
        } else if (this.queue.length > 0) {
            this.playTrackAtIndex(0);
        }
    }

    pause() {
        if (this.audio) {
            try {
                this.audio.pause();
            } catch (e) {}
        }
    }

    togglePlayPause() {
        if (this.audio && !this.audio.paused) {
            this.pause();
        } else {
            this.play();
        }
    }

    async next() {
        if (this.queue.length === 0) return;

        let nextIdx = this.currentIndex + 1;
        if (this.shuffle) {
            nextIdx = Math.floor(Math.random() * this.queue.length);
        }

        if (nextIdx >= this.queue.length) {
            if (this.repeat === 'all') {
                nextIdx = 0;
            } else if (this.autoplay && this.currentTrack && window.soundFlowApp) {
                this.showLoading(true);
                const added = await window.soundFlowApp.fetchAndAppendSimilarTracks(this.currentTrack);
                this.showLoading(false);
                if (added && nextIdx < this.queue.length) {
                    this.playTrackAtIndex(nextIdx);
                    return;
                }
                return;
            } else {
                return;
            }
        }

        this.playTrackAtIndex(nextIdx);
    }

    prev() {
        if (this.audio && this.audio.currentTime > 3) {
            this.audio.currentTime = 0;
            return;
        }

        if (this.queue.length === 0) return;

        let prevIdx = this.currentIndex - 1;
        if (prevIdx < 0) {
            prevIdx = this.repeat === 'all' ? this.queue.length - 1 : 0;
        }
        this.playTrackAtIndex(prevIdx);
    }

    toggleShuffle() {
        this.shuffle = !this.shuffle;
        const btn = document.getElementById('btn-shuffle');
        if (btn) btn.classList.toggle('active', this.shuffle);
        if (window.soundFlowApp) {
            window.soundFlowApp.showToast(this.shuffle ? 'Випадковий порядок увімкнено' : 'Порядок за чергою');
        }
    }

    toggleRepeat() {
        const modes = ['off', 'all', 'one'];
        const nextIdx = (modes.indexOf(this.repeat) + 1) % modes.length;
        this.repeat = modes[nextIdx];

        const btn = document.getElementById('btn-repeat');
        const badge = document.getElementById('repeat-badge');

        if (btn && badge) {
            if (this.repeat === 'off') {
                btn.classList.remove('active');
                badge.style.display = 'none';
                btn.title = 'Повторення (Вимкнено)';
            } else if (this.repeat === 'all') {
                btn.classList.add('active');
                badge.style.display = 'none';
                btn.title = 'Повторювати все';
            } else {
                btn.classList.add('active');
                badge.style.display = 'flex';
                btn.title = 'Повторювати цей трек';
            }
        }
    }

    toggleAutoplay() {
        this.autoplay = !this.autoplay;
        this.updateAutoplayUI();
        if (window.soundFlowApp) {
            const tOn = window.soundFlowI18n ? window.soundFlowI18n.t('toast_autoplay_on') : 'Авторадіо увімкнено: схожі треки вмикатимуться автоматично ♾️';
            const tOff = window.soundFlowI18n ? window.soundFlowI18n.t('toast_autoplay_off') : 'Авторадіо вимкнено';
            window.soundFlowApp.showToast(this.autoplay ? tOn : tOff);
        }
    }

    updateAutoplayUI() {
        const btn = document.getElementById('btn-autoplay');
        const pill = document.getElementById('queue-autoplay-toggle');
        if (btn) {
            btn.classList.toggle('active', this.autoplay);
            btn.title = this.autoplay
                ? (window.soundFlowI18n ? window.soundFlowI18n.t('btn_autoplay') + ' (Увімкнено)' : 'Автоматичне радіо схожих треків (Увімкнено)')
                : (window.soundFlowI18n ? window.soundFlowI18n.t('btn_autoplay') + ' (Вимкнено)' : 'Автоматичне радіо схожих треків (Вимкнено)');
        }
        if (pill) {
            pill.classList.toggle('active', this.autoplay);
        }
    }

    seek(timeSeconds) {
        if (this.audio && this.audio.duration && !isNaN(this.audio.duration)) {
            this.audio.currentTime = Math.max(0, Math.min(timeSeconds, this.audio.duration));
        }
    }

    setVolume(value) {
        const v = Math.max(0, Math.min(1, parseFloat(value)));
        this.lastVolume = v;
        if (this.audio) {
            this.audio.volume = v;
        }
        if (v > 0) this.isMuted = false;
        this.updateVolumeUI();
    }

    toggleMute() {
        if (this.isMuted || (this.audio && this.audio.volume === 0)) {
            const v = this.lastVolume || 0.8;
            if (this.audio) this.audio.volume = v;
            this.isMuted = false;
        } else {
            this.lastVolume = (this.audio ? this.audio.volume : 0.8) || 0.8;
            if (this.audio) this.audio.volume = 0;
            this.isMuted = true;
        }
        this.updateVolumeUI();
    }

    onTrackEnded() {
        if (this.repeat === 'one') {
            if (this.audio) {
                this.audio.currentTime = 0;
                this.audio.play().catch(e => {
                    if (this.currentIndex >= 0) {
                        this.playTrackAtIndex(this.currentIndex, true);
                    }
                });
            }
        } else {
            this.next();
        }
    }

    onTimeUpdate() {
        if (!this.audio) return;
        const current = this.audio.currentTime || 0;
        const duration = this.audio.duration || 0;

        const currentElem = document.getElementById('time-current');
        const totalElem = document.getElementById('time-total');
        const seekbar = document.getElementById('seekbar');
        const progress = document.getElementById('seekbar-progress');

        if (currentElem) currentElem.textContent = this.formatTime(current);
        if (totalElem && duration > 0) totalElem.textContent = this.formatTime(duration);

        if (duration > 0 && seekbar && progress) {
            const pct = (current / duration) * 100;
            seekbar.value = pct;
            progress.style.width = `${pct}%`;

            // Pre-fetch next track or pre-fetch similar tracks if 20s remaining
            if (duration - current < 20) {
                if (this.currentIndex < this.queue.length - 1 && !this.prefetchedUrl) {
                    this.prefetchNextTrack();
                } else if (this.currentIndex === this.queue.length - 1 && this.autoplay && !this.isFetchingSimilar && window.soundFlowApp) {
                    if (this._prefetchedSimilarTrackId !== this.currentTrack?.id) {
                        this._prefetchedSimilarTrackId = this.currentTrack?.id;
                        this.isFetchingSimilar = true;
                        window.soundFlowApp.fetchAndAppendSimilarTracks(this.currentTrack).finally(() => {
                            this.isFetchingSimilar = false;
                        });
                    }
                }
            }
        }
    }

    onProgressUpdate() {
        if (!this.audio) return;
        const buffer = document.getElementById('seekbar-buffer');
        if (!buffer || !this.audio.duration) return;

        if (this.audio.buffered && this.audio.buffered.length > 0) {
            const bufferedEnd = this.audio.buffered.end(this.audio.buffered.length - 1);
            const duration = this.audio.duration;
            buffer.style.width = `${(bufferedEnd / duration) * 100}%`;
        }
    }

    async prefetchNextTrack() {
        const nextIdx = this.currentIndex + 1;
        if (nextIdx < this.queue.length && window.pywebview && window.pywebview.api) {
            const nextTrack = this.queue[nextIdx];
            try {
                this.prefetchedTrackId = nextTrack.id;
                this.prefetchedUrl = await window.pywebview.api.get_stream(nextTrack, false);
            } catch (e) {
                console.warn('Pre-fetch failed', e);
            }
        }
    }

    updateNowPlayingUI(track) {
        const thumb = document.getElementById('player-thumb');
        const thumbPh = document.getElementById('player-thumb-ph');
        const title = document.getElementById('player-title');
        const artist = document.getElementById('player-artist');
        const sourceBadge = document.getElementById('player-source-badge');
        const likeBtn = document.getElementById('player-like-btn');

        if (track.thumbnail) {
            thumb.src = track.thumbnail;
            thumb.style.display = 'block';
            thumbPh.style.display = 'none';
        } else {
            thumb.style.display = 'none';
            thumbPh.style.display = 'flex';
        }

        title.textContent = track.title || 'Unknown Title';
        title.title = track.title || '';
        artist.textContent = track.artist || 'Unknown Artist';

        if (sourceBadge) {
            sourceBadge.style.display = 'inline-block';
            sourceBadge.textContent = track.source === 'soundcloud' ? 'SC' : 'YT';
            sourceBadge.className = `source-tag ${track.source === 'soundcloud' ? 'sc' : 'yt'}`;
        }

        if (likeBtn) {
            likeBtn.style.display = 'flex';
        }
    }

    updatePlayPauseUI(isPlaying) {
        const playIcon = document.getElementById('play-icon');
        const pauseIcon = document.getElementById('pause-icon');
        if (playIcon && pauseIcon) {
            playIcon.style.display = isPlaying ? 'none' : 'block';
            pauseIcon.style.display = isPlaying ? 'block' : 'none';
        }
    }

    showLoading(isLoading) {
        this.isLoading = isLoading;
        const spinner = document.getElementById('player-spinner');
        const playIcon = document.getElementById('play-icon');
        const pauseIcon = document.getElementById('pause-icon');

        if (spinner) spinner.style.display = isLoading ? 'block' : 'none';
        if (isLoading) {
            if (playIcon) playIcon.style.display = 'none';
            if (pauseIcon) pauseIcon.style.display = 'none';
        } else {
            const isPlaying = this.audio ? !this.audio.paused : false;
            this.updatePlayPauseUI(isPlaying);
        }
    }

    updateVolumeUI() {
        const slider = document.getElementById('volume-slider');
        const highIcon = document.getElementById('vol-high-icon');
        const muteIcon = document.getElementById('vol-mute-icon');

        const curVol = this.audio ? this.audio.volume : this.lastVolume;
        if (slider) slider.value = curVol;

        const isZero = curVol === 0 || this.isMuted;
        if (highIcon) highIcon.style.display = isZero ? 'none' : 'block';
        if (muteIcon) muteIcon.style.display = isZero ? 'block' : 'none';
    }

    formatTime(seconds) {
        if (isNaN(seconds) || seconds < 0) return '0:00';
        const sec = Math.floor(seconds);
        const m = Math.floor(sec / 60);
        const s = sec % 60;
        return `${m}:${s < 10 ? '0' : ''}${s}`;
    }
}
