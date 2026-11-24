import 'dart:math' as math;

import 'package:flutter/material.dart';

/// Live UI preferences for the Cider / Apple Music inspired SoundFlow Preview.
class RedesignPrefs extends ChangeNotifier {
  String vizMode = 'bars'; // 'bars' | 'wave' | 'mirror' | 'off'
  bool showLibrarySidebar = true;
  bool showQueuePanel = true;
  bool immersiveOpen = false;
  bool immersiveVisualizerTab = false; // false = Focus Lines, true = Big Visualizer
  bool miniPlayerOpen = false;
  bool pinnedExpanded = true;
  bool recentsExpanded = true;
  bool playlistsExpanded = true;

  void cycleVizMode() {
    const order = ['bars', 'wave', 'mirror', 'off'];
    vizMode = order[(order.indexOf(vizMode) + 1) % order.length];
    notifyListeners();
  }

  void setVizMode(String mode) {
    if (vizMode == mode) return;
    vizMode = mode;
    notifyListeners();
  }

  void toggleLibrarySidebar() {
    showLibrarySidebar = !showLibrarySidebar;
    notifyListeners();
  }

  void toggleQueuePanel() {
    showQueuePanel = !showQueuePanel;
    notifyListeners();
  }

  void setImmersive(bool value) {
    if (immersiveOpen == value) return;
    immersiveOpen = value;
    notifyListeners();
  }

  void toggleImmersive() => setImmersive(!immersiveOpen);

  void setImmersiveVisualizerTab(bool value) {
    if (immersiveVisualizerTab == value) return;
    immersiveVisualizerTab = value;
    notifyListeners();
  }

  void toggleMiniPlayer() {
    miniPlayerOpen = !miniPlayerOpen;
    notifyListeners();
  }

  void togglePinned() {
    pinnedExpanded = !pinnedExpanded;
    notifyListeners();
  }

  void toggleRecents() {
    recentsExpanded = !recentsExpanded;
    notifyListeners();
  }

  void togglePlaylists() {
    playlistsExpanded = !playlistsExpanded;
    notifyListeners();
  }
}

final redesignPrefs = RedesignPrefs();

/// Deterministic rich muted ambient glow color per track (0 extra RAM, 0 blur).
Color ambientToneFor(String seedKey) {
  const tones = [
    Color(0xff7a2e4d), // warm magenta-plum (matches reference bottle glow!)
    Color(0xff2c5378), // deep ocean sapphire
    Color(0xff7c4b27), // warm amber copper
    Color(0xff355e47), // deep emerald pine
    Color(0xff5a3775), // muted twilight violet
    Color(0xff783333), // velvet crimson
  ];
  if (seedKey.isEmpty) return tones.first;
  var hash = 0;
  for (var i = 0; i < seedKey.length; i++) {
    hash = (hash * 31 + seedKey.codeUnitAt(i)) & 0x7fffffff;
  }
  return tones[hash % tones.length];
}

/// Lightweight vector audio visualizer (0 BackdropFilter, <1 MB RAM, 0 FPS when paused).
class SoundFlowVisualizer extends StatefulWidget {
  const SoundFlowVisualizer({
    super.key,
    required this.playing,
    required this.progress,
    required this.volume,
    required this.mode,
    required this.accent,
    required this.muted,
    this.height = 30,
    this.onSeekFraction,
    this.seed = 1,
  });

  final bool playing;
  final double progress;
  final double volume;
  final String mode;
  final Color accent;
  final Color muted;
  final double height;
  final ValueChanged<double>? onSeekFraction;
  final int seed;

  @override
  State<SoundFlowVisualizer> createState() => _SoundFlowVisualizerState();
}

class _SoundFlowVisualizerState extends State<SoundFlowVisualizer>
    with SingleTickerProviderStateMixin {
  late final AnimationController _ticker = AnimationController(
    vsync: this,
    duration: const Duration(seconds: 12),
  );
  double? _dragFraction;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _syncTicker();
  }

  @override
  void didUpdateWidget(covariant SoundFlowVisualizer oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.playing != widget.playing || oldWidget.mode != widget.mode) {
      _syncTicker();
    }
  }

  void _syncTicker() {
    final disabled =
        mounted && MediaQuery.maybeDisableAnimationsOf(context) == true;
    final shouldRun = widget.playing && widget.mode != 'off' && !disabled;
    if (shouldRun) {
      if (!_ticker.isAnimating) _ticker.repeat();
    } else {
      if (_ticker.isAnimating) _ticker.stop();
    }
  }

  @override
  void dispose() {
    _ticker.dispose();
    super.dispose();
  }

  double _fractionFor(double dx, double width) =>
      width <= 0 ? 0 : (dx / width).clamp(0.0, 1.0);

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, box) {
      final width = box.maxWidth;
      final effectiveProgress = (_dragFraction ?? widget.progress).clamp(
        0.0,
        1.0,
      );
      return MouseRegion(
        cursor: widget.onSeekFraction != null
            ? SystemMouseCursors.click
            : SystemMouseCursors.basic,
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTapDown: widget.onSeekFraction == null
              ? null
              : (d) => widget.onSeekFraction!(
                  _fractionFor(d.localPosition.dx, width),
                ),
          onHorizontalDragStart: widget.onSeekFraction == null
              ? null
              : (d) => setState(
                  () => _dragFraction = _fractionFor(d.localPosition.dx, width),
                ),
          onHorizontalDragUpdate: widget.onSeekFraction == null
              ? null
              : (d) => setState(
                  () => _dragFraction = _fractionFor(d.localPosition.dx, width),
                ),
          onHorizontalDragEnd: widget.onSeekFraction == null
              ? null
              : (_) {
                  final target = _dragFraction;
                  setState(() => _dragFraction = null);
                  if (target != null) widget.onSeekFraction!(target);
                },
          onHorizontalDragCancel: () => setState(() => _dragFraction = null),
          child: SizedBox(
            height: widget.height,
            width: double.infinity,
            child: AnimatedBuilder(
              animation: _ticker,
              builder: (context, _) => CustomPaint(
                painter: _VisualizerPainter(
                  phase: _ticker.value * math.pi * 24,
                  playing: widget.playing,
                  progress: effectiveProgress,
                  volume: widget.volume.clamp(0.0, 1.0),
                  mode: widget.mode,
                  accent: widget.accent,
                  muted: widget.muted,
                  seed: widget.seed,
                ),
              ),
            ),
          ),
        ),
      );
    },
  );
}

class _VisualizerPainter extends CustomPainter {
  _VisualizerPainter({
    required this.phase,
    required this.playing,
    required this.progress,
    required this.volume,
    required this.mode,
    required this.accent,
    required this.muted,
    required this.seed,
  });

  final double phase;
  final bool playing;
  final double progress;
  final double volume;
  final String mode;
  final Color accent;
  final Color muted;
  final int seed;

  double _band(int i, int count) {
    final n = count <= 1 ? 0.0 : i / (count - 1);
    final seedShift = (seed % 17) * 0.37;
    if (!playing) {
      final base =
          0.20 +
          0.16 * math.sin(i * 0.72 + seedShift).abs() +
          0.12 * math.cos(i * 1.35 + seedShift).abs();
      return base.clamp(0.12, 0.52);
    }
    final volScale = 0.30 + 0.70 * volume;
    final kick = math.pow(0.5 + 0.5 * math.sin(phase * 1.15), 3).toDouble();
    final low = math.exp(-n * 2.8) * (0.42 + 0.58 * kick);
    final mid =
        (0.35 + 0.35 * math.sin(phase * 1.85 + i * 0.45 + seedShift)) *
        math.exp(-math.pow((n - 0.38) * 3.0, 2));
    final hi =
        (0.22 +
            0.28 *
                math.sin(phase * 3.4 + i * 1.05 + seedShift) *
                math.cos(phase * 1.4 - i * 0.52)) *
        n;
    return ((low + mid + hi * 0.65) * volScale).clamp(0.08, 1.0);
  }

  @override
  void paint(Canvas canvas, Size size) {
    final w = size.width;
    final h = size.height;
    if (w <= 4 || h <= 4) return;
    final progX = w * progress;

    if (mode == 'off') {
      final trackY = h / 2;
      canvas.drawLine(
        Offset(0, trackY),
        Offset(w, trackY),
        Paint()
          ..color = muted.withValues(alpha: 0.28)
          ..strokeWidth = 3
          ..strokeCap = StrokeCap.round,
      );
      canvas.drawLine(
        Offset(0, trackY),
        Offset(progX, trackY),
        Paint()
          ..color = accent
          ..strokeWidth = 3
          ..strokeCap = StrokeCap.round,
      );
      canvas.drawCircle(Offset(progX, trackY), 4.2, Paint()..color = accent);
      return;
    }

    if (mode == 'wave') {
      final mid = h / 2;
      final amp = (h * 0.40) * (playing ? (0.30 + 0.70 * volume) : 0.12);
      final playedPath = Path();
      final unplayedPath = Path();
      const step = 3.0;
      var sp = false, su = false;
      final seedShift = (seed % 17) * 0.41;

      for (double x = 0; x <= w; x += step) {
        final p = x / w;
        final env = math.sin(p * math.pi);
        final y =
            mid +
            (math.sin(p * 16 - phase * 2.1 + seedShift) * 0.65 +
                    math.cos(p * 8 + phase * 1.3 - seedShift) * 0.35) *
                amp *
                env;
        if (x <= progX) {
          if (!sp) {
            playedPath.moveTo(x, y);
            sp = true;
          } else {
            playedPath.lineTo(x, y);
          }
        } else {
          if (!su) {
            unplayedPath.moveTo(x, y);
            su = true;
          } else {
            unplayedPath.lineTo(x, y);
          }
        }
      }
      canvas.drawPath(
        unplayedPath,
        Paint()
          ..color = muted.withValues(alpha: 0.34)
          ..style = PaintingStyle.stroke
          ..strokeWidth = 1.8
          ..strokeCap = StrokeCap.round,
      );
      canvas.drawPath(
        playedPath,
        Paint()
          ..color = accent
          ..style = PaintingStyle.stroke
          ..strokeWidth = 2.0
          ..strokeCap = StrokeCap.round,
      );
      return;
    }

    final count = (w / 5.5).floor().clamp(16, 48);
    const gap = 2.0;
    final barW = (w - gap * (count - 1)) / count;
    final playedPaint = Paint()..color = accent.withValues(alpha: 0.94);
    final restPaint = Paint()..color = muted.withValues(alpha: 0.28);

    if (mode == 'mirror') {
      final mid = h / 2;
      for (var i = 0; i < count; i++) {
        final x = i * (barW + gap);
        final halfH = (_band(i, count) * (mid - 2)).clamp(1.5, mid - 1.5);
        canvas.drawRRect(
          RRect.fromRectAndRadius(
            Rect.fromLTRB(x, mid - halfH, x + barW, mid + halfH),
            const Radius.circular(1.5),
          ),
          x <= progX ? playedPaint : restPaint,
        );
      }
      return;
    }

    for (var i = 0; i < count; i++) {
      final x = i * (barW + gap);
      final bh = (_band(i, count) * (h - 4)).clamp(2.5, h - 2);
      canvas.drawRRect(
        RRect.fromRectAndRadius(
          Rect.fromLTWH(x, h - bh, barW, bh),
          const Radius.circular(1.5),
        ),
        x <= progX ? playedPaint : restPaint,
      );
    }
  }

  @override
  bool shouldRepaint(covariant _VisualizerPainter old) =>
      old.phase != phase ||
      old.playing != playing ||
      old.progress != progress ||
      old.volume != volume ||
      old.mode != mode ||
      old.accent != accent;
}

/// Tiny 4-bar animated equalizer matching the reference top-right mini-card.
class MiniEqBars extends StatefulWidget {
  const MiniEqBars({
    super.key,
    required this.playing,
    required this.color,
    this.width = 16,
    this.height = 13,
  });

  final bool playing;
  final Color color;
  final double width;
  final double height;

  @override
  State<MiniEqBars> createState() => _MiniEqBarsState();
}

class _MiniEqBarsState extends State<MiniEqBars>
    with SingleTickerProviderStateMixin {
  late final AnimationController _ctrl = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 1400),
  );

  @override
  void initState() {
    super.initState();
    _sync();
  }

  @override
  void didUpdateWidget(covariant MiniEqBars oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.playing != widget.playing) _sync();
  }

  void _sync() {
    final disabled =
        mounted && MediaQuery.maybeDisableAnimationsOf(context) == true;
    if (widget.playing && !disabled) {
      if (!_ctrl.isAnimating) _ctrl.repeat();
    } else {
      if (_ctrl.isAnimating) _ctrl.stop();
    }
  }

  @override
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => SizedBox(
    width: widget.width,
    height: widget.height,
    child: AnimatedBuilder(
      animation: _ctrl,
      builder: (context, _) => CustomPaint(
        painter: _MiniEqPainter(
          phase: _ctrl.value * math.pi * 2,
          playing: widget.playing,
          color: widget.color,
        ),
      ),
    ),
  );
}

class _MiniEqPainter extends CustomPainter {
  _MiniEqPainter({
    required this.phase,
    required this.playing,
    required this.color,
  });
  final double phase;
  final bool playing;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    const count = 4;
    const gap = 2.0;
    final barW = (size.width - gap * (count - 1)) / count;
    final paint = Paint()..color = color;
    for (var i = 0; i < count; i++) {
      final frac = playing
          ? (0.28 + 0.72 * (0.5 + 0.5 * math.sin(phase * 2 + i * 1.45)))
          : 0.25 + (i.isEven ? 0.15 : 0.0);
      final bh = (frac * size.height).clamp(2.2, size.height);
      final x = i * (barW + gap);
      canvas.drawRRect(
        RRect.fromRectAndRadius(
          Rect.fromLTWH(x, size.height - bh, barW, bh),
          const Radius.circular(1.2),
        ),
        paint,
      );
    }
  }

  @override
  bool shouldRepaint(covariant _MiniEqPainter old) =>
      old.phase != phase || old.playing != playing || old.color != color;
}
