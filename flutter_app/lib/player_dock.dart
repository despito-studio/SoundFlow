import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'anicry_motion.dart';
import 'core.dart';
import 'design.dart';
import 'media_slider.dart';

typedef _QueueEntry = ({int index, Json track});

List<_QueueEntry> _upcoming(Core core) {
  final queue = core.queue;
  if (queue.isEmpty) return [];
  final current = core.queueIndex.clamp(0, queue.length - 1);
  final shuffle = core.settings['shuffle'] == true;
  final wrap = core.settings['repeat'] == 'all';
  final indices = shuffle
      ? [
          for (var i = 0; i < queue.length; i++)
            if (i != current) i,
        ]
      : [
          for (var i = current + 1; i < queue.length; i++) i,
          if (wrap)
            for (var i = 0; i < current; i++) i,
        ];
  if (indices.isEmpty && wrap) indices.add(current);
  return [for (final index in indices) (index: index, track: queue[index])];
}

/// A glass dock that follows vertical gestures into the track and queue view.
class PlayerDock extends StatefulWidget {
  const PlayerDock(this.core, {super.key, required this.onAddPlaylist});
  final Core core;
  final ValueChanged<Json> onAddPlaylist;
  @override
  State<PlayerDock> createState() => _PlayerDockState();
}

class _PlayerDockState extends State<PlayerDock>
    with SingleTickerProviderStateMixin {
  late final expansion = AnimationController(
    vsync: this,
    duration: Anicry.layout,
  );
  final focus = FocusScopeNode(
    traversalEdgeBehavior: TraversalEdgeBehavior.parentScope,
  );
  FocusNode? previousFocus;
  double travel = 1;
  late String route;

  @override
  void initState() {
    super.initState();
    route = widget.core.route;
    widget.core.addListener(syncRoute);
  }

  void syncRoute() {
    if (route == widget.core.route) return;
    route = widget.core.route;
    if (expansion.value > 0) expand(false);
  }

  void expand(bool value) {
    focus.traversalEdgeBehavior = value
        ? TraversalEdgeBehavior.closedLoop
        : TraversalEdgeBehavior.parentScope;
    if (value && !focus.hasFocus) {
      previousFocus = FocusManager.instance.primaryFocus;
      focus.requestFocus();
    } else if (!value) {
      focus.unfocus();
      if (previousFocus?.context != null) previousFocus?.requestFocus();
    }
    if (MediaQuery.disableAnimationsOf(context)) {
      expansion.value = value ? 1 : 0;
    } else {
      expansion.animateTo(value ? 1 : 0, curve: Anicry.easeOut);
    }
  }

  void dragStart(DragStartDetails _) {
    expansion.stop();
  }

  void dragUpdate(DragUpdateDetails details) {
    expansion.value = (expansion.value - details.delta.dy / travel).clamp(0, 1);
  }

  void dragEnd(DragEndDetails details) {
    final velocity = details.primaryVelocity ?? 0;
    expand(velocity.abs() > 250 ? velocity < 0 : expansion.value > .24);
  }

  Widget dragArea(Widget child) => GestureDetector(
    behavior: HitTestBehavior.deferToChild,
    onVerticalDragStart: dragStart,
    onVerticalDragUpdate: dragUpdate,
    onVerticalDragEnd: dragEnd,
    onVerticalDragCancel: () {
      expand(expansion.value > .5);
    },
    child: child,
  );

  @override
  void dispose() {
    widget.core.removeListener(syncRoute);
    expansion.dispose();
    focus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, box) {
      final p = context.paints;
      final collapsedHeight =
          104.0 +
          math.max(0, MediaQuery.textScalerOf(context).scale(14) - 14) * 2;
      final expandedHeight = math.max(collapsedHeight, box.maxHeight - 32);
      final fullWidth = math.max(0.0, box.maxWidth - 32);
      final dockWidth = math.min(720.0, fullWidth);
      travel = math.max(1, expandedHeight - collapsedHeight);
      return CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.escape): () => expand(false),
        },
        child: FocusScope(
          node: focus,
          child: dragArea(
            AnimatedBuilder(
              animation: expansion,
              builder: (context, _) {
                final progress = expansion.value;
                final width = ui.lerpDouble(dockWidth, fullWidth, progress)!;
                final height = ui.lerpDouble(
                  collapsedHeight,
                  expandedHeight,
                  progress,
                )!;
                return Stack(
                  children: [
                    if (progress > 0)
                      Positioned.fill(
                        key: const ValueKey('player-dock-scrim'),
                        child: IgnorePointer(
                          ignoring: progress < .85,
                          child: GestureDetector(
                            onTap: () => expand(false),
                            child: ColoredBox(
                              color: Colors.black.withValues(
                                alpha: progress * .28,
                              ),
                            ),
                          ),
                        ),
                      ),
                    Positioned(
                      key: const ValueKey('player-dock-surface'),
                      left: (box.maxWidth - width) / 2,
                      bottom: 16,
                      width: width,
                      height: height,
                      child: DecoratedBox(
                        decoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(28),
                          boxShadow: [
                            BoxShadow(
                              color: Colors.black.withValues(
                                alpha: widget.core.dark ? .16 : .08,
                              ),
                              blurRadius: 24,
                              offset: const Offset(0, 12),
                            ),
                          ],
                        ),
                        child: ClipRRect(
                          borderRadius: BorderRadius.circular(28),
                          child: BackdropFilter(
                            filter: ui.ImageFilter.blur(sigmaX: 24, sigmaY: 24),
                            child: DecoratedBox(
                              decoration: BoxDecoration(
                                borderRadius: BorderRadius.circular(28),
                                border: Border.all(
                                  color: p.ink.withValues(
                                    alpha: widget.core.dark ? .10 : .08,
                                  ),
                                ),
                                gradient: LinearGradient(
                                  begin: Alignment.topLeft,
                                  end: Alignment.bottomRight,
                                  colors: [
                                    p.surface.withValues(
                                      alpha: ui.lerpDouble(
                                        widget.core.dark ? .60 : .80,
                                        .96,
                                        progress,
                                      )!,
                                    ),
                                    p.canvas.withValues(
                                      alpha: ui.lerpDouble(
                                        widget.core.dark ? .46 : .68,
                                        .94,
                                        progress,
                                      )!,
                                    ),
                                  ],
                                ),
                              ),
                              child: Stack(
                                fit: StackFit.expand,
                                children: [
                                  if (progress < 1)
                                    IgnorePointer(
                                      ignoring: progress > .12,
                                      child: Opacity(
                                        opacity: (1 - progress * 3).clamp(0, 1),
                                        child: OverflowBox(
                                          alignment: Alignment.bottomCenter,
                                          minHeight: collapsedHeight,
                                          maxHeight: collapsedHeight,
                                          child: ExcludeFocus(
                                            excluding: progress > .12,
                                            child: _DockControls(
                                              widget.core,
                                              onExpand: () => expand(true),
                                            ),
                                          ),
                                        ),
                                      ),
                                    ),
                                  if (progress > 0)
                                    IgnorePointer(
                                      ignoring: progress < .85,
                                      child: Opacity(
                                        opacity: ((progress - .15) / .85).clamp(
                                          0,
                                          1,
                                        ),
                                        child: OverflowBox(
                                          alignment: Alignment.topCenter,
                                          minWidth: fullWidth,
                                          maxWidth: fullWidth,
                                          minHeight: expandedHeight,
                                          maxHeight: expandedHeight,
                                          child: ExcludeFocus(
                                            excluding: progress < .85,
                                            child: _PlayerDetails(
                                              widget.core,
                                              onCollapse: () => expand(false),
                                              onAddPlaylist:
                                                  widget.onAddPlaylist,
                                            ),
                                          ),
                                        ),
                                      ),
                                    ),
                                ],
                              ),
                            ),
                          ),
                        ),
                      ),
                    ),
                  ],
                );
              },
            ),
          ),
        ),
      );
    },
  );
}

class _DockControls extends StatelessWidget {
  const _DockControls(this.core, {required this.onExpand});
  final Core core;
  final VoidCallback onExpand;

  Future<void> options(BuildContext context, Offset position) async {
    final result = await showMenu<String>(
      context: context,
      position: RelativeRect.fromLTRB(position.dx, position.dy - 180, 16, 0),
      items: [
        PopupMenuItem(
          value: 'like',
          child: Text(
            core.isLiked(core.current)
                ? 'Прибрати з улюбленого'
                : 'Додати в улюблене',
          ),
        ),
        CheckedPopupMenuItem(
          value: 'shuffle',
          checked: core.settings['shuffle'] == true,
          child: const Text('Перемішувати'),
        ),
        PopupMenuItem(
          value: 'repeat',
          child: Text(
            'Повтор: ${switch (core.settings['repeat']) {
              'all' => 'уся черга',
              'one' => 'один трек',
              _ => 'вимкнено',
            }}',
          ),
        ),
        const PopupMenuItem(value: 'creator', child: Text('Перейти до автора')),
      ],
    );
    if (result == 'like') {
      core.action('like', core.current);
    } else if (result == 'creator') {
      core.showCreator(core.current);
    } else if (result != null) {
      core.player(result);
    }
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: core,
    builder: (context, _) => ValueListenableBuilder<Json>(
      valueListenable: core.playback,
      builder: (context, pb, _) {
        final p = context.paints, track = object(pb['track']);
        final volume = (pb['volume'] as num?)?.toDouble() ?? .8;
        final duration = (pb['duration'] as num?)?.toDouble() ?? 0;
        final queued = _upcoming(core).length;
        return GestureDetector(
          onSecondaryTapDown: (details) =>
              options(context, details.globalPosition),
          child: Padding(
            padding: const EdgeInsets.fromLTRB(16, 8, 16, 7),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              mainAxisSize: MainAxisSize.min,
              children: [
                Tooltip(
                  message: 'Потягни вгору, щоб відкрити пісню та чергу',
                  child: Semantics(
                    button: true,
                    label: 'Розгорнути плеєр',
                    child: AnicryTap(
                      onTap: onExpand,
                      radius: 6,
                      child: SizedBox(
                        height: 10,
                        width: 72,
                        child: Center(
                          child: Container(
                            width: 24,
                            height: 2,
                            decoration: BoxDecoration(
                              color: p.muted.withValues(alpha: .4),
                              borderRadius: BorderRadius.circular(2),
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
                const SizedBox(height: 2),
                Row(
                  children: [
                    Expanded(
                      child: Semantics(
                        button: true,
                        label: 'Зараз звучить: ${trackTitle(track)}',
                        child: AnicryTap(
                          onTap: onExpand,
                          radius: 10,
                          child: Row(
                            children: [
                              Artwork(trackCover(track), size: 44, radius: 10),
                              const SizedBox(width: 12),
                              Expanded(
                                child: Column(
                                  mainAxisSize: MainAxisSize.min,
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    Tooltip(
                                      message: trackTitle(track),
                                      child: Text(
                                        trackTitle(track),
                                        maxLines: 1,
                                        overflow: TextOverflow.ellipsis,
                                        style: const TextStyle(
                                          fontSize: 13,
                                          height: 1.3,
                                          fontWeight: FontWeight.w500,
                                        ),
                                      ),
                                    ),
                                    const SizedBox(height: 4),
                                    Text(
                                      pb['loading'] == true
                                          ? 'Завантажуємо…'
                                          : trackArtist(track),
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: TextStyle(
                                        fontSize: 11,
                                        height: 1.3,
                                        color: p.muted,
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
                    const SizedBox(width: 12),
                    Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Control(
                          'skip-back',
                          'Попередній трек',
                          size: 32,
                          glyphSize: 19,
                          onTap: () => core.player('previous'),
                        ),
                        Padding(
                          padding: const EdgeInsets.symmetric(horizontal: 7),
                          child: Control(
                            pb['playing'] == true ? 'pause' : 'play',
                            pb['playing'] == true ? 'Пауза' : 'Відтворити',
                            primary: true,
                            size: 40,
                            glyphSize: 21,
                            onTap: () => core.player('toggle'),
                          ),
                        ),
                        Control(
                          'skip-forward',
                          'Наступний трек',
                          size: 32,
                          glyphSize: 19,
                          onTap: () => core.player('next'),
                        ),
                      ],
                    ),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Row(
                        mainAxisAlignment: MainAxisAlignment.end,
                        children: [
                          Builder(
                            builder: (anchor) => Control(
                              volume == 0 ? 'volume-x' : 'volume-2',
                              'Гучність · ${(volume * 100).round()}%',
                              size: 30,
                              glyphSize: 17,
                              onTap: () => showVolume(anchor, core),
                            ),
                          ),
                          Control(
                            'list',
                            'Черга · далі $queued',
                            active: core.route == 'queue',
                            size: 30,
                            glyphSize: 17,
                            onTap: onExpand,
                          ),
                          Control(
                            'picture-in-picture',
                            'Dynamic Island · Ctrl+Alt+M',
                            size: 30,
                            glyphSize: 17,
                            onTap: core.island,
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 6),
                PlaybackTimeline(
                  key: ValueKey(trackKey(track)),
                  position: (pb['position'] as num?)?.toDouble() ?? 0,
                  duration: duration,
                  inline: true,
                  fontSize: 10,
                  onSeek: duration > 0
                      ? (v) => core.command({'type': 'seek', 'value': v})
                      : null,
                ),
              ],
            ),
          ),
        );
      },
    ),
  );
}

class _PlayerDetails extends StatelessWidget {
  const _PlayerDetails(
    this.core, {
    required this.onCollapse,
    required this.onAddPlaylist,
  });
  final Core core;
  final VoidCallback onCollapse;
  final ValueChanged<Json> onAddPlaylist;

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: core,
    builder: (context, _) => ValueListenableBuilder<Json>(
      valueListenable: core.playback,
      builder: (context, playback, _) {
        final tracks = _upcoming(core);
        final p = context.paints;
        return Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(24, 12, 16, 12),
              child: Column(
                children: [
                  Container(
                    width: 34,
                    height: 4,
                    decoration: BoxDecoration(
                      color: p.muted.withValues(alpha: .5),
                      borderRadius: BorderRadius.circular(2),
                    ),
                  ),
                  const SizedBox(height: 10),
                  Row(
                    children: [
                      Text(
                        'Зараз звучить',
                        style: TextStyle(
                          fontSize: 14,
                          fontWeight: FontWeight.w600,
                          color: p.muted,
                        ),
                      ),
                      const Spacer(),
                      Control(
                        'chevron-down',
                        'Згорнути плеєр · Escape',
                        size: 34,
                        onTap: onCollapse,
                      ),
                    ],
                  ),
                ],
              ),
            ),
            Divider(height: 1, color: p.ink.withValues(alpha: .08)),
            Expanded(
              child: LayoutBuilder(
                builder: (context, box) {
                  if (box.maxWidth >= 840) {
                    return Row(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        Expanded(
                          flex: 5,
                          child: SingleChildScrollView(
                            padding: const EdgeInsets.fromLTRB(32, 24, 32, 28),
                            child: _track(context, playback),
                          ),
                        ),
                        Container(
                          width: 1,
                          color: p.ink.withValues(alpha: .07),
                        ),
                        Expanded(
                          flex: 5,
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Padding(
                                padding: const EdgeInsets.fromLTRB(
                                  24,
                                  20,
                                  16,
                                  12,
                                ),
                                child: _queueHeader(context, tracks.length),
                              ),
                              Expanded(
                                child: tracks.isEmpty
                                    ? _emptyQueue(context)
                                    : ListView.builder(
                                        padding: const EdgeInsets.fromLTRB(
                                          12,
                                          0,
                                          12,
                                          24,
                                        ),
                                        itemCount: tracks.length,
                                        itemBuilder: (context, i) =>
                                            _queueRow(tracks[i]),
                                      ),
                              ),
                            ],
                          ),
                        ),
                      ],
                    );
                  }
                  return CustomScrollView(
                    slivers: [
                      SliverPadding(
                        padding: const EdgeInsets.all(24),
                        sliver: SliverToBoxAdapter(
                          child: _track(context, playback),
                        ),
                      ),
                      SliverPadding(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 24,
                          vertical: 12,
                        ),
                        sliver: SliverToBoxAdapter(
                          child: _queueHeader(context, tracks.length),
                        ),
                      ),
                      if (tracks.isEmpty)
                        SliverToBoxAdapter(child: _emptyQueue(context)),
                      SliverPadding(
                        padding: const EdgeInsets.fromLTRB(12, 0, 12, 24),
                        sliver: SliverList.builder(
                          itemCount: tracks.length,
                          itemBuilder: (context, i) => _queueRow(tracks[i]),
                        ),
                      ),
                    ],
                  );
                },
              ),
            ),
          ],
        );
      },
    ),
  );

  Widget _queueRow(_QueueEntry entry) => TrackRow(
    entry.track,
    key: ValueKey('expanded-queue:${entry.index}:${trackKey(entry.track)}'),
    core: core,
    quiet: true,
    queueIndex: entry.index,
    onPlay: () => core.action('queue_play', {'index': entry.index}),
    onAddPlaylist: () => onAddPlaylist(entry.track),
  );

  Widget _queueHeader(BuildContext context, int count) => Row(
    children: [
      Expanded(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              core.settings['shuffle'] == true ||
                      core.settings['repeat'] == 'one'
                  ? 'У черзі'
                  : 'Далі в черзі',
              style: const TextStyle(fontSize: 18, fontWeight: FontWeight.w600),
            ),
            const SizedBox(height: 5),
            Text(
              core.settings['repeat'] == 'one'
                  ? 'Повторюється поточна пісня'
                  : core.settings['shuffle'] == true
                  ? 'Випадковий порядок · ${trackCount(count)}'
                  : trackCount(count),
              style: TextStyle(fontSize: 12, color: context.paints.muted),
            ),
          ],
        ),
      ),
      Control(
        'trash-2',
        'Очистити наступні треки',
        size: 32,
        glyphSize: 17,
        onTap: count == 0 ? null : () => core.action('queue_clear'),
      ),
    ],
  );

  Widget _emptyQueue(BuildContext context) => Padding(
    padding: const EdgeInsets.all(24),
    child: EmptyState(
      'Черга відкрита для нового',
      'Додай наступний трек із пошуку або своєї медіатеки.',
      action: ActionButton(
        'Знайти музику',
        icon: 'search',
        onTap: () {
          onCollapse();
          core.nav('search');
        },
      ),
    ),
  );

  Widget _track(BuildContext context, Json playback) {
    final track = object(playback['track']), p = context.paints;
    final repeat = core.settings['repeat']?.toString() ?? 'off';
    final duration = (playback['duration'] as num?)?.toDouble() ?? 0;
    final volume = (playback['volume'] as num?)?.toDouble() ?? .8;
    return Column(
      children: [
        ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 300),
          child: AspectRatio(
            aspectRatio: 1,
            child: Artwork(trackCover(track), radius: 24),
          ),
        ),
        const SizedBox(height: 22),
        Text(
          trackTitle(track),
          textAlign: TextAlign.center,
          maxLines: 2,
          overflow: TextOverflow.ellipsis,
          style: const TextStyle(
            fontSize: 26,
            height: 1.2,
            fontWeight: FontWeight.w600,
          ),
        ),
        const SizedBox(height: 8),
        AnicryTap(
          onTap: () {
            onCollapse();
            core.showCreator(track);
          },
          radius: 8,
          child: Padding(
            padding: const EdgeInsets.all(4),
            child: Text(
              trackArtist(track),
              textAlign: TextAlign.center,
              style: TextStyle(fontSize: 15, color: p.muted),
            ),
          ),
        ),
        const SizedBox(height: 20),
        PlaybackTimeline(
          key: ValueKey('expanded:${trackKey(track)}'),
          position: (playback['position'] as num?)?.toDouble() ?? 0,
          duration: duration,
          fontSize: 12,
          onSeek: duration > 0
              ? (v) => core.command({'type': 'seek', 'value': v})
              : null,
        ),
        const SizedBox(height: 12),
        Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Control(
              'shuffle',
              'Перемішувати',
              active: core.settings['shuffle'] == true,
              size: 34,
              glyphSize: 18,
              onTap: () => core.player('shuffle'),
            ),
            const SizedBox(width: 10),
            Control(
              'skip-back',
              'Попередній трек',
              size: 38,
              glyphSize: 23,
              onTap: () => core.player('previous'),
            ),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12),
              child: Control(
                playback['playing'] == true ? 'pause' : 'play',
                playback['playing'] == true ? 'Пауза' : 'Відтворити',
                primary: true,
                size: 58,
                glyphSize: 27,
                onTap: () => core.player('toggle'),
              ),
            ),
            Control(
              'skip-forward',
              'Наступний трек',
              size: 38,
              glyphSize: 23,
              onTap: () => core.player('next'),
            ),
            const SizedBox(width: 10),
            Control(
              repeat == 'one' ? 'repeat-1' : 'repeat',
              'Повтор: ${switch (repeat) {
                'one' => 'один трек',
                'all' => 'уся черга',
                _ => 'вимкнено',
              }}',
              active: repeat != 'off',
              size: 34,
              glyphSize: 18,
              onTap: () => core.player('repeat'),
            ),
          ],
        ),
        const SizedBox(height: 16),
        Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Control(
              'heart',
              core.isLiked(track)
                  ? 'Прибрати з улюбленого'
                  : 'Додати в улюблене',
              active: core.isLiked(track),
              size: 34,
              glyphSize: 18,
              onTap: () => core.action('like', track),
            ),
            const SizedBox(width: 12),
            Control(
              volume == 0 ? 'volume-x' : 'volume-2',
              volume == 0 ? 'Увімкнути звук' : 'Вимкнути звук',
              size: 32,
              glyphSize: 18,
              onTap: () => core.player('mute'),
            ),
            SizedBox(
              width: 100,
              child: MediaSlider(
                value: volume,
                continuous: true,
                onChanged: core.setVolume,
                semanticFormatterCallback: (v) =>
                    'Гучність ${(v * 100).round()}%',
              ),
            ),
            const SizedBox(width: 12),
            Control(
              'picture-in-picture',
              'Відкрити Dynamic Island',
              size: 34,
              glyphSize: 18,
              onTap: core.island,
            ),
          ],
        ),
        const SizedBox(height: 14),
        Text(
          playback['loading'] == true
              ? 'Завантажуємо аудіо…'
              : '${sourceName(track)}${track['duration_formatted'] == null ? '' : ' · ${track['duration_formatted']}'}',
          style: TextStyle(fontSize: 12, color: p.muted),
        ),
      ],
    );
  }
}
