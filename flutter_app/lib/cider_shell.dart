import 'dart:ui' as ui;

import 'package:flutter/material.dart';

import 'core.dart';
import 'design.dart';
import 'media_slider.dart';
import 'visualizer.dart';

String _remainingClock(double position, double duration) {
  if (duration <= 0) return '-0:00';
  final rem = (duration - position).clamp(0.0, duration);
  return '-${clock(rem)}';
}

String _queueSummary(List<Json> upcoming, double currentRemaining) {
  double total = currentRemaining;
  for (final t in upcoming) {
    final d = (t['duration'] as num?)?.toDouble() ?? 0;
    if (d > 0) total += d;
  }
  final mins = (total / 60).round();
  final end = DateTime.now().add(Duration(seconds: total.round()));
  final hh = end.hour.toString().padLeft(2, '0');
  final mm = end.minute.toString().padLeft(2, '0');
  if (mins <= 0) return '${upcoming.length} tracks · $hh:$mm';
  return '$mins min · $hh:$mm';
}

/// 1. Far-left 46px vertical icon rail matching the reference screenshot.
class CiderIconRail extends StatelessWidget {
  const CiderIconRail({
    super.key,
    required this.core,
    required this.onOpenConnections,
  });

  final Core core;
  final VoidCallback onOpenConnections;

  @override
  Widget build(BuildContext context) {
    final p = context.paints;
    Widget item(
      IconData icon,
      String tip, {
      required bool active,
      required VoidCallback onTap,
    }) => Tooltip(
      message: tip,
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 2),
        child: Material(
          color: active ? p.ink.withValues(alpha: .10) : Colors.transparent,
          borderRadius: BorderRadius.circular(8),
          child: InkWell(
            onTap: onTap,
            borderRadius: BorderRadius.circular(8),
            child: SizedBox(
              width: 34,
              height: 34,
              child: Icon(
                icon,
                size: 18,
                color: active ? p.ink : p.muted.withValues(alpha: .82),
              ),
            ),
          ),
        ),
      ),
    );

    return Container(
      width: 48,
      decoration: BoxDecoration(
        color: const Color(0xff101012),
        border: Border(
          right: BorderSide(color: p.ink.withValues(alpha: .06)),
        ),
      ),
      padding: const EdgeInsets.symmetric(vertical: 10),
      child: Column(
        children: [
          item(
            Icons.search_rounded,
            'Search (Ctrl+K)',
            active: core.route == 'search',
            onTap: () => core.nav('search'),
          ),
          const SizedBox(height: 6),
          item(
            Icons.home_rounded,
            'Home',
            active: core.route == 'home',
            onTap: () => core.nav('home'),
          ),
          item(
            Icons.grid_view_rounded,
            'Library',
            active: core.route == 'library',
            onTap: () => core.nav('library'),
          ),
          item(
            Icons.sensors_rounded,
            'Connections',
            active: core.route == 'connections',
            onTap: onOpenConnections,
          ),
          item(
            Icons.library_music_rounded,
            'Playlists',
            active: core.route == 'playlist',
            onTap: () => core.nav('library'),
          ),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
            child: Divider(height: 1, color: p.ink.withValues(alpha: .08)),
          ),
          item(
            Icons.schedule_rounded,
            'History',
            active: core.route == 'history',
            onTap: () => core.nav('history'),
          ),
          item(
            Icons.music_note_rounded,
            'Favorites',
            active: core.route == 'liked',
            onTap: () => core.nav('liked'),
          ),
          item(
            Icons.download_rounded,
            'Downloads',
            active: core.route == 'downloads',
            onTap: () => core.nav('downloads'),
          ),
          item(
            Icons.people_outline_rounded,
            'Profile',
            active: core.route == 'profile',
            onTap: () => core.nav('profile'),
          ),
          const Spacer(),
          item(
            Icons.settings_outlined,
            'Settings',
            active: core.route == 'settings',
            onTap: () => core.nav('settings'),
          ),
        ],
      ),
    );
  }
}

/// 2. Adjacent 228px Library panel (Pinned / Recents / Playlists with cover thumbnails).
class CiderLibraryPanel extends StatelessWidget {
  const CiderLibraryPanel({
    super.key,
    required this.core,
    required this.onCreatePlaylist,
  });

  final Core core;
  final VoidCallback onCreatePlaylist;

  @override
  Widget build(BuildContext context) {
    final p = context.paints;
    final pinnedPlaylists = core.playlists.take(2).toList();
    final recents = core.homeTracks.take(4).toList();
    final allPlaylists = core.playlists;

    Widget sectionHeader(
      String title,
      bool expanded,
      VoidCallback onToggle,
    ) => InkWell(
      onTap: onToggle,
      borderRadius: BorderRadius.circular(6),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(8, 10, 6, 5),
        child: Row(
          children: [
            Expanded(
              child: Text(
                title,
                style: TextStyle(
                  fontSize: 11,
                  fontWeight: FontWeight.w600,
                  color: p.muted.withValues(alpha: .82),
                ),
              ),
            ),
            Icon(
              expanded
                  ? Icons.keyboard_arrow_down_rounded
                  : Icons.keyboard_arrow_right_rounded,
              size: 16,
              color: p.muted.withValues(alpha: .72),
            ),
          ],
        ),
      ),
    );

    Widget mediaRow({
      required Widget leading,
      required String title,
      required String subtitle,
      bool selected = false,
      Color? titleColor,
      Widget? trailing,
      required VoidCallback onTap,
    }) => Padding(
      padding: const EdgeInsets.symmetric(vertical: 1.5),
      child: Material(
        color: selected ? p.ink.withValues(alpha: .08) : Colors.transparent,
        borderRadius: BorderRadius.circular(8),
        child: InkWell(
          onTap: onTap,
          borderRadius: BorderRadius.circular(8),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
            child: Row(
              children: [
                leading,
                const SizedBox(width: 10),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        title,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          fontSize: 12.5,
                          fontWeight: FontWeight.w600,
                          color: titleColor ?? p.ink,
                        ),
                      ),
                      const SizedBox(height: 1.5),
                      Text(
                        subtitle,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          fontSize: 10.5,
                          color: p.muted.withValues(alpha: .85),
                        ),
                      ),
                    ],
                  ),
                ),
                if (trailing != null) ...[
                  const SizedBox(width: 6),
                  trailing,
                ],
              ],
            ),
          ),
        ),
      ),
    );

    return Container(
      width: 234,
      decoration: BoxDecoration(
        color: const Color(0xff151518),
        border: Border(
          right: BorderSide(color: p.ink.withValues(alpha: .06)),
        ),
      ),
      child: Column(
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(14, 12, 10, 6),
            child: Row(
              children: [
                Expanded(
                  child: Text(
                    'Library',
                    style: TextStyle(
                      fontFamily: SoundFlowType.heading,
                      fontSize: 14,
                      fontWeight: FontWeight.w700,
                      color: p.ink,
                    ),
                  ),
                ),
                Tooltip(
                  message: 'New Playlist',
                  child: InkWell(
                    onTap: onCreatePlaylist,
                    borderRadius: BorderRadius.circular(7),
                    child: SizedBox(
                      width: 26,
                      height: 26,
                      child: Icon(Icons.add_rounded, size: 18, color: p.muted),
                    ),
                  ),
                ),
              ],
            ),
          ),
          Expanded(
            child: ListView(
              padding: const EdgeInsets.fromLTRB(6, 0, 6, 14),
              children: [
                sectionHeader(
                  'Pinned',
                  redesignPrefs.pinnedExpanded,
                  redesignPrefs.togglePinned,
                ),
                if (redesignPrefs.pinnedExpanded) ...[
                  mediaRow(
                    leading: Container(
                      width: 34,
                      height: 34,
                      decoration: BoxDecoration(
                        gradient: const LinearGradient(
                          begin: Alignment.topLeft,
                          end: Alignment.bottomRight,
                          colors: [Color(0xff2c2c34), Color(0xff1c1c22)],
                        ),
                        borderRadius: BorderRadius.circular(7),
                        border: Border.all(
                          color: Colors.white.withValues(alpha: .08),
                        ),
                      ),
                      child: const Icon(
                        Icons.star_rounded,
                        size: 18,
                        color: Color(0xfff5f5f7),
                      ),
                    ),
                    title: 'Favorite Songs',
                    subtitle: 'Playlist · ${core.liked.length} tracks',
                    selected: core.route == 'liked',
                    onTap: () => core.nav('liked'),
                  ),
                  for (final pl in pinnedPlaylists)
                    Builder(
                      builder: (context) {
                        final ts = objects(pl['tracks']);
                        return mediaRow(
                          leading: Artwork(
                            ts.isEmpty ? '' : trackCover(ts.first),
                            size: 34,
                            radius: 7,
                          ),
                          title: pl['name']?.toString() ?? 'Playlist',
                          subtitle: 'Playlist · ${ts.length} tracks',
                          selected: core.selectedPlaylist == pl['id'],
                          onTap: () => core.selectPlaylist(pl),
                        );
                      },
                    ),
                ],
                sectionHeader(
                  'Recents',
                  redesignPrefs.recentsExpanded,
                  redesignPrefs.toggleRecents,
                ),
                if (redesignPrefs.recentsExpanded) ...[
                  mediaRow(
                    leading: Container(
                      width: 34,
                      height: 34,
                      decoration: BoxDecoration(
                        gradient: const LinearGradient(
                          begin: Alignment.topLeft,
                          end: Alignment.bottomRight,
                          colors: [Color(0xff9c35d6), Color(0xff2d59c9)],
                        ),
                        borderRadius: BorderRadius.circular(7),
                      ),
                      child: const Icon(
                        Icons.auto_awesome_rounded,
                        size: 16,
                        color: Colors.white,
                      ),
                    ),
                    title: 'Discovery Station',
                    subtitle: 'Station · YouTube & SoundCloud',
                    selected: core.route == 'home',
                    titleColor: core.route == 'home'
                        ? const Color(0xfff15c7a)
                        : p.ink,
                    trailing: Icon(
                      Icons.more_horiz_rounded,
                      size: 15,
                      color: core.route == 'home'
                          ? const Color(0xfff15c7a)
                          : p.muted,
                    ),
                    onTap: () => core.nav('home'),
                  ),
                  for (final t in recents)
                    mediaRow(
                      leading: Artwork(trackCover(t), size: 34, radius: 7),
                      title: trackTitle(t, context),
                      subtitle:
                          '${trackArtist(t, context)} · ${sourceName(t, context)}',
                      selected: trackKey(core.current) == trackKey(t),
                      onTap: () => core.play(t, recents),
                    ),
                ],
                sectionHeader(
                  'Playlists',
                  redesignPrefs.playlistsExpanded,
                  redesignPrefs.togglePlaylists,
                ),
                if (redesignPrefs.playlistsExpanded) ...[
                  if (allPlaylists.isEmpty)
                    Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 10,
                        vertical: 6,
                      ),
                      child: Text(
                        'Click + to create your first playlist',
                        style: TextStyle(fontSize: 11, color: p.muted),
                      ),
                    )
                  else
                    for (final pl in allPlaylists)
                      Builder(
                        builder: (context) {
                          final ts = objects(pl['tracks']);
                          return mediaRow(
                            leading: Container(
                              width: 34,
                              height: 34,
                              decoration: BoxDecoration(
                                color: p.ink.withValues(alpha: .07),
                                borderRadius: BorderRadius.circular(7),
                              ),
                              child: ts.isNotEmpty
                                  ? Artwork(
                                      trackCover(ts.first),
                                      size: 34,
                                      radius: 7,
                                    )
                                  : Icon(
                                      Icons.folder_outlined,
                                      size: 17,
                                      color: p.muted,
                                    ),
                            ),
                            title: pl['name']?.toString() ?? 'Playlist',
                            subtitle: '${ts.length} tracks',
                            selected: core.selectedPlaylist == pl['id'],
                            trailing: Icon(
                              Icons.chevron_right_rounded,
                              size: 16,
                              color: p.muted.withValues(alpha: .6),
                            ),
                            onTap: () => core.selectPlaylist(pl),
                          );
                        },
                      ),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// 3. Floating Top Toolbar matching the reference circular capsules (< > [|] ... right pills).
class CiderTopBar extends StatelessWidget {
  const CiderTopBar({
    super.key,
    required this.core,
    required this.onCreatePlaylist,
    required this.onOpenConnections,
  });

  final Core core;
  final VoidCallback onCreatePlaylist;
  final VoidCallback onOpenConnections;

  @override
  Widget build(BuildContext context) {
    final p = context.paints;
    Widget circleBtn(
      IconData icon,
      String tip, {
      bool active = false,
      VoidCallback? onTap,
    }) => Tooltip(
      message: tip,
      child: Material(
        color: active
            ? Colors.white.withValues(alpha: .18)
            : Colors.white.withValues(alpha: .08),
        shape: const CircleBorder(),
        child: InkWell(
          onTap: onTap,
          customBorder: const CircleBorder(),
          child: SizedBox(
            width: 30,
            height: 30,
            child: Icon(
              icon,
              size: 16,
              color: onTap == null
                  ? p.muted.withValues(alpha: .35)
                  : active
                  ? Colors.white
                  : p.ink.withValues(alpha: .88),
            ),
          ),
        ),
      ),
    );

    return SizedBox(
      height: 52,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 18),
        child: Row(
          children: [
            circleBtn(
              Icons.more_horiz_rounded,
              'Settings',
              active: core.route == 'settings',
              onTap: () => core.nav('settings'),
            ),
            const SizedBox(width: 8),
            Container(
              height: 30,
              decoration: BoxDecoration(
                color: Colors.white.withValues(alpha: .08),
                borderRadius: BorderRadius.circular(15),
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  InkWell(
                    onTap: core.canBack ? core.back : null,
                    borderRadius: const BorderRadius.horizontal(
                      left: Radius.circular(15),
                    ),
                    child: SizedBox(
                      width: 28,
                      height: 30,
                      child: Icon(
                        Icons.chevron_left_rounded,
                        size: 18,
                        color: core.canBack
                            ? p.ink
                            : p.muted.withValues(alpha: .35),
                      ),
                    ),
                  ),
                  Container(
                    width: 1,
                    height: 14,
                    color: Colors.white.withValues(alpha: .10),
                  ),
                  InkWell(
                    onTap: () => core.nav('home'),
                    borderRadius: const BorderRadius.horizontal(
                      right: Radius.circular(15),
                    ),
                    child: SizedBox(
                      width: 28,
                      height: 30,
                      child: Icon(
                        Icons.chevron_right_rounded,
                        size: 18,
                        color: p.muted.withValues(alpha: .6),
                      ),
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(width: 8),
            circleBtn(
              Icons.view_sidebar_outlined,
              redesignPrefs.showLibrarySidebar
                  ? 'Hide Library sidebar'
                  : 'Show Library sidebar',
              active: redesignPrefs.showLibrarySidebar,
              onTap: redesignPrefs.toggleLibrarySidebar,
            ),
            const Spacer(),
            if (MediaQuery.sizeOf(context).width > 980 &&
                core.route != 'search') ...[
              Material(
                color: Colors.white.withValues(alpha: .07),
                borderRadius: BorderRadius.circular(15),
                child: InkWell(
                  onTap: () => core.nav('search'),
                  borderRadius: BorderRadius.circular(15),
                  child: SizedBox(
                    height: 30,
                    width: 206,
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 11),
                      child: Row(
                        children: [
                          Icon(Icons.search_rounded, size: 15, color: p.muted),
                          const SizedBox(width: 7),
                          Expanded(
                            child: Text(
                              'Search or paste URL',
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: TextStyle(fontSize: 11.5, color: p.muted),
                            ),
                          ),
                          Text(
                            'Ctrl+K',
                            style: TextStyle(
                              fontSize: 9.5,
                              color: p.muted.withValues(alpha: .7),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
              ),
              const SizedBox(width: 10),
            ],
            circleBtn(
              Icons.add_rounded,
              'New Playlist',
              onTap: onCreatePlaylist,
            ),
            const SizedBox(width: 6),
            circleBtn(
              Icons.picture_in_picture_alt_rounded,
              'Toggle Floating Mini-Player Card',
              active: redesignPrefs.miniPlayerOpen,
              onTap: redesignPrefs.toggleMiniPlayer,
            ),
            const SizedBox(width: 6),
            circleBtn(
              Icons.format_quote_rounded,
              'Immersive Lyrics & Visualizer Stage',
              active: redesignPrefs.immersiveOpen,
              onTap: redesignPrefs.toggleImmersive,
            ),
            const SizedBox(width: 6),
            circleBtn(
              Icons.queue_music_rounded,
              redesignPrefs.showQueuePanel
                  ? 'Hide Up Next Queue'
                  : 'Show Up Next Queue',
              active: redesignPrefs.showQueuePanel,
              onTap: redesignPrefs.toggleQueuePanel,
            ),
            const SizedBox(width: 6),
            circleBtn(
              Icons.graphic_eq_rounded,
              'Visualizer: ${redesignPrefs.vizMode.toUpperCase()}',
              active: redesignPrefs.vizMode != 'off',
              onTap: redesignPrefs.cycleVizMode,
            ),
            const SizedBox(width: 6),
            circleBtn(
              Icons.sensors_rounded,
              'Connections & Discord',
              onTap: onOpenConnections,
            ),
            const SizedBox(width: 6),
            circleBtn(
              Icons.person_outline_rounded,
              'Profile',
              active: core.route == 'profile',
              onTap: () => core.nav('profile'),
            ),
          ],
        ),
      ),
    );
  }
}

/// 4. Iconic Center Hero Banner (big square cover, huge uppercase display title,
/// artist avatar, metadata micro-pills, solid white Play pill + smoked Shuffle pill).
class CiderHeroHeader extends StatelessWidget {
  const CiderHeroHeader({
    super.key,
    required this.core,
    required this.cover,
    required this.title,
    required this.subtitle,
    required this.pills,
    required this.onPlay,
    required this.onShuffle,
    this.onAdd,
    this.onMore,
    this.ambientKey = '',
  });

  final Core core;
  final String cover;
  final String title;
  final String subtitle;
  final List<String> pills;
  final VoidCallback? onPlay;
  final VoidCallback? onShuffle;
  final VoidCallback? onAdd;
  final VoidCallback? onMore;
  final String ambientKey;

  @override
  Widget build(BuildContext context) {
    final p = context.paints;
    final tone = ambientToneFor(ambientKey.isEmpty ? title : ambientKey);
    return LayoutBuilder(
      builder: (context, box) {
        final compact = box.maxWidth < 640;
        final artSize = compact ? 156.0 : 212.0;
        return Container(
          padding: const EdgeInsets.fromLTRB(22, 18, 22, 24),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(18),
            gradient: RadialGradient(
              center: const Alignment(-0.32, -0.35),
              radius: 1.28,
              colors: [
                tone.withValues(alpha: .46),
                tone.withValues(alpha: .16),
                const Color(0xff18181c).withValues(alpha: .45),
              ],
              stops: const [0.0, 0.48, 1.0],
            ),
            border: Border.all(color: Colors.white.withValues(alpha: .06)),
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Container(
                decoration: BoxDecoration(
                  borderRadius: BorderRadius.circular(14),
                  boxShadow: [
                    BoxShadow(
                      color: Colors.black.withValues(alpha: .48),
                      blurRadius: 28,
                      offset: const Offset(0, 14),
                    ),
                  ],
                  border: Border.all(
                    color: Colors.white.withValues(alpha: .10),
                  ),
                ),
                child: Artwork(cover, size: artSize, radius: 13),
              ),
              SizedBox(width: compact ? 18 : 26),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: [
                    Text(
                      title.toUpperCase(),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        fontFamily: SoundFlowType.heading,
                        fontSize: compact ? 26 : 38,
                        height: 1.05,
                        letterSpacing: -1.1,
                        fontWeight: FontWeight.w700,
                        color: Colors.white,
                      ),
                    ),
                    const SizedBox(height: 10),
                    Row(
                      children: [
                        Container(
                          width: 20,
                          height: 20,
                          decoration: BoxDecoration(
                            color: tone.withValues(alpha: .75),
                            shape: BoxShape.circle,
                            border: Border.all(
                              color: Colors.white.withValues(alpha: .28),
                            ),
                          ),
                          child: const Icon(
                            Icons.music_note_rounded,
                            size: 11,
                            color: Colors.white,
                          ),
                        ),
                        const SizedBox(width: 8),
                        Expanded(
                          child: Text(
                            subtitle,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(
                              fontSize: 13.5,
                              fontWeight: FontWeight.w500,
                              color: Color(0xe6ffffff),
                            ),
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 10),
                    Wrap(
                      spacing: 6,
                      runSpacing: 6,
                      children: [
                        for (final pill in pills)
                          Container(
                            padding: const EdgeInsets.symmetric(
                              horizontal: 9,
                              vertical: 3.5,
                            ),
                            decoration: BoxDecoration(
                              color: Colors.black.withValues(alpha: .28),
                              borderRadius: BorderRadius.circular(12),
                              border: Border.all(
                                color: Colors.white.withValues(alpha: .10),
                              ),
                            ),
                            child: Text(
                              pill,
                              style: const TextStyle(
                                fontSize: 10.5,
                                fontWeight: FontWeight.w600,
                                color: Color(0xd9ffffff),
                              ),
                            ),
                          ),
                      ],
                    ),
                    const SizedBox(height: 18),
                    Row(
                      children: [
                        Material(
                          color: Colors.white,
                          borderRadius: BorderRadius.circular(18),
                          child: InkWell(
                            onTap: onPlay,
                            borderRadius: BorderRadius.circular(18),
                            child: SizedBox(
                              height: 36,
                              child: Padding(
                                padding: const EdgeInsets.symmetric(
                                  horizontal: 20,
                                ),
                                child: Row(
                                  mainAxisSize: MainAxisSize.min,
                                  children: const [
                                    Icon(
                                      Icons.play_arrow_rounded,
                                      size: 20,
                                      color: Color(0xff111113),
                                    ),
                                    SizedBox(width: 5),
                                    Text(
                                      'Play',
                                      style: TextStyle(
                                        fontSize: 13,
                                        fontWeight: FontWeight.w700,
                                        color: Color(0xff111113),
                                      ),
                                    ),
                                  ],
                                ),
                              ),
                            ),
                          ),
                        ),
                        const SizedBox(width: 10),
                        Material(
                          color: Colors.white.withValues(alpha: .13),
                          borderRadius: BorderRadius.circular(18),
                          child: InkWell(
                            onTap: onShuffle,
                            borderRadius: BorderRadius.circular(18),
                            child: SizedBox(
                              height: 36,
                              child: Padding(
                                padding: const EdgeInsets.symmetric(
                                  horizontal: 18,
                                ),
                                child: Row(
                                  mainAxisSize: MainAxisSize.min,
                                  children: const [
                                    Icon(
                                      Icons.shuffle_rounded,
                                      size: 16,
                                      color: Colors.white,
                                    ),
                                    SizedBox(width: 6),
                                    Text(
                                      'Shuffle',
                                      style: TextStyle(
                                        fontSize: 13,
                                        fontWeight: FontWeight.w600,
                                        color: Colors.white,
                                      ),
                                    ),
                                  ],
                                ),
                              ),
                            ),
                          ),
                        ),
                        const Spacer(),
                        if (onAdd != null)
                          Material(
                            color: Colors.white.withValues(alpha: .11),
                            borderRadius: BorderRadius.circular(18),
                            child: InkWell(
                              onTap: onAdd,
                              borderRadius: BorderRadius.circular(18),
                              child: const SizedBox(
                                height: 34,
                                child: Padding(
                                  padding: EdgeInsets.symmetric(horizontal: 14),
                                  child: Row(
                                    mainAxisSize: MainAxisSize.min,
                                    children: [
                                      Icon(
                                        Icons.add_rounded,
                                        size: 16,
                                        color: Colors.white,
                                      ),
                                      SizedBox(width: 4),
                                      Text(
                                        'Add',
                                        style: TextStyle(
                                          fontSize: 12.5,
                                          fontWeight: FontWeight.w600,
                                          color: Colors.white,
                                        ),
                                      ),
                                    ],
                                  ),
                                ),
                              ),
                            ),
                          ),
                        if (onMore != null) ...[
                          const SizedBox(width: 8),
                          Material(
                            color: Colors.white.withValues(alpha: .11),
                            shape: const CircleBorder(),
                            child: InkWell(
                              onTap: onMore,
                              customBorder: const CircleBorder(),
                              child: SizedBox(
                                width: 34,
                                height: 34,
                                child: Icon(
                                  Icons.more_horiz_rounded,
                                  size: 18,
                                  color: p.ink,
                                ),
                              ),
                            ),
                          ),
                        ],
                      ],
                    ),
                  ],
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}

/// 5. Right 276px Persistent "Up Next" Queue Panel + Compact Playing Card at Top!
class CiderQueuePanel extends StatelessWidget {
  const CiderQueuePanel({super.key, required this.core});

  final Core core;

  @override
  Widget build(BuildContext context) {
    final p = context.paints;
    return ValueListenableBuilder<Json>(
      valueListenable: core.playback,
      builder: (context, pb, _) {
        final track = object(pb['track']);
        final hasTrack = track.isNotEmpty;
        final playing = pb['playing'] == true;
        final pos = (pb['position'] as num?)?.toDouble() ?? 0;
        final dur = (pb['duration'] as num?)?.toDouble() ?? 0;
        final frac = dur > 0 ? (pos / dur).clamp(0.0, 1.0) : 0.0;
        final queue = core.queue;
        final curIdx = queue.isEmpty
            ? 0
            : core.queueIndex.clamp(0, queue.length - 1);
        final upcoming = <(int, Json)>[
          for (var i = curIdx + 1; i < queue.length; i++) (i, queue[i]),
        ];

        return Container(
          width: 278,
          decoration: BoxDecoration(
            color: const Color(0xff141417),
            border: Border(
              left: BorderSide(color: p.ink.withValues(alpha: .06)),
            ),
          ),
          child: Column(
            children: [
              // Compact Now Playing Card at top of right column (mirrors screenshot top-right!)
              if (hasTrack)
                GestureDetector(
                  onTap: redesignPrefs.toggleImmersive,
                  child: Container(
                    margin: const EdgeInsets.fromLTRB(10, 10, 10, 8),
                    padding: const EdgeInsets.all(10),
                    decoration: BoxDecoration(
                      color: const Color(0xff1b1b20),
                      borderRadius: BorderRadius.circular(12),
                      border: Border.all(
                        color: Colors.white.withValues(alpha: .08),
                      ),
                    ),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Row(
                          children: [
                            Artwork(trackCover(track), size: 46, radius: 8),
                            const SizedBox(width: 10),
                            Expanded(
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Text(
                                    trackTitle(track, context).toUpperCase(),
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                    style: const TextStyle(
                                      fontFamily: SoundFlowType.heading,
                                      fontSize: 12,
                                      fontWeight: FontWeight.w700,
                                      color: Colors.white,
                                    ),
                                  ),
                                  const SizedBox(height: 2),
                                  Text(
                                    trackArtist(track, context),
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                    style: TextStyle(
                                      fontSize: 11,
                                      color: p.muted,
                                    ),
                                  ),
                                ],
                              ),
                            ),
                            const SizedBox(width: 6),
                            MiniEqBars(
                              playing: playing,
                              color: Colors.white.withValues(alpha: .9),
                            ),
                          ],
                        ),
                        const SizedBox(height: 9),
                        GestureDetector(
                          behavior: HitTestBehavior.opaque,
                          onTapDown: dur > 0
                              ? (d) {
                                  final w = 236.0;
                                  final f = (d.localPosition.dx / w).clamp(
                                    0.0,
                                    1.0,
                                  );
                                  core.command({
                                    'type': 'seek',
                                    'value': f * dur,
                                  });
                                }
                              : null,
                          child: ClipRRect(
                            borderRadius: BorderRadius.circular(2),
                            child: LinearProgressIndicator(
                              value: frac,
                              minHeight: 3,
                              backgroundColor: Colors.white.withValues(
                                alpha: .14,
                              ),
                              valueColor: const AlwaysStoppedAnimation<Color>(
                                Colors.white,
                              ),
                            ),
                          ),
                        ),
                        const SizedBox(height: 5),
                        Row(
                          mainAxisAlignment: MainAxisAlignment.spaceBetween,
                          children: [
                            Text(
                              clock(pos),
                              style: TextStyle(
                                fontSize: 10,
                                color: p.muted,
                                fontFeatures: const [
                                  ui.FontFeature.tabularFigures(),
                                ],
                              ),
                            ),
                            Text(
                              _remainingClock(pos, dur),
                              style: TextStyle(
                                fontSize: 10,
                                color: p.muted,
                                fontFeatures: const [
                                  ui.FontFeature.tabularFigures(),
                                ],
                              ),
                            ),
                          ],
                        ),
                      ],
                    ),
                  ),
                ),

              // Queue Header
              Padding(
                padding: const EdgeInsets.fromLTRB(14, 6, 10, 6),
                child: Row(
                  children: [
                    Text(
                      'Up Next',
                      style: TextStyle(
                        fontFamily: SoundFlowType.heading,
                        fontSize: 12.5,
                        fontWeight: FontWeight.w700,
                        color: p.ink,
                      ),
                    ),
                    const SizedBox(width: 6),
                    Text(
                      '${upcoming.length}',
                      style: TextStyle(fontSize: 11, color: p.muted),
                    ),
                    const Spacer(),
                    Tooltip(
                      message: 'Shuffle',
                      child: InkWell(
                        onTap: () => core.player('shuffle'),
                        borderRadius: BorderRadius.circular(6),
                        child: Padding(
                          padding: const EdgeInsets.all(4),
                          child: Icon(
                            Icons.shuffle_rounded,
                            size: 15,
                            color: core.settings['shuffle'] == true
                                ? p.accent
                                : p.muted,
                          ),
                        ),
                      ),
                    ),
                    Tooltip(
                      message: 'Repeat',
                      child: InkWell(
                        onTap: () => core.player('repeat'),
                        borderRadius: BorderRadius.circular(6),
                        child: Padding(
                          padding: const EdgeInsets.all(4),
                          child: Icon(
                            core.settings['repeat'] == 'one'
                                ? Icons.repeat_one_rounded
                                : Icons.repeat_rounded,
                            size: 15,
                            color:
                                (core.settings['repeat'] ?? 'off') != 'off'
                                ? p.accent
                                : p.muted,
                          ),
                        ),
                      ),
                    ),
                    Tooltip(
                      message: 'Clear Queue',
                      child: InkWell(
                        onTap: upcoming.isEmpty
                            ? null
                            : () => core.action('queue_clear'),
                        borderRadius: BorderRadius.circular(6),
                        child: Padding(
                          padding: const EdgeInsets.all(4),
                          child: Icon(
                            Icons.delete_outline_rounded,
                            size: 15,
                            color: upcoming.isEmpty
                                ? p.muted.withValues(alpha: .3)
                                : p.muted,
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
              ),

              // Upcoming Queue Items
              Expanded(
                child: upcoming.isEmpty
                    ? Center(
                        child: Padding(
                          padding: const EdgeInsets.all(20),
                          child: Text(
                            'Play any song or playlist to populate Up Next.',
                            textAlign: TextAlign.center,
                            style: TextStyle(fontSize: 12, color: p.muted),
                          ),
                        ),
                      )
                    : ListView.builder(
                        padding: const EdgeInsets.symmetric(horizontal: 8),
                        itemCount: upcoming.length,
                        itemExtent: 52,
                        itemBuilder: (context, idx) {
                          final (absIndex, item) = upcoming[idx];
                          return Material(
                            color: Colors.transparent,
                            borderRadius: BorderRadius.circular(8),
                            child: InkWell(
                              onTap: () => core.action('queue_play', {
                                'index': absIndex,
                              }),
                              borderRadius: BorderRadius.circular(8),
                              child: Padding(
                                padding: const EdgeInsets.symmetric(
                                  horizontal: 6,
                                  vertical: 5,
                                ),
                                child: Row(
                                  children: [
                                    Artwork(
                                      trackCover(item),
                                      size: 38,
                                      radius: 6,
                                    ),
                                    const SizedBox(width: 10),
                                    Expanded(
                                      child: Column(
                                        mainAxisAlignment:
                                            MainAxisAlignment.center,
                                        crossAxisAlignment:
                                            CrossAxisAlignment.start,
                                        children: [
                                          Text(
                                            trackTitle(item, context),
                                            maxLines: 1,
                                            overflow: TextOverflow.ellipsis,
                                            style: TextStyle(
                                              fontSize: 12,
                                              fontWeight: FontWeight.w600,
                                              color: p.ink,
                                            ),
                                          ),
                                          const SizedBox(height: 1.5),
                                          Text(
                                            trackArtist(item, context),
                                            maxLines: 1,
                                            overflow: TextOverflow.ellipsis,
                                            style: TextStyle(
                                              fontSize: 10.5,
                                              color: p.muted,
                                            ),
                                          ),
                                        ],
                                      ),
                                    ),
                                    InkWell(
                                      onTap: () => core.action('queue_remove', {
                                        'index': absIndex,
                                      }),
                                      borderRadius: BorderRadius.circular(6),
                                      child: Padding(
                                        padding: const EdgeInsets.all(4),
                                        child: Icon(
                                          Icons.close_rounded,
                                          size: 13,
                                          color: p.muted.withValues(alpha: .55),
                                        ),
                                      ),
                                    ),
                                  ],
                                ),
                              ),
                            ),
                          );
                        },
                      ),
              ),

              // Bottom Queue Summary Footer (matches "33 min · 3:07 PM" in screenshot!)
              Container(
                height: 36,
                padding: const EdgeInsets.symmetric(horizontal: 14),
                decoration: BoxDecoration(
                  border: Border(
                    top: BorderSide(color: p.ink.withValues(alpha: .06)),
                  ),
                ),
                child: Row(
                  children: [
                    Icon(
                      Icons.schedule_rounded,
                      size: 13,
                      color: p.muted.withValues(alpha: .75),
                    ),
                    const SizedBox(width: 6),
                    Expanded(
                      child: Text(
                        _queueSummary(
                          upcoming.map((e) => e.$2).toList(),
                          (dur - pos).clamp(0.0, dur),
                        ),
                        style: TextStyle(
                          fontSize: 11,
                          color: p.muted.withValues(alpha: .85),
                          fontFeatures: const [ui.FontFeature.tabularFigures()],
                        ),
                      ),
                    ),
                    Tooltip(
                      message: 'Dynamic Island (Ctrl+Alt+M)',
                      child: InkWell(
                        onTap: core.island,
                        borderRadius: BorderRadius.circular(6),
                        child: Padding(
                          padding: const EdgeInsets.all(4),
                          child: Icon(
                            Icons.picture_in_picture_rounded,
                            size: 15,
                            color: p.muted,
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}

/// 6. Sleek Persistent Bottom Playback Bar with Clickable Live Visualizer Scrubber.
class CiderBottomBar extends StatelessWidget {
  const CiderBottomBar({
    super.key,
    required this.core,
    required this.onAddPlaylist,
  });

  final Core core;
  final ValueChanged<Json> onAddPlaylist;

  @override
  Widget build(BuildContext context) {
    final p = context.paints;
    return ValueListenableBuilder<Json>(
      valueListenable: core.playback,
      builder: (context, pb, _) {
        final track = object(pb['track']);
        final hasTrack = track.isNotEmpty;
        final playing = pb['playing'] == true;
        final pos = (pb['position'] as num?)?.toDouble() ?? 0;
        final dur = (pb['duration'] as num?)?.toDouble() ?? 0;
        final vol = (pb['volume'] as num?)?.toDouble() ?? .8;
        final frac = dur > 0 ? (pos / dur).clamp(0.0, 1.0) : 0.0;
        final liked = hasTrack && core.isLiked(track);
        final tone = ambientToneFor(trackKey(track));

        return Container(
          height: 68,
          padding: const EdgeInsets.symmetric(horizontal: 14),
          decoration: BoxDecoration(
            color: const Color(0xff131316),
            border: Border(
              top: BorderSide(color: p.ink.withValues(alpha: .07)),
            ),
          ),
          child: Row(
            children: [
              // Left Track Block (click to open Immersive Stage!)
              SizedBox(
                width: 240,
                child: hasTrack
                    ? InkWell(
                        onTap: redesignPrefs.toggleImmersive,
                        borderRadius: BorderRadius.circular(10),
                        child: Padding(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 4,
                            vertical: 4,
                          ),
                          child: Row(
                            children: [
                              Artwork(trackCover(track), size: 44, radius: 8),
                              const SizedBox(width: 10),
                              Expanded(
                                child: Column(
                                  mainAxisAlignment: MainAxisAlignment.center,
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    Text(
                                      trackTitle(track, context),
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: TextStyle(
                                        fontSize: 12.5,
                                        fontWeight: FontWeight.w600,
                                        color: p.ink,
                                      ),
                                    ),
                                    const SizedBox(height: 2),
                                    Text(
                                      trackArtist(track, context),
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: TextStyle(
                                        fontSize: 11,
                                        color: p.muted,
                                      ),
                                    ),
                                  ],
                                ),
                              ),
                              IconButton(
                                tooltip: liked
                                    ? 'Remove from Favorites'
                                    : 'Add to Favorites',
                                icon: Icon(
                                  liked
                                      ? Icons.star_rounded
                                      : Icons.star_border_rounded,
                                  size: 18,
                                  color: liked ? Colors.white : p.muted,
                                ),
                                onPressed: () => core.action('like', track),
                              ),
                            ],
                          ),
                        ),
                      )
                    : Row(
                        children: [
                          Container(
                            width: 42,
                            height: 42,
                            decoration: BoxDecoration(
                              color: p.ink.withValues(alpha: .06),
                              borderRadius: BorderRadius.circular(8),
                            ),
                            child: Icon(
                              Icons.music_note_rounded,
                              size: 18,
                              color: p.muted,
                            ),
                          ),
                          const SizedBox(width: 10),
                          Text(
                            'Nothing playing',
                            style: TextStyle(fontSize: 12, color: p.muted),
                          ),
                        ],
                      ),
              ),

              // Center Transport + Live Clickable Visualizer Scrubber
              Expanded(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Row(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        IconButton(
                          tooltip: 'Shuffle',
                          visualDensity: VisualDensity.compact,
                          iconSize: 16,
                          color: core.settings['shuffle'] == true
                              ? Colors.white
                              : p.muted,
                          icon: const Icon(Icons.shuffle_rounded),
                          onPressed: () => core.player('shuffle'),
                        ),
                        IconButton(
                          tooltip: 'Previous',
                          visualDensity: VisualDensity.compact,
                          iconSize: 20,
                          color: p.ink,
                          icon: const Icon(Icons.skip_previous_rounded),
                          onPressed: hasTrack
                              ? () => core.player('previous')
                              : null,
                        ),
                        const SizedBox(width: 4),
                        Material(
                          color: Colors.white,
                          shape: const CircleBorder(),
                          child: InkWell(
                            onTap: hasTrack
                                ? () => core.player('toggle')
                                : null,
                            customBorder: const CircleBorder(),
                            child: SizedBox(
                              width: 32,
                              height: 32,
                              child: Icon(
                                playing
                                    ? Icons.pause_rounded
                                    : Icons.play_arrow_rounded,
                                size: 20,
                                color: const Color(0xff111113),
                              ),
                            ),
                          ),
                        ),
                        const SizedBox(width: 4),
                        IconButton(
                          tooltip: 'Next',
                          visualDensity: VisualDensity.compact,
                          iconSize: 20,
                          color: p.ink,
                          icon: const Icon(Icons.skip_next_rounded),
                          onPressed: hasTrack
                              ? () => core.player('next')
                              : null,
                        ),
                        IconButton(
                          tooltip: 'Repeat',
                          visualDensity: VisualDensity.compact,
                          iconSize: 16,
                          color: (core.settings['repeat'] ?? 'off') != 'off'
                              ? Colors.white
                              : p.muted,
                          icon: Icon(
                            core.settings['repeat'] == 'one'
                                ? Icons.repeat_one_rounded
                                : Icons.repeat_rounded,
                          ),
                          onPressed: () => core.player('repeat'),
                        ),
                      ],
                    ),
                    Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 20),
                      child: Row(
                        children: [
                          Text(
                            clock(pos),
                            style: TextStyle(
                              fontSize: 10,
                              color: p.muted,
                              fontFeatures: const [
                                ui.FontFeature.tabularFigures(),
                              ],
                            ),
                          ),
                          const SizedBox(width: 8),
                          Expanded(
                            child: SoundFlowVisualizer(
                              playing: playing,
                              progress: frac,
                              volume: vol,
                              mode: redesignPrefs.vizMode,
                              accent: Color.lerp(Colors.white, tone, .35)!,
                              muted: p.muted,
                              height: 18,
                              seed: trackKey(track).hashCode,
                              onSeekFraction: dur > 0
                                  ? (f) => core.command({
                                      'type': 'seek',
                                      'value': f * dur,
                                    })
                                  : null,
                            ),
                          ),
                          const SizedBox(width: 8),
                          Text(
                            _remainingClock(pos, dur),
                            style: TextStyle(
                              fontSize: 10,
                              color: p.muted,
                              fontFeatures: const [
                                ui.FontFeature.tabularFigures(),
                              ],
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),

              // Right Volume + Stage / Mini-Player / Island Controls
              SizedBox(
                width: 240,
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: [
                    IconButton(
                      tooltip: 'Add to Playlist',
                      visualDensity: VisualDensity.compact,
                      iconSize: 17,
                      color: p.muted,
                      icon: const Icon(Icons.playlist_add_rounded),
                      onPressed: hasTrack ? () => onAddPlaylist(track) : null,
                    ),
                    IconButton(
                      tooltip: 'Immersive Stage',
                      visualDensity: VisualDensity.compact,
                      iconSize: 17,
                      color: redesignPrefs.immersiveOpen
                          ? Colors.white
                          : p.muted,
                      icon: const Icon(Icons.format_quote_rounded),
                      onPressed: redesignPrefs.toggleImmersive,
                    ),
                    IconButton(
                      tooltip: 'Mute',
                      visualDensity: VisualDensity.compact,
                      iconSize: 17,
                      color: p.muted,
                      icon: Icon(
                        vol <= 0.001
                            ? Icons.volume_off_rounded
                            : Icons.volume_up_rounded,
                      ),
                      onPressed: () => core.player('mute'),
                    ),
                    SizedBox(
                      width: 82,
                      child: MediaSlider(
                        value: vol,
                        continuous: true,
                        onChanged: core.setVolume,
                      ),
                    ),
                    const SizedBox(width: 4),
                    IconButton(
                      tooltip: 'Dynamic Island',
                      visualDensity: VisualDensity.compact,
                      iconSize: 17,
                      color: p.muted,
                      icon: const Icon(Icons.picture_in_picture_rounded),
                      onPressed: core.island,
                    ),
                  ],
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}

/// 7. Immersive Full-Stage Lyrics / Focus Queue + Big Visualizer Window
/// (Reproduces the iconic lower-center ambient tinted panel in the screenshot!).
class CiderImmersiveOverlay extends StatelessWidget {
  const CiderImmersiveOverlay({super.key, required this.core});

  final Core core;

  @override
  Widget build(BuildContext context) {
    final p = context.paints;
    return ValueListenableBuilder<Json>(
      valueListenable: core.playback,
      builder: (context, pb, _) {
        final track = object(pb['track']);
        final hasTrack = track.isNotEmpty;
        final playing = pb['playing'] == true;
        final pos = (pb['position'] as num?)?.toDouble() ?? 0;
        final dur = (pb['duration'] as num?)?.toDouble() ?? 0;
        final vol = (pb['volume'] as num?)?.toDouble() ?? .8;
        final frac = dur > 0 ? (pos / dur).clamp(0.0, 1.0) : 0.0;
        final tone = ambientToneFor(trackKey(track));
        final queue = core.queue;
        final curIdx = queue.isEmpty
            ? 0
            : core.queueIndex.clamp(0, queue.length - 1);
        final focusLines = <String>[
          if (hasTrack) trackTitle(track, context),
          if (hasTrack) trackArtist(track, context),
          for (var i = curIdx + 1; i < queue.length && i < curIdx + 6; i++)
            trackTitle(queue[i], context),
          if (!hasTrack) 'Play any song to enter Immersive Stage',
        ];

        return Positioned.fill(
          child: Container(
            margin: const EdgeInsets.all(14),
            decoration: BoxDecoration(
              color: const Color(0xff141418),
              borderRadius: BorderRadius.circular(18),
              gradient: RadialGradient(
                center: const Alignment(-0.25, -0.15),
                radius: 1.35,
                colors: [
                  Color.lerp(tone, const Color(0xff141418), .32)!,
                  Color.lerp(tone, const Color(0xff141418), .74)!,
                  const Color(0xff141418),
                ],
                stops: const [0.0, 0.55, 1.0],
              ),
              border: Border.all(color: Colors.white.withValues(alpha: .12)),
              boxShadow: [
                BoxShadow(
                  color: Colors.black.withValues(alpha: .72),
                  blurRadius: 48,
                  offset: const Offset(0, 20),
                ),
              ],
            ),
            child: Column(
              children: [
                // Top Strip inside Immersive Stage
                Padding(
                  padding: const EdgeInsets.fromLTRB(22, 16, 18, 8),
                  child: Row(
                    children: [
                      Container(
                        padding: const EdgeInsets.all(3),
                        decoration: BoxDecoration(
                          color: Colors.black.withValues(alpha: .28),
                          borderRadius: BorderRadius.circular(10),
                        ),
                        child: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            _tabBtn(
                              'Focus View',
                              !redesignPrefs.immersiveVisualizerTab,
                              () =>
                                  redesignPrefs.setImmersiveVisualizerTab(false),
                            ),
                            _tabBtn(
                              'Live Visualizer',
                              redesignPrefs.immersiveVisualizerTab,
                              () =>
                                  redesignPrefs.setImmersiveVisualizerTab(true),
                            ),
                          ],
                        ),
                      ),
                      const Spacer(),
                      Material(
                        color: Colors.white.withValues(alpha: .12),
                        borderRadius: BorderRadius.circular(16),
                        child: InkWell(
                          onTap: () => redesignPrefs.setImmersive(false),
                          borderRadius: BorderRadius.circular(16),
                          child: const Padding(
                            padding: EdgeInsets.symmetric(
                              horizontal: 14,
                              vertical: 6,
                            ),
                            child: Row(
                              mainAxisSize: MainAxisSize.min,
                              children: [
                                Icon(
                                  Icons.close_rounded,
                                  size: 15,
                                  color: Colors.white,
                                ),
                                SizedBox(width: 5),
                                Text(
                                  'Close (Esc)',
                                  style: TextStyle(
                                    fontSize: 12,
                                    fontWeight: FontWeight.w600,
                                    color: Colors.white,
                                  ),
                                ),
                              ],
                            ),
                          ),
                        ),
                      ),
                    ],
                  ),
                ),

                // Main Split Stage (Left Big Square Art + Scrubber | Right Focus Typography or Visualizer)
                Expanded(
                  child: Padding(
                    padding: const EdgeInsets.fromLTRB(44, 10, 44, 34),
                    child: Row(
                      children: [
                        // Left Column: Big Cover + Hairline Scrubber + Centered Title/Artist
                        SizedBox(
                          width: 310,
                          child: Column(
                            mainAxisAlignment: MainAxisAlignment.center,
                            children: [
                              Container(
                                decoration: BoxDecoration(
                                  borderRadius: BorderRadius.circular(14),
                                  boxShadow: [
                                    BoxShadow(
                                      color: Colors.black.withValues(
                                        alpha: .52,
                                      ),
                                      blurRadius: 34,
                                      offset: const Offset(0, 16),
                                    ),
                                  ],
                                ),
                                child: Artwork(
                                  trackCover(track),
                                  size: 274,
                                  radius: 14,
                                ),
                              ),
                              const SizedBox(height: 24),
                              Row(
                                mainAxisAlignment:
                                    MainAxisAlignment.spaceBetween,
                                children: [
                                  Text(
                                    clock(pos),
                                    style: const TextStyle(
                                      fontSize: 11,
                                      color: Color(0xb8ffffff),
                                      fontFeatures: [
                                        ui.FontFeature.tabularFigures(),
                                      ],
                                    ),
                                  ),
                                  Text(
                                    _remainingClock(pos, dur),
                                    style: const TextStyle(
                                      fontSize: 11,
                                      color: Color(0xb8ffffff),
                                      fontFeatures: [
                                        ui.FontFeature.tabularFigures(),
                                      ],
                                    ),
                                  ),
                                ],
                              ),
                              const SizedBox(height: 6),
                              MediaSlider(
                                value: frac,
                                onChanged: dur > 0
                                    ? (v) => core.command({
                                        'type': 'seek',
                                        'value': v * dur,
                                      })
                                    : null,
                              ),
                              const SizedBox(height: 16),
                              Text(
                                hasTrack
                                    ? trackTitle(track, context).toUpperCase()
                                    : 'SOUNDFLOW',
                                textAlign: TextAlign.center,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: const TextStyle(
                                  fontFamily: SoundFlowType.heading,
                                  fontSize: 15,
                                  fontWeight: FontWeight.w700,
                                  color: Colors.white,
                                ),
                              ),
                              const SizedBox(height: 4),
                              Text(
                                hasTrack
                                    ? '${trackArtist(track, context)} · ${sourceName(track, context)}'
                                    : 'Select any track',
                                textAlign: TextAlign.center,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: const TextStyle(
                                  fontSize: 12,
                                  color: Color(0xa6ffffff),
                                ),
                              ),
                              const SizedBox(height: 18),
                              Row(
                                mainAxisAlignment: MainAxisAlignment.center,
                                children: [
                                  IconButton(
                                    onPressed: () => core.player('previous'),
                                    icon: const Icon(
                                      Icons.skip_previous_rounded,
                                      color: Colors.white,
                                    ),
                                  ),
                                  const SizedBox(width: 8),
                                  Material(
                                    color: Colors.white,
                                    shape: const CircleBorder(),
                                    child: InkWell(
                                      onTap: () => core.player('toggle'),
                                      customBorder: const CircleBorder(),
                                      child: SizedBox(
                                        width: 46,
                                        height: 46,
                                        child: Icon(
                                          playing
                                              ? Icons.pause_rounded
                                              : Icons.play_arrow_rounded,
                                          size: 26,
                                          color: const Color(0xff111113),
                                        ),
                                      ),
                                    ),
                                  ),
                                  const SizedBox(width: 8),
                                  IconButton(
                                    onPressed: () => core.player('next'),
                                    icon: const Icon(
                                      Icons.skip_next_rounded,
                                      color: Colors.white,
                                    ),
                                  ),
                                ],
                              ),
                            ],
                          ),
                        ),

                        const SizedBox(width: 56),

                        // Right Column: Iconic Progressive Dimmed Typography OR Big Live Visualizer
                        Expanded(
                          child: redesignPrefs.immersiveVisualizerTab
                              ? Column(
                                  mainAxisAlignment: MainAxisAlignment.center,
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    Text(
                                      'Live Audio Spectrum',
                                      style: TextStyle(
                                        fontSize: 13,
                                        fontWeight: FontWeight.w600,
                                        color: Colors.white.withValues(
                                          alpha: .7,
                                        ),
                                      ),
                                    ),
                                    const SizedBox(height: 18),
                                    SoundFlowVisualizer(
                                      playing: playing,
                                      progress: frac,
                                      volume: vol,
                                      mode: redesignPrefs.vizMode == 'off'
                                          ? 'bars'
                                          : redesignPrefs.vizMode,
                                      accent: Colors.white,
                                      muted: Colors.white.withValues(
                                        alpha: .45,
                                      ),
                                      height: 140,
                                      seed: trackKey(track).hashCode,
                                      onSeekFraction: dur > 0
                                          ? (f) => core.command({
                                              'type': 'seek',
                                              'value': f * dur,
                                            })
                                          : null,
                                    ),
                                    const SizedBox(height: 22),
                                    Row(
                                      children: [
                                        for (final m in const [
                                          ('bars', 'Bars'),
                                          ('wave', 'Wave'),
                                          ('mirror', 'Mirror'),
                                        ])
                                          Padding(
                                            padding: const EdgeInsets.only(
                                              right: 8,
                                            ),
                                            child: _tabBtn(
                                              m.$2,
                                              redesignPrefs.vizMode == m.$1,
                                              () =>
                                                  redesignPrefs.setVizMode(m.$1),
                                            ),
                                          ),
                                      ],
                                    ),
                                  ],
                                )
                              : Column(
                                  mainAxisAlignment: MainAxisAlignment.center,
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    for (
                                      var i = 0;
                                      i < focusLines.length;
                                      i++
                                    )
                                      Padding(
                                        padding: const EdgeInsets.symmetric(
                                          vertical: 10,
                                        ),
                                        child: Text(
                                          focusLines[i],
                                          maxLines: 2,
                                          overflow: TextOverflow.ellipsis,
                                          style: TextStyle(
                                            fontFamily: SoundFlowType.heading,
                                            fontSize: i == 0 ? 32 : 26,
                                            height: 1.18,
                                            letterSpacing: -0.6,
                                            fontWeight: FontWeight.w700,
                                            color: Colors.white.withValues(
                                              alpha: switch (i) {
                                                0 => 1.0,
                                                1 => 0.56,
                                                2 => 0.36,
                                                3 => 0.24,
                                                _ => 0.14,
                                              },
                                            ),
                                          ),
                                        ),
                                      ),
                                    const SizedBox(height: 24),
                                    SizedBox(
                                      width: 260,
                                      child: SoundFlowVisualizer(
                                        playing: playing,
                                        progress: frac,
                                        volume: vol,
                                        mode: redesignPrefs.vizMode,
                                        accent: Colors.white.withValues(
                                          alpha: .88,
                                        ),
                                        muted: p.muted,
                                        height: 26,
                                        seed: trackKey(track).hashCode,
                                      ),
                                    ),
                                  ],
                                ),
                        ),
                      ],
                    ),
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }

  Widget _tabBtn(String label, bool active, VoidCallback onTap) => Material(
    color: active ? Colors.white.withValues(alpha: .18) : Colors.transparent,
    borderRadius: BorderRadius.circular(8),
    child: InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(8),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
        child: Text(
          label,
          style: TextStyle(
            fontSize: 12,
            fontWeight: FontWeight.w600,
            color: active ? Colors.white : Colors.white.withValues(alpha: .65),
          ),
        ),
      ),
    ),
  );
}

/// 8. Floating Square Mini-Player Card (reproduces the bottom-left card in the screenshot!).
class CiderMiniPlayerCard extends StatelessWidget {
  const CiderMiniPlayerCard({
    super.key,
    required this.core,
    required this.onAddPlaylist,
  });

  final Core core;
  final ValueChanged<Json> onAddPlaylist;

  @override
  Widget build(BuildContext context) {
    final p = context.paints;
    return ValueListenableBuilder<Json>(
      valueListenable: core.playback,
      builder: (context, pb, _) {
        final track = object(pb['track']);
        if (track.isEmpty) return const SizedBox.shrink();
        final playing = pb['playing'] == true;
        final pos = (pb['position'] as num?)?.toDouble() ?? 0;
        final dur = (pb['duration'] as num?)?.toDouble() ?? 0;
        final frac = dur > 0 ? (pos / dur).clamp(0.0, 1.0) : 0.0;
        final liked = core.isLiked(track);

        return Positioned(
          left: 16,
          bottom: 80,
          child: Container(
            width: 244,
            decoration: BoxDecoration(
              color: const Color(0xff141418),
              borderRadius: BorderRadius.circular(14),
              border: Border.all(color: Colors.white.withValues(alpha: .12)),
              boxShadow: [
                BoxShadow(
                  color: Colors.black.withValues(alpha: .65),
                  blurRadius: 30,
                  offset: const Offset(0, 12),
                ),
              ],
            ),
            clipBehavior: Clip.antiAlias,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                // Full-bleed top cover with top-right overlay buttons
                SizedBox(
                  height: 204,
                  width: 244,
                  child: Stack(
                    fit: StackFit.expand,
                    children: [
                      Artwork(trackCover(track), radius: 0),
                      const DecoratedBox(
                        decoration: BoxDecoration(
                          gradient: LinearGradient(
                            begin: Alignment.topCenter,
                            end: Alignment.bottomCenter,
                            colors: [
                              Color(0x55000000),
                              Colors.transparent,
                              Color(0xe6141418),
                            ],
                            stops: [0.0, 0.4, 1.0],
                          ),
                        ),
                      ),
                      Positioned(
                        top: 8,
                        right: 8,
                        child: Row(
                          children: [
                            _miniPillBtn(
                              Icons.volume_up_rounded,
                              () => core.player('mute'),
                            ),
                            const SizedBox(width: 5),
                            _miniPillBtn(
                              Icons.close_rounded,
                              redesignPrefs.toggleMiniPlayer,
                              tint: const Color(0xffd9415e),
                            ),
                          ],
                        ),
                      ),
                      Positioned(
                        left: 12,
                        right: 12,
                        bottom: 8,
                        child: Row(
                          children: [
                            Expanded(
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Text(
                                    trackTitle(track, context).toUpperCase(),
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                    style: const TextStyle(
                                      fontFamily: SoundFlowType.heading,
                                      fontSize: 12.5,
                                      fontWeight: FontWeight.w700,
                                      color: Colors.white,
                                    ),
                                  ),
                                  Text(
                                    trackArtist(track, context),
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                    style: TextStyle(
                                      fontSize: 10.5,
                                      color: p.muted,
                                    ),
                                  ),
                                ],
                              ),
                            ),
                            InkWell(
                              onTap: () => core.action('like', track),
                              child: Icon(
                                liked
                                    ? Icons.star_rounded
                                    : Icons.star_border_rounded,
                                size: 17,
                                color: Colors.white,
                              ),
                            ),
                            const SizedBox(width: 8),
                            InkWell(
                              onTap: () => onAddPlaylist(track),
                              child: const Icon(
                                Icons.add_rounded,
                                size: 18,
                                color: Colors.white,
                              ),
                            ),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),

                // Scrubber + Transport Row
                Padding(
                  padding: const EdgeInsets.fromLTRB(12, 6, 12, 10),
                  child: Column(
                    children: [
                      ClipRRect(
                        borderRadius: BorderRadius.circular(2),
                        child: LinearProgressIndicator(
                          value: frac,
                          minHeight: 3,
                          backgroundColor: Colors.white.withValues(alpha: .15),
                          valueColor: const AlwaysStoppedAnimation<Color>(
                            Colors.white,
                          ),
                        ),
                      ),
                      const SizedBox(height: 5),
                      Row(
                        mainAxisAlignment: MainAxisAlignment.spaceBetween,
                        children: [
                          Text(
                            clock(pos),
                            style: TextStyle(
                              fontSize: 10,
                              color: p.muted,
                              fontFeatures: const [
                                ui.FontFeature.tabularFigures(),
                              ],
                            ),
                          ),
                          Text(
                            _remainingClock(pos, dur),
                            style: TextStyle(
                              fontSize: 10,
                              color: p.muted,
                              fontFeatures: const [
                                ui.FontFeature.tabularFigures(),
                              ],
                            ),
                          ),
                        ],
                      ),
                      const SizedBox(height: 6),
                      Row(
                        mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                        children: [
                          IconButton(
                            visualDensity: VisualDensity.compact,
                            iconSize: 19,
                            color: Colors.white,
                            onPressed: () => core.player('previous'),
                            icon: const Icon(Icons.fast_rewind_rounded),
                          ),
                          IconButton(
                            visualDensity: VisualDensity.compact,
                            iconSize: 26,
                            color: Colors.white,
                            onPressed: () => core.player('toggle'),
                            icon: Icon(
                              playing
                                  ? Icons.pause_rounded
                                  : Icons.play_arrow_rounded,
                            ),
                          ),
                          IconButton(
                            visualDensity: VisualDensity.compact,
                            iconSize: 19,
                            color: Colors.white,
                            onPressed: () => core.player('next'),
                            icon: const Icon(Icons.fast_forward_rounded),
                          ),
                          IconButton(
                            visualDensity: VisualDensity.compact,
                            iconSize: 16,
                            color: (core.settings['repeat'] ?? 'off') != 'off'
                                ? p.accent
                                : p.muted,
                            onPressed: () => core.player('repeat'),
                            icon: Icon(
                              core.settings['repeat'] == 'one'
                                  ? Icons.repeat_one_rounded
                                  : Icons.repeat_rounded,
                            ),
                          ),
                        ],
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }

  Widget _miniPillBtn(IconData icon, VoidCallback onTap, {Color? tint}) =>
      Material(
        color: tint ?? Colors.black.withValues(alpha: .55),
        shape: const CircleBorder(),
        child: InkWell(
          onTap: onTap,
          customBorder: const CircleBorder(),
          child: SizedBox(
            width: 24,
            height: 24,
            child: Icon(icon, size: 13, color: Colors.white),
          ),
        ),
      );
}
