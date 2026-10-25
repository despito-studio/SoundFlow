import 'dart:async';
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'anicry_motion.dart';
import 'concentric.dart';
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
  late final AnimationController expansion =
      AnimationController(
        vsync: this,
        duration: Anicry.layout,
      )..addStatusListener((status) {
        if (status == AnimationStatus.completed ||
            status == AnimationStatus.dismissed) {
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (!mounted) return;
            if (expansion.value == 1) {
              detailsFocus.requestFocus();
            } else if (expansion.value == 0 && previousFocus?.context != null) {
              previousFocus?.requestFocus();
            }
          });
        }
      });
  final focus = FocusScopeNode(
    traversalEdgeBehavior: TraversalEdgeBehavior.parentScope,
  );
  final detailsFocus = FocusNode();
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
    if (value) {
      if (expansion.value < .85 || !focus.hasFocus) {
        previousFocus = FocusManager.instance.primaryFocus;
      }
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
    detailsFocus.dispose();
    focus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, box) {
      final fullWidth = math.max(0.0, box.maxWidth - 32);
      final dockWidth = _DockLayout.widthFor(fullWidth);
      final collapsedHeight = _DockLayout.heightFor(dockWidth);
      final expandedHeight = math.max(collapsedHeight, box.maxHeight - 32);
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
                      child: _ExpandedDockBackground(
                        progress: progress,
                        child: Stack(
                          fit: StackFit.expand,
                          children: [
                            IgnorePointer(
                              ignoring: progress > .12,
                              child: Opacity(
                                opacity: (1 - progress * 3).clamp(0, 1),
                                child: OverflowBox(
                                  alignment: Alignment.bottomCenter,
                                  minWidth: dockWidth,
                                  maxWidth: dockWidth,
                                  minHeight: collapsedHeight,
                                  maxHeight: collapsedHeight,
                                  child: ExcludeFocus(
                                    excluding: progress > .12,
                                    child: _DockControls(
                                      widget.core,
                                      active: progress == 0,
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
                                  opacity: ((progress - .15) / .85).clamp(0, 1),
                                  child: OverflowBox(
                                    alignment: Alignment.topCenter,
                                    minWidth: fullWidth,
                                    maxWidth: fullWidth,
                                    minHeight: expandedHeight,
                                    maxHeight: expandedHeight,
                                    child: ExcludeFocus(
                                      excluding: progress < .85,
                                      child: Focus(
                                        focusNode: detailsFocus,
                                        child: _PlayerDetails(
                                          widget.core,
                                          onCollapse: () => expand(false),
                                          onAddPlaylist: widget.onAddPlaylist,
                                        ),
                                      ),
                                    ),
                                  ),
                                ),
                              ),
                          ],
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

class _ExpandedDockBackground extends StatelessWidget {
  const _ExpandedDockBackground({required this.progress, required this.child});
  final double progress;
  final Widget child;

  @override
  Widget build(BuildContext context) => ClipRRect(
    borderRadius: BorderRadius.circular(
      ui.lerpDouble(_DockLayout.radiusFor('track'), 28, progress)!,
    ),
    child: Stack(
      fit: StackFit.expand,
      children: [
        if (progress > 0)
          Positioned.fill(
            child: IgnorePointer(
              child: Opacity(
                opacity: (progress * 3).clamp(0, 1),
                child: BackdropFilter(
                  filter: ui.ImageFilter.blur(sigmaX: 24, sigmaY: 24),
                  child: ColoredBox(
                    color: context.paints.surface.withValues(alpha: .96),
                  ),
                ),
              ),
            ),
          ),
        child,
      ],
    ),
  );
}

class _DockControls extends StatelessWidget {
  const _DockControls(
    this.core, {
    required this.onExpand,
    required this.active,
  });
  final Core core;
  final VoidCallback onExpand;
  final bool active;

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
        const PopupMenuItem(value: 'island', child: Text('Dynamic Island')),
      ],
    );
    if (result == 'island') {
      core.island();
    } else if (result == 'like') {
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
        final p = context.paints;
        final queued = _upcoming(core).length;
        return LayoutBuilder(
          builder: (context, box) {
            final layout = _DockLayout(box.biggest);
            Widget place(String id, Widget child) => Positioned.fromRect(
              rect: layout.pieces[id]!,
              child: SizedBox(key: ValueKey('dock-$id'), child: child),
            );
            return ClipPath(
              clipper: _DockPiecesClipper(layout),
              child: BackdropFilter(
                filter: ui.ImageFilter.blur(sigmaX: 20, sigmaY: 20),
                child: ColoredBox(
                  color: p.surface.withValues(alpha: core.dark ? .94 : .96),
                  child: GestureDetector(
                    behavior: HitTestBehavior.deferToChild,
                    onSecondaryTapDown: (details) =>
                        options(context, details.globalPosition),
                    child: Stack(
                      children: [
                        place('track', _DockTrack(core, onExpand: onExpand)),
                        place(
                          'previous',
                          _DockButton(
                            'skip-back',
                            'Попередній трек',
                            onTap: () => core.player('previous'),
                          ),
                        ),
                        place(
                          'toggle',
                          _DockButton(
                            pb['playing'] == true ? 'pause' : 'play',
                            pb['playing'] == true ? 'Пауза' : 'Відтворити',
                            wide: true,
                            onTap: () => core.player('toggle'),
                          ),
                        ),
                        place(
                          'next',
                          _DockButton(
                            'skip-forward',
                            'Наступний трек',
                            onTap: () => core.player('next'),
                          ),
                        ),
                        place('volume', _DockVolume(core, active: active)),
                        place(
                          'queue',
                          _DockButton(
                            'list',
                            'Черга · далі $queued',
                            onTap: onExpand,
                          ),
                        ),
                        if (layout.pieces.containsKey('island'))
                          place(
                            'island',
                            _DockButton(
                              'picture-in-picture',
                              'Dynamic Island · Ctrl+Alt+M',
                              onTap: core.island,
                            ),
                          ),
                      ],
                    ),
                  ),
                ),
              ),
            );
          },
        );
      },
    ),
  );
}

class _DockLayout {
  _DockLayout(Size size) {
    final stacked = size.width < 504;
    final island = stacked ? size.width >= 340 : true;
    final controlsWidth = island ? 340.0 : 288.0;
    final trackWidth = stacked ? size.width : 148.0;
    pieces['track'] = Rect.fromLTWH(0, 0, trackWidth, 56);
    final ids = [
      'previous',
      'toggle',
      'next',
      'volume',
      'queue',
      if (island) 'island',
    ];
    var x = stacked ? (size.width - controlsWidth) / 2 : trackWidth + 16;
    final centerY = stacked ? 92.0 : 28.0;
    for (final id in ids) {
      final width = id == 'toggle' ? 72.0 : 44.0;
      final height = id == 'toggle' ? 56.0 : 44.0;
      pieces[id] = Rect.fromLTWH(x, centerY - height / 2, width, height);
      x += width + 8;
      if (id == 'next') x += 8;
    }
  }
  final pieces = <String, Rect>{};
  static double widthFor(double available) => switch (available) {
    >= 504 => 504,
    >= 340 => 340,
    _ => math.min(288, available),
  };
  static double heightFor(double width) => width < 504 ? 120 : 56;
  static double radiusFor(String id) => switch (id) {
    'track' || 'toggle' => SoundFlowCorners.panel - 4,
    _ => 22,
  };
  Path get path {
    final path = Path();
    for (final entry in pieces.entries) {
      path.addRRect(
        RRect.fromRectAndRadius(
          entry.value,
          Radius.circular(radiusFor(entry.key)),
        ),
      );
    }
    return path;
  }
}

class _DockPiecesClipper extends CustomClipper<Path> {
  const _DockPiecesClipper(this.layout);
  final _DockLayout layout;
  @override
  Path getClip(Size size) => layout.path;
  @override
  bool shouldReclip(covariant _DockPiecesClipper old) => true;
}

class _DockButton extends StatelessWidget {
  const _DockButton(
    this.icon,
    this.label, {
    required this.onTap,
    this.wide = false,
    this.focusNode,
  });
  final String icon, label;
  final VoidCallback onTap;
  final bool wide;
  final FocusNode? focusNode;
  @override
  Widget build(BuildContext context) => Tooltip(
    message: label,
    child: Semantics(
      button: true,
      label: label,
      child: AnicryTap(
        onTap: onTap,
        focusNode: focusNode,
        icon: icon,
        circle: !wide,
        radius: _DockLayout.radiusFor('toggle'),
        focusColor: context.paints.accent,
        child: Center(
          child: AnimatedSwitcher(
            duration: MediaQuery.disableAnimationsOf(context)
                ? Duration.zero
                : Anicry.press,
            child: Glyph(
              icon,
              key: ValueKey(icon),
              size: wide ? 24 : 20,
              filled: const {
                'play',
                'pause',
                'skip-back',
                'skip-forward',
              }.contains(icon),
            ),
          ),
        ),
      ),
    ),
  );
}

class _DockVolume extends StatefulWidget {
  const _DockVolume(this.core, {required this.active});
  final Core core;
  final bool active;
  @override
  State<_DockVolume> createState() => _DockVolumeState();
}

class _DockVolumeState extends State<_DockVolume>
    with SingleTickerProviderStateMixin, WidgetsBindingObserver {
  final portal = OverlayPortalController();
  final anchor = GlobalKey();
  final tapGroup = Object();
  final buttonFocus = FocusNode();
  final sliderFocus = FocusNode();
  late final reveal = AnimationController(vsync: this, duration: Anicry.popover)
    ..addStatusListener((status) {
      if (status == AnimationStatus.dismissed) {
        overPanel = false;
        dragging = false;
        closeTimer?.cancel();
        portal.hide();
      }
    });
  Timer? closeTimer;
  bool overButton = false, overPanel = false, dragging = false;
  late String route;

  @override
  void initState() {
    super.initState();
    route = widget.core.route;
    WidgetsBinding.instance.addObserver(this);
    FocusManager.instance.addEarlyKeyEventHandler(handleKey);
    sliderFocus.addListener(scheduleClose);
  }

  @override
  void didUpdateWidget(covariant _DockVolume oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (portal.isShowing && (!widget.active || route != widget.core.route)) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) dismiss(immediate: true);
      });
    }
    route = widget.core.route;
  }

  @override
  void didChangeMetrics() => dismiss(immediate: true);

  KeyEventResult handleKey(KeyEvent event) {
    if (event is KeyDownEvent &&
        event.logicalKey == LogicalKeyboardKey.escape &&
        ModalRoute.of(context)?.isCurrent == true &&
        portal.isShowing) {
      dismiss(restoreFocus: true);
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  void show({bool focusSlider = false}) {
    if (!widget.active) return;
    closeTimer?.cancel();
    if (!portal.isShowing) portal.show();
    if (MediaQuery.disableAnimationsOf(context)) {
      reveal.value = 1;
    } else if (reveal.status != AnimationStatus.completed &&
        reveal.status != AnimationStatus.forward) {
      reveal.forward();
    }
    if (focusSlider) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && portal.isShowing) sliderFocus.requestFocus();
      });
    }
  }

  void scheduleClose() {
    closeTimer?.cancel();
    if (!portal.isShowing) return;
    if (overButton || overPanel || dragging || sliderFocus.hasFocus) return;
    closeTimer = Timer(Anicry.reveal, () {
      if (mounted) dismiss();
    });
  }

  void dismiss({bool restoreFocus = false, bool immediate = false}) {
    closeTimer?.cancel();
    if (!portal.isShowing) return;
    overPanel = false;
    dragging = false;
    if (sliderFocus.hasFocus) {
      if (restoreFocus) {
        buttonFocus.requestFocus();
      } else {
        sliderFocus.unfocus();
      }
    }
    if (immediate || MediaQuery.disableAnimationsOf(context)) {
      reveal.value = 0;
      portal.hide();
    } else {
      reveal.reverse();
    }
  }

  @override
  void dispose() {
    closeTimer?.cancel();
    FocusManager.instance.removeEarlyKeyEventHandler(handleKey);
    WidgetsBinding.instance.removeObserver(this);
    sliderFocus.removeListener(scheduleClose);
    sliderFocus.dispose();
    buttonFocus.dispose();
    reveal.dispose();
    super.dispose();
  }

  Widget panel(BuildContext context) {
    final render = anchor.currentContext!.findRenderObject() as RenderBox;
    final point = render.localToGlobal(Offset.zero);
    final media = MediaQuery.of(context);
    final left = (point.dx + (render.size.width - 56) / 2).clamp(
      media.padding.left + 12,
      media.size.width - media.padding.right - 68,
    );
    final curve = reveal.drive(CurveTween(curve: Anicry.easeOut));
    return Positioned(
      left: left,
      bottom: media.size.height - point.dy + 12,
      width: 56,
      child: TapRegion(
        groupId: tapGroup,
        child: MouseRegion(
          onEnter: (_) {
            overPanel = true;
            closeTimer?.cancel();
          },
          onExit: (_) {
            overPanel = false;
            scheduleClose();
          },
          child: Listener(
            onPointerDown: (_) => dragging = true,
            onPointerUp: (_) {
              dragging = false;
              scheduleClose();
            },
            onPointerCancel: (_) {
              dragging = false;
              scheduleClose();
            },
            child: CallbackShortcuts(
              bindings: {
                const SingleActivator(LogicalKeyboardKey.escape): () =>
                    dismiss(restoreFocus: true),
              },
              child: FadeTransition(
                opacity: curve,
                child: SlideTransition(
                  position: Tween(
                    begin: const Offset(0, .06),
                    end: Offset.zero,
                  ).animate(curve),
                  child: Material(
                    color: context.paints.surface,
                    borderRadius: BorderRadius.circular(
                      _DockLayout.radiusFor('toggle'),
                    ),
                    clipBehavior: Clip.antiAlias,
                    child: ConstrainedBox(
                      constraints: BoxConstraints(
                        maxHeight: math.max(
                          0,
                          point.dy - media.padding.top - 24,
                        ),
                      ),
                      child: SingleChildScrollView(
                        child: ValueListenableBuilder<Json>(
                          valueListenable: widget.core.playback,
                          builder: (context, pb, _) {
                            final volume =
                                (pb['volume'] as num?)?.toDouble() ?? .8;
                            return Padding(
                              key: const ValueKey('dock-volume-popover'),
                              padding: const EdgeInsets.symmetric(vertical: 8),
                              child: Column(
                                mainAxisSize: MainAxisSize.min,
                                children: [
                                  Padding(
                                    padding: const EdgeInsets.symmetric(
                                      vertical: 4,
                                    ),
                                    child: Text(
                                      '${(volume * 100).round()}',
                                      semanticsLabel:
                                          'Гучність ${(volume * 100).round()}%',
                                      style: const TextStyle(fontSize: 11),
                                    ),
                                  ),
                                  SizedBox(
                                    key: const ValueKey('dock-volume-slider'),
                                    width: 56,
                                    height: 144,
                                    child: RotatedBox(
                                      quarterTurns: 3,
                                      child: Directionality(
                                        textDirection: TextDirection.ltr,
                                        child: MediaSlider(
                                          value: volume,
                                          focusNode: sliderFocus,
                                          continuous: true,
                                          onChanged: widget.core.setVolume,
                                          semanticFormatterCallback: (v) =>
                                              'Гучність ${(v * 100).round()}%',
                                        ),
                                      ),
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
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) => OverlayPortal(
    controller: portal,
    overlayChildBuilder: panel,
    child: TapRegion(
      groupId: tapGroup,
      onTapOutside: (_) => dismiss(),
      child: MouseRegion(
        onEnter: (_) {
          overButton = true;
          show();
        },
        onExit: (_) {
          overButton = false;
          scheduleClose();
        },
        child: CallbackShortcuts(
          bindings: {
            const SingleActivator(LogicalKeyboardKey.arrowUp): () =>
                show(focusSlider: true),
            const SingleActivator(LogicalKeyboardKey.arrowDown): () =>
                show(focusSlider: true),
          },
          child: SizedBox(
            key: anchor,
            child: ValueListenableBuilder<Json>(
              valueListenable: widget.core.playback,
              builder: (context, pb, _) {
                final volume = (pb['volume'] as num?)?.toDouble() ?? .8;
                return TooltipVisibility(
                  visible: false,
                  child: _DockButton(
                    volume == 0 ? 'volume-x' : 'volume-2',
                    '${volume == 0 ? 'Увімкнути звук' : 'Вимкнути звук'} · ${(volume * 100).round()}%',
                    focusNode: buttonFocus,
                    onTap: () => widget.core.player('mute'),
                  ),
                );
              },
            ),
          ),
        ),
      ),
    ),
  );
}

class _DockTrack extends StatelessWidget {
  const _DockTrack(this.core, {required this.onExpand});
  final Core core;
  final VoidCallback onExpand;

  Future<void> preview(BuildContext anchor) async {
    var expandAfterClose = false;
    await showPlayerPopover(
      anchor,
      width: 320,
      label: 'Закрити прев’ю пісні',
      child: ValueListenableBuilder<Json>(
        valueListenable: core.playback,
        builder: (context, playback, _) {
          final track = object(playback['track']);
          final duration = (playback['duration'] as num?)?.toDouble() ?? 0;
          return Padding(
            key: const ValueKey('dock-track-preview'),
            padding: const EdgeInsets.all(16),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            trackTitle(track),
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(
                              fontSize: 14,
                              fontWeight: FontWeight.w600,
                            ),
                          ),
                          const SizedBox(height: 4),
                          Text(
                            playback['loading'] == true
                                ? 'Завантажуємо…'
                                : trackArtist(track),
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              fontSize: 12,
                              color: context.paints.muted,
                            ),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(width: 8),
                    Control(
                      'expand',
                      'Відкрити пісню та чергу',
                      size: 44,
                      onTap: () {
                        expandAfterClose = true;
                        Navigator.pop(context);
                      },
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                PlaybackTimeline(
                  key: ValueKey(trackKey(track)),
                  position: (playback['position'] as num?)?.toDouble() ?? 0,
                  duration: duration,
                  onSeek: duration > 0
                      ? (value) =>
                            core.command({'type': 'seek', 'value': value})
                      : null,
                ),
              ],
            ),
          );
        },
      ),
    );
    if (expandAfterClose && anchor.mounted) onExpand();
  }

  @override
  Widget build(BuildContext context) {
    final track = core.current;
    final radius = _DockLayout.radiusFor('track');
    return Tooltip(
      message: 'Потягни вгору, щоб відкрити пісню та чергу',
      child: Semantics(
        button: true,
        label: 'Зараз звучить: ${trackTitle(track)}. Прев’ю пісні',
        child: AnicryTap(
          onTap: () => preview(context),
          radius: radius,
          focusColor: context.paints.accent,
          child: Padding(
            padding: const EdgeInsets.fromLTRB(6, 4, 12, 4),
            child: Row(
              children: [
                Artwork(
                  trackCover(track),
                  size: 44,
                  radius: SoundFlowCorners.inner(radius, 6),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      Text(
                        trackTitle(track),
                        key: const ValueKey('dock-track-title'),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(
                          fontSize: 13,
                          fontWeight: FontWeight.w600,
                          height: 1,
                        ),
                      ),
                      if (MediaQuery.textScalerOf(context).scale(13) <= 17)
                        const SizedBox(height: 3),
                      Text(
                        core.playback.value['loading'] == true
                            ? 'Завантажуємо…'
                            : trackArtist(track),
                        key: const ValueKey('dock-track-artist'),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          fontSize: 11,
                          height: 1,
                          color: context.paints.muted,
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
    );
  }
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
              style: const TextStyle(
                fontFamily: SoundFlowType.heading,
                fontSize: 18,
                fontWeight: FontWeight.w600,
              ),
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
            fontFamily: SoundFlowType.heading,
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
