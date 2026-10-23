import 'dart:io';
import 'dart:async';

import 'package:flutter/material.dart';

import 'anicry_motion.dart';

import 'core.dart';
import 'concentric.dart';
import 'media_slider.dart';

class Paints extends ThemeExtension<Paints> {
  const Paints(
    this.canvas,
    this.surface,
    this.hover,
    this.ink,
    this.muted,
    this.accent,
  );
  final Color canvas, surface, hover, ink, muted, accent;
  static const night = Paints(
    Color(0xff181818),
    Color(0xff242424),
    Color(0xff333333),
    Color(0xfff5f5f7),
    Color(0xffb8b8b8),
    Color(0xff9bbcff),
  );
  static const day = Paints(
    Color(0xfff5f5f7),
    Colors.white,
    Color(0xffe8e8ee),
    Color(0xff191a20),
    Color(0xff60616b),
    Color(0xff285bbf),
  );
  static Paints forScheme(String scheme) => switch (scheme) {
    'light' => day,
    'rose' => const Paints(
      Color(0xff1f141b),
      Color(0xff30212b),
      Color(0xff422d3c),
      Color(0xfffff1f8),
      Color(0xffd1b5c8),
      Color(0xfff5adc9),
    ),
    'violet' => const Paints(
      Color(0xff1b1625),
      Color(0xff2b2338),
      Color(0xff3b304b),
      Color(0xfff4efff),
      Color(0xffc0b4d6),
      Color(0xffc6b2ff),
    ),
    'ocean' => const Paints(
      Color(0xff111e27),
      Color(0xff1d303c),
      Color(0xff2b4352),
      Color(0xffedf8ff),
      Color(0xffadc9d9),
      Color(0xff8fcfff),
    ),
    _ => night,
  };
  @override
  Paints copyWith({
    Color? canvas,
    Color? surface,
    Color? hover,
    Color? ink,
    Color? muted,
    Color? accent,
  }) => Paints(
    canvas ?? this.canvas,
    surface ?? this.surface,
    hover ?? this.hover,
    ink ?? this.ink,
    muted ?? this.muted,
    accent ?? this.accent,
  );
  @override
  Paints lerp(covariant Paints? other, double t) => other == null
      ? this
      : Paints(
          Color.lerp(canvas, other.canvas, t)!,
          Color.lerp(surface, other.surface, t)!,
          Color.lerp(hover, other.hover, t)!,
          Color.lerp(ink, other.ink, t)!,
          Color.lerp(muted, other.muted, t)!,
          Color.lerp(accent, other.accent, t)!,
        );
}

extension Design on BuildContext {
  Paints get paints => Theme.of(this).extension<Paints>()!;
}

ThemeData soundflowTheme(bool dark, bool reduced, {String? scheme}) {
  final p = Paints.forScheme(scheme ?? (dark ? 'dark' : 'light'));
  return ThemeData(
    brightness: dark ? Brightness.dark : Brightness.light,
    fontFamily: 'Segoe UI',
    scaffoldBackgroundColor: p.canvas,
    colorScheme: ColorScheme.fromSeed(
      seedColor: p.accent,
      brightness: dark ? Brightness.dark : Brightness.light,
      surface: p.surface,
    ),
    extensions: [p],
    splashFactory: NoSplash.splashFactory,
    highlightColor: Colors.transparent,
    hoverColor: Colors.transparent,
    focusColor: Colors.transparent,
    textTheme: TextTheme(
      bodyMedium: TextStyle(fontSize: 14, color: p.ink),
      bodySmall: TextStyle(fontSize: 13, color: p.muted),
      titleLarge: TextStyle(
        fontSize: 22,
        fontWeight: FontWeight.w600,
        color: p.ink,
      ),
    ),
    tooltipTheme: TooltipThemeData(
      waitDuration: const Duration(milliseconds: 550),
      decoration: BoxDecoration(
        color: p.hover,
        borderRadius: BorderRadius.circular(7),
      ),
      textStyle: TextStyle(fontSize: 12, color: p.ink),
    ),
    sliderTheme: SliderThemeData(
      trackHeight: 3,
      activeTrackColor: p.ink,
      inactiveTrackColor: dark
          ? const Color(0xff646671)
          : const Color(0xff858692),
      thumbColor: p.ink,
      overlayShape: SliderComponentShape.noOverlay,
      thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 4),
      trackShape: const RoundedRectSliderTrackShape(),
      padding: const EdgeInsets.symmetric(horizontal: 4),
    ),
    dialogTheme: DialogThemeData(
      backgroundColor: p.surface,
      surfaceTintColor: Colors.transparent,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(24)),
    ),
    inputDecorationTheme: InputDecorationTheme(
      filled: true,
      fillColor: p.surface,
      contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 13),
      border: OutlineInputBorder(
        borderSide: BorderSide.none,
        borderRadius: BorderRadius.circular(12),
      ),
      focusedBorder: OutlineInputBorder(
        borderSide: BorderSide(color: p.accent, width: 2),
        borderRadius: BorderRadius.circular(12),
      ),
      hintStyle: TextStyle(color: p.muted, fontSize: 14),
    ),
  );
}

class Glyph extends StatelessWidget {
  const Glyph(
    this.name, {
    super.key,
    this.size = 20,
    this.color,
    this.filled = false,
  });
  final String name;
  final double size;
  final Color? color;
  final bool filled;
  @override
  Widget build(BuildContext context) => AnicryGlyph(
    name == 'more' ? 'ellipsis' : name,
    size: size,
    color: color ?? context.paints.ink,
    filled: filled,
  );
}

class Control extends StatelessWidget {
  const Control(
    this.icon,
    this.label, {
    super.key,
    required this.onTap,
    this.active = false,
    this.primary = false,
    this.size = 36,
    this.glyphSize,
    this.color,
    this.surface = false,
  });
  final String icon, label;
  final VoidCallback? onTap;
  final bool active, primary, surface;
  final double size;
  final double? glyphSize;
  final Color? color;
  @override
  Widget build(BuildContext context) {
    final p = context.paints;
    return Tooltip(
      message: label,
      child: Semantics(
        button: true,
        label: label,
        enabled: onTap != null,
        selected: active,
        child: Material(
          color: primary
              ? p.ink
              : active || surface
              ? p.surface
              : Colors.transparent,
          shape: const CircleBorder(),
          child: AnicryTap(
            onTap: onTap,
            icon: icon,
            circle: true,
            focusColor: p.accent,
            hoverColor: primary ? p.muted : p.hover,
            child: SizedBox(
              width: size,
              height: size,
              child: Center(
                child: Opacity(
                  opacity: onTap == null ? 0.35 : 1,
                  child: AnimatedSwitcher(
                    duration: MediaQuery.disableAnimationsOf(context)
                        ? Duration.zero
                        : const Duration(milliseconds: 180),
                    child: Glyph(
                      icon,
                      key: ValueKey(icon),
                      filled: const {
                        'play',
                        'pause',
                        'skip-back',
                        'skip-forward',
                      }.contains(icon),
                      size: glyphSize ?? (size >= 52 ? 26 : 20),
                      color:
                          color ??
                          (primary
                              ? p.canvas
                              : active
                              ? p.accent
                              : p.ink),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class ActionButton extends StatelessWidget {
  const ActionButton(
    this.label, {
    super.key,
    required this.onTap,
    this.primary = false,
    this.flat = false,
    this.compact = false,
    this.icon,
  });
  final String label;
  final VoidCallback? onTap;
  final bool primary, flat, compact;
  final String? icon;
  @override
  Widget build(BuildContext context) {
    final p = context.paints;
    final foreground = primary ? p.canvas : p.ink;
    final radius = compact
        ? SoundFlowCorners.compactControl
        : SoundFlowCorners.control;
    return Material(
      color: primary
          ? p.ink
          : flat
          ? Colors.transparent
          : p.surface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(radius),
        side: BorderSide(
          color: flat || primary
              ? Colors.transparent
              : p.ink.withValues(alpha: .07),
        ),
      ),
      child: AnicryTap(
        onTap: onTap,
        icon: icon,
        radius: radius,
        child: SizedBox(
          height: compact ? 32 : 40,
          child: Padding(
            padding: EdgeInsets.symmetric(horizontal: compact ? 12 : 16),
            child: Opacity(
              opacity: onTap == null ? .4 : 1,
              child: Row(
                mainAxisSize: MainAxisSize.min,
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  if (icon != null) ...[
                    Glyph(icon!, size: 18, color: foreground),
                    const SizedBox(width: 8),
                  ],
                  Text(
                    label,
                    style: TextStyle(
                      fontSize: compact ? 13 : 14,
                      fontWeight: FontWeight.w600,
                      color: foreground,
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class Artwork extends StatelessWidget {
  const Artwork(
    this.path, {
    super.key,
    this.size,
    this.radius = 12,
    this.corners,
  });
  final String path;
  final double? size;
  final double radius;
  final BorderRadius? corners;
  Widget fallback(BuildContext c) => ColoredBox(
    color: c.paints.surface,
    child: Center(child: Glyph('book-open', size: 28, color: c.paints.muted)),
  );
  @override
  Widget build(BuildContext context) {
    Widget art;
    if (path.isEmpty) {
      art = fallback(context);
    } else if (path.startsWith('http://') || path.startsWith('https://')) {
      art = Image.network(
        path,
        fit: BoxFit.cover,
        cacheWidth: 700,
        gaplessPlayback: true,
        errorBuilder: (c, e, s) => fallback(c),
      );
    } else {
      final p = path.startsWith('file:')
          ? Uri.parse(path).toFilePath()
          : path.startsWith('cover:')
          ? '${File(Platform.resolvedExecutable).parent.path}/resources/covers/${path.substring(6)}'
          : path;
      art = Image.file(
        File(p),
        fit: BoxFit.cover,
        cacheWidth: 700,
        gaplessPlayback: true,
        errorBuilder: (c, e, s) => fallback(c),
      );
    }
    return RepaintBoundary(
      child: ClipRRect(
        borderRadius: corners ?? BorderRadius.circular(radius),
        child: SizedBox(width: size, height: size, child: art),
      ),
    );
  }
}

class Heading extends StatelessWidget {
  const Heading(this.text, {super.key, this.action});
  final String text;
  final Widget? action;
  @override
  Widget build(BuildContext context) => Row(
    children: [
      Expanded(
        child: Text(
          text,
          style: TextStyle(
            fontSize: 20,
            fontWeight: FontWeight.w600,
            color: context.paints.ink,
          ),
        ),
      ),
      ?action,
    ],
  );
}

class EmptyState extends StatelessWidget {
  const EmptyState(this.title, this.description, {super.key, this.action});
  final String title, description;
  final Widget? action;
  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.symmetric(vertical: 38),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          title,
          style: const TextStyle(fontSize: 26, fontWeight: FontWeight.w600),
        ),
        const SizedBox(height: 10),
        Text(
          description,
          style: TextStyle(color: context.paints.muted, fontSize: 15),
        ),
        if (action != null) ...[const SizedBox(height: 24), action!],
      ],
    ),
  );
}

class AlbumCard extends StatefulWidget {
  const AlbumCard(
    this.track, {
    super.key,
    required this.onTap,
    this.playing = false,
    this.reduced = false,
  });
  final Json track;
  final VoidCallback onTap;
  final bool playing, reduced;
  @override
  State<AlbumCard> createState() => _AlbumCardState();
}

class _AlbumCardState extends State<AlbumCard> {
  bool hover = false;
  @override
  Widget build(BuildContext context) => MouseRegion(
    onEnter: (_) => setState(() => hover = true),
    onExit: (_) => setState(() => hover = false),
    child: Semantics(
      button: true,
      label:
          '${widget.playing ? 'Пауза' : 'Відтворити'}: ${trackTitle(widget.track)}, ${trackArtist(widget.track)}',
      child: ClipRRect(
        borderRadius: BorderRadius.circular(22),
        child: Material(
          color: context.paints.surface,
          child: InkWell(
            onTap: widget.onTap,
            child: Stack(
              fit: StackFit.expand,
              children: [
                AnimatedScale(
                  scale: hover ? 1.035 : 1,
                  duration: Duration(milliseconds: widget.reduced ? 0 : 300),
                  curve: Curves.easeOutCubic,
                  child: Artwork(trackCover(widget.track), radius: 0),
                ),
                const DecoratedBox(
                  decoration: BoxDecoration(
                    gradient: LinearGradient(
                      begin: Alignment(0, -.4),
                      end: Alignment.bottomCenter,
                      colors: [
                        Colors.transparent,
                        Color(0xb3000000),
                        Color(0xf2000000),
                      ],
                      stops: [0, .35, 1],
                    ),
                  ),
                ),
                Positioned(
                  left: 20,
                  right: 20,
                  bottom: 20,
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.end,
                    children: [
                      Expanded(
                        child: Column(
                          mainAxisSize: MainAxisSize.min,
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              trackTitle(widget.track),
                              maxLines: 2,
                              overflow: TextOverflow.ellipsis,
                              style: const TextStyle(
                                fontSize: 20,
                                height: 1.2,
                                fontWeight: FontWeight.w600,
                                color: Colors.white,
                              ),
                            ),
                            const SizedBox(height: 8),
                            Text(
                              trackArtist(widget.track),
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: const TextStyle(
                                fontSize: 13,
                                height: 1.3,
                                color: Color(0xffdddddf),
                              ),
                            ),
                          ],
                        ),
                      ),
                      const SizedBox(width: 12),
                      Container(
                        width: 38,
                        height: 38,
                        decoration: const BoxDecoration(
                          color: Colors.white,
                          shape: BoxShape.circle,
                        ),
                        child: Center(
                          child: Glyph(
                            widget.playing ? 'pause' : 'play',
                            size: 20,
                            color: const Color(0xff17181b),
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    ),
  );
}

class PlaylistCard extends StatelessWidget {
  const PlaylistCard(
    this.playlist, {
    super.key,
    required this.onTap,
    this.compact = false,
  });
  final Json playlist;
  final VoidCallback onTap;
  final bool compact;
  @override
  Widget build(BuildContext context) {
    final ts = objects(playlist['tracks']),
        cover = ts.isEmpty ? '' : trackCover(ts.first);
    if (compact) {
      return Material(
        color: context.paints.surface,
        borderRadius: BorderRadius.circular(20),
        child: AnicryTap(
          onTap: onTap,
          radius: 20,
          child: Padding(
            padding: const EdgeInsets.all(10),
            child: Row(
              children: [
                Artwork(
                  cover,
                  size: 68,
                  radius: SoundFlowCorners.inner(20, 10),
                ),
                const SizedBox(width: 16),
                Expanded(
                  child: Column(
                    mainAxisAlignment: MainAxisAlignment.center,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        playlist['name'] ?? 'Плейліст',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(
                          fontWeight: FontWeight.w600,
                          fontSize: 15,
                        ),
                      ),
                      const SizedBox(height: 6),
                      Text(
                        playlist['subtitle']?.toString() ??
                            trackCount(ts.length),
                        style: TextStyle(
                          color: context.paints.muted,
                          fontSize: 13,
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
      );
    }
    return Material(
      color: Colors.transparent,
      borderRadius: BorderRadius.circular(SoundFlowCorners.card),
      child: AnicryTap(
        onTap: onTap,
        radius: SoundFlowCorners.card,
        hoverColor: context.paints.surface,
        child: Padding(
          padding: const EdgeInsets.all(SoundFlowCorners.cardInset),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: SizedBox.expand(
                  child: Artwork(
                    cover,
                    radius: SoundFlowCorners.inner(
                      SoundFlowCorners.card,
                      SoundFlowCorners.cardInset,
                    ),
                  ),
                ),
              ),
              const SizedBox(height: 10),
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 4),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Tooltip(
                      message: playlist['name']?.toString() ?? 'Плейліст',
                      child: Text(
                        playlist['name']?.toString() ?? 'Плейліст',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(
                          fontSize: 15,
                          height: 1.3,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ),
                    const SizedBox(height: 5),
                    Text(
                      trackCount(ts.length),
                      style: TextStyle(
                        fontSize: 12,
                        height: 1.3,
                        color: context.paints.muted,
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 4),
            ],
          ),
        ),
      ),
    );
  }
}

class TrackRow extends StatefulWidget {
  const TrackRow(
    this.track, {
    super.key,
    required this.core,
    required this.onPlay,
    required this.onAddPlaylist,
    this.queueIndex,
    this.quiet = false,
  });
  final Json track;
  final Core core;
  final VoidCallback onPlay, onAddPlaylist;
  final int? queueIndex;
  final bool quiet;
  @override
  State<TrackRow> createState() => _TrackRowState();
}

class _TrackRowState extends State<TrackRow> {
  bool hover = false;
  Future<void> menu(Offset offset) async {
    final c = widget.core;
    final result = await showMenu<String>(
      context: context,
      position: RelativeRect.fromLTRB(offset.dx, offset.dy, 0, 0),
      items: [
        PopupMenuItem(
          value: 'like',
          child: Text(
            c.isLiked(widget.track)
                ? 'Прибрати з улюбленого'
                : 'Додати в улюблене',
          ),
        ),
        const PopupMenuItem(
          value: 'add_next',
          child: Text('Відтворити наступним'),
        ),
        const PopupMenuItem(
          value: 'playlist',
          child: Text('Додати до плейліста'),
        ),
        const PopupMenuItem(value: 'creator', child: Text('Перейти до автора')),
        const PopupMenuItem(value: 'download', child: Text('Завантажити')),
        if (widget.queueIndex != null) ...[
          PopupMenuItem(
            value: 'up',
            enabled: widget.queueIndex! > c.queueIndex + 1,
            child: const Text('Перемістити вище'),
          ),
          PopupMenuItem(
            value: 'down',
            enabled: widget.queueIndex! < c.queue.length - 1,
            child: const Text('Перемістити нижче'),
          ),
          const PopupMenuItem(
            value: 'remove_queue',
            child: Text('Прибрати з черги'),
          ),
        ],
        if (c.route == 'playlist')
          const PopupMenuItem(
            value: 'remove_playlist',
            child: Text('Прибрати з плейліста'),
          ),
      ],
    );
    if (result == null) return;
    switch (result) {
      case 'playlist':
        widget.onAddPlaylist();
      case 'creator':
        c.showCreator(widget.track);
      case 'up':
        c.action('queue_move', {
          'from': widget.queueIndex,
          'to': widget.queueIndex! - 1,
        });
      case 'down':
        c.action('queue_move', {
          'from': widget.queueIndex,
          'to': widget.queueIndex! + 1,
        });
      case 'remove_queue':
        c.action('queue_remove', {'index': widget.queueIndex});
      case 'remove_playlist':
        c.call('remove_from_playlist', [
          c.selectedPlaylist,
          widget.track['id'],
        ]);
      default:
        c.action(result, widget.track);
    }
  }

  @override
  Widget build(BuildContext context) {
    final c = widget.core, p = context.paints, t = widget.track;
    final current =
            c.hasTrack &&
            (widget.queueIndex != null
                ? widget.queueIndex == c.queueIndex
                : trackKey(t) == trackKey(c.current)),
        active = current && c.playing;
    return MouseRegion(
      onEnter: (_) => setState(() => hover = true),
      onExit: (_) => setState(() => hover = false),
      child: GestureDetector(
        onSecondaryTapDown: (details) => menu(details.globalPosition),
        child: Material(
          color: current ? p.surface : Colors.transparent,
          borderRadius: BorderRadius.circular(9),
          child: InkWell(
            onTap: widget.onPlay,
            onFocusChange: (f) => setState(() => hover = f),
            borderRadius: BorderRadius.circular(9),
            child: LayoutBuilder(
              builder: (context, box) => SizedBox(
                height: 64,
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 8),
                  child: Row(
                    children: [
                      Artwork(trackCover(t), size: 42, radius: 7),
                      const SizedBox(width: 14),
                      Expanded(
                        child: Column(
                          mainAxisAlignment: MainAxisAlignment.center,
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              trackTitle(t),
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: TextStyle(
                                color: current ? p.accent : p.ink,
                                fontSize: 15,
                              ),
                            ),
                            const SizedBox(height: 3),
                            InkWell(
                              onTap: () => c.showCreator(t),
                              child: Text(
                                trackArtist(t),
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: TextStyle(color: p.muted, fontSize: 13),
                              ),
                            ),
                          ],
                        ),
                      ),
                      if (box.maxWidth > 850)
                        SizedBox(
                          width: 100,
                          child: Text(
                            sourceName(t),
                            style: TextStyle(color: p.muted, fontSize: 12),
                          ),
                        ),
                      const SizedBox(width: 12),
                      SizedBox(
                        width: 44,
                        child: Text(
                          t['duration_formatted']?.toString() ?? '',
                          textAlign: TextAlign.right,
                          style: TextStyle(color: p.muted, fontSize: 12),
                        ),
                      ),
                      const SizedBox(width: 12),
                      if (!widget.quiet)
                        Control(
                          'heart',
                          c.isLiked(t)
                              ? 'Прибрати з улюбленого'
                              : 'Додати в улюблене',
                          active: c.isLiked(t),
                          color: c.isLiked(t)
                              ? p.accent
                              : hover
                              ? p.ink
                              : p.muted,
                          onTap: () => c.action('like', t),
                        ),
                      if (!widget.quiet)
                        Control(
                          active ? 'pause' : 'play',
                          active ? 'Пауза' : 'Відтворити',
                          onTap: widget.onPlay,
                        ),
                      Control(
                        'more',
                        'Дії з треком',
                        onTap: () {
                          final r = context.findRenderObject() as RenderBox;
                          menu(
                            r.localToGlobal(
                              Offset(r.size.width - 48, r.size.height),
                            ),
                          );
                        },
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

Future<void> showVolume(BuildContext anchor, Core core) async {
  final render = anchor.findRenderObject() as RenderBox;
  final point = render.localToGlobal(Offset.zero);
  final screen = MediaQuery.sizeOf(anchor);
  await showGeneralDialog<void>(
    context: anchor,
    barrierDismissible: true,
    barrierLabel: 'Закрити гучність',
    barrierColor: Colors.transparent,
    transitionDuration: MediaQuery.disableAnimationsOf(anchor)
        ? Duration.zero
        : Anicry.popover,
    pageBuilder: (context, animation, secondary) => Stack(
      children: [
        Positioned(
          left: (point.dx - 86).clamp(12, screen.width - 224),
          top: (point.dy - 74).clamp(12, screen.height - 72),
          child: Material(
            color: anchor.paints.surface,
            borderRadius: BorderRadius.circular(18),
            child: SizedBox(
              width: 212,
              height: 56,
              child: ValueListenableBuilder<Json>(
                valueListenable: core.playback,
                builder: (context, value, _) {
                  final volume = (value['volume'] as num?)?.toDouble() ?? .8;
                  return Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 10),
                    child: Row(
                      children: [
                        Control(
                          volume == 0 ? 'volume-x' : 'volume-2',
                          'Вимкнути звук',
                          size: 32,
                          onTap: () => core.player('mute'),
                        ),
                        Expanded(
                          child: MediaSlider(
                            value: volume,
                            continuous: true,
                            onChanged: core.setVolume,
                            semanticFormatterCallback: (v) =>
                                'Гучність ${(v * 100).round()}%',
                          ),
                        ),
                        SizedBox(
                          width: 32,
                          child: Text(
                            '${(volume * 100).round()}%',
                            textAlign: TextAlign.right,
                            style: const TextStyle(fontSize: 11),
                          ),
                        ),
                      ],
                    ),
                  );
                },
              ),
            ),
          ),
        ),
      ],
    ),
    transitionBuilder: (context, animation, secondary, child) => FadeTransition(
      opacity: CurvedAnimation(parent: animation, curve: Anicry.easeOut),
      child: child,
    ),
  );
}

String initials(String name) {
  final words = name
      .trim()
      .split(RegExp(r'\s+'))
      .where((s) => s.isNotEmpty)
      .take(2);
  return words
      .map((s) => String.fromCharCodes(s.runes.take(1)))
      .join()
      .toUpperCase();
}

class ArtistCard extends StatelessWidget {
  const ArtistCard(this.track, {super.key, required this.core});
  final Json track;
  final Core core;
  @override
  Widget build(BuildContext context) {
    final data = core.artistData(track), p = context.paints;
    final avatar = [data['avatar'], track['artist_avatar']]
        .map((v) => v?.toString() ?? '')
        .firstWhere((v) => v.isNotEmpty, orElse: () => '');
    final name = data['name']?.toString() ?? trackArtist(track);
    return Material(
      color: Colors.transparent,
      borderRadius: BorderRadius.circular(18),
      child: AnicryTap(
        onTap: () => core.showCreator(track),
        radius: 18,
        hoverColor: p.surface,
        child: Padding(
          padding: const EdgeInsets.all(12),
          child: Column(
            children: [
              if (avatar.isNotEmpty)
                Artwork(avatar, size: 112, radius: 56)
              else
                Container(
                  width: 112,
                  height: 112,
                  decoration: BoxDecoration(
                    shape: BoxShape.circle,
                    color: p.hover,
                  ),
                  child: Center(
                    child: Text(
                      initials(name),
                      style: TextStyle(
                        fontSize: 30,
                        fontWeight: FontWeight.w600,
                        color: p.ink,
                      ),
                    ),
                  ),
                ),
              const SizedBox(height: 14),
              SizedBox(
                height: 36,
                child: Text(
                  name,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  textAlign: TextAlign.center,
                  style: const TextStyle(
                    fontSize: 14,
                    height: 1.25,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
              const SizedBox(height: 4),
              Text(
                sourceName(track),
                style: TextStyle(fontSize: 12, color: p.muted),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class ErrorNotice extends StatefulWidget {
  const ErrorNotice(this.message, {super.key, required this.onDismiss});
  final String message;
  final VoidCallback onDismiss;
  @override
  State<ErrorNotice> createState() => _ErrorNoticeState();
}

class _ErrorNoticeState extends State<ErrorNotice>
    with SingleTickerProviderStateMixin {
  late final controller = AnimationController(
    vsync: this,
    duration: Anicry.reveal,
  );
  Timer? timer;
  @override
  void initState() {
    super.initState();
    timer = Timer(const Duration(seconds: 7), () {
      if (mounted) widget.onDismiss();
    });
  }

  bool revealed = false;
  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (MediaQuery.disableAnimationsOf(context)) {
      controller.value = 1;
    } else if (!revealed) {
      controller.forward();
    }
    revealed = true;
  }

  @override
  void dispose() {
    timer?.cancel();
    controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => FadeTransition(
    opacity: controller,
    child: SlideTransition(
      position: Tween<Offset>(
        begin: const Offset(0, .06),
        end: Offset.zero,
      ).animate(CurvedAnimation(parent: controller, curve: Anicry.easeOut)),
      child: Material(
        color: context.paints.surface,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(12),
          side: BorderSide(color: context.paints.ink.withValues(alpha: .12)),
        ),
        child: Padding(
          padding: const EdgeInsets.fromLTRB(16, 12, 8, 12),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Glyph('info', size: 18, color: Color(0xffedb0a7)),
              const SizedBox(width: 12),
              Flexible(
                child: Text(
                  widget.message,
                  maxLines: 4,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(fontSize: 13, height: 1.4),
                ),
              ),
              const SizedBox(width: 12),
              Control(
                'x',
                'Закрити повідомлення',
                size: 28,
                onTap: widget.onDismiss,
              ),
            ],
          ),
        ),
      ),
    ),
  );
}
