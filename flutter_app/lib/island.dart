import 'dart:async';
import 'dart:io';
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';

import 'anicry_motion.dart';
import 'core.dart';
import 'concentric.dart';
import 'design.dart';
import 'media_slider.dart';

class IslandApp extends StatefulWidget {
  const IslandApp({super.key, this.exportPath});
  final String? exportPath;
  @override
  State<IslandApp> createState() => _IslandAppState();
}

class _IslandAppState extends State<IslandApp>
    with SingleTickerProviderStateMixin {
  final playback = ValueNotifier<Json>({});
  final boundaryKey = GlobalKey();
  bool exported = false;
  bool hover = false,
      expanded = false,
      reduced = false,
      visible = false,
      keyboard = false;
  Timer? idle, dismissal, hoverDelay;
  late final expansion = AnimationController(
    vsync: this,
    duration: Anicry.layout,
  );
  Animation<double> get curve => expansion;

  @override
  void initState() {
    super.initState();
    expansion.addListener(syncContour);
    Core.window.setMethodCallHandler((call) async {
      if (call.method == 'state') {
        final data = object(call.arguments);
        playback.value = object(data['player']);
        if (reduced != (data['reduced'] == true)) {
          setState(() => reduced = data['reduced'] == true);
        }
      } else if (call.method == 'shown') {
        dismissal?.cancel();
        if (!visible) expansion.value = 0;
        setState(() {
          visible = true;
          keyboard = call.arguments == true;
        });
        setExpanded(keyboard);
        if (!exported && widget.exportPath != null) {
          exported = true;
          Future<void>.delayed(const Duration(milliseconds: 700), () async {
            await exportFrame('expanded');
            setExpanded(false);
            await Future<void>.delayed(const Duration(milliseconds: 700));
            await exportFrame('compact');
            await Core.window.invokeMethod('hide-island');
            await Future<void>.delayed(const Duration(milliseconds: 250));
            await exportFrame('desktop');
          });
        }
      } else if (call.method == 'desktop') {
        dismissal?.cancel();
        idle?.cancel();
        hoverDelay?.cancel();
        setState(() {
          visible = true;
          expanded = false;
          keyboard = false;
          hover = false;
        });
        expansion.value = 0;
        syncContour();
      } else if (call.method == 'hidden') {
        setState(() {
          visible = false;
          expanded = false;
        });
        idle?.cancel();
        hoverDelay?.cancel();
        expansion.value = 0;
      } else if (call.method == 'dismiss') {
        hide();
      }
    });
  }

  Future<void> exportFrame(String label) async {
    final boundary =
        boundaryKey.currentContext?.findRenderObject()
            as RenderRepaintBoundary?;
    if (boundary == null || !mounted) return;
    final frame = await Core.window.invokeMapMethod<String, dynamic>(
      'island-snapshot',
    );
    if (frame == null || !mounted) return;
    final image = await boundary.toImage(
      pixelRatio: MediaQuery.devicePixelRatioOf(boundaryKey.currentContext!),
    );
    final pixels = await image.toByteData(format: ui.ImageByteFormat.rawRgba);
    final mask = frame['bgra'] as Uint8List?;
    if (pixels != null && mask != null && pixels.lengthInBytes == mask.length) {
      final rgba = pixels.buffer.asUint8List();
      for (var i = 3; i < rgba.length; i += 4) {
        rgba[i] = mask[i];
      }
      final done = Completer<ui.Image>();
      ui.decodeImageFromPixels(
        rgba,
        image.width,
        image.height,
        ui.PixelFormat.rgba8888,
        done.complete,
      );
      final result = await done.future;
      final png = await result.toByteData(format: ui.ImageByteFormat.png);
      if (png != null) {
        final file = File('${widget.exportPath}-$label.png');
        await file.parent.create(recursive: true);
        await file.writeAsBytes(png.buffer.asUint8List());
        debugPrint(
          'Island $label: ${image.width}x${image.height}, anchor ${frame['x']},${frame['y']}, alpha composed ${frame['composed']}, desktop ${frame['desktop']}, visible ${frame['visible']}, topmost ${frame['topmost']}, below foreground ${frame['belowForeground']}, foreground hook ${frame['foregroundHook']}',
        );
      }
      result.dispose();
    }
    image.dispose();
  }

  void syncContour() => Core.window.invokeMethod('island-shape', curve.value);
  void setExpanded(bool value) {
    if (!mounted) return;
    setState(() => expanded = value);
    if (reduced) {
      expansion.value = value ? 1 : 0;
    } else {
      expansion.animateTo(
        value ? 1 : 0,
        duration: value ? Anicry.layout : const Duration(milliseconds: 180),
        curve: Anicry.easeOut,
      );
    }
    syncContour();
    resetIdle();
  }

  void resetIdle() {
    idle?.cancel();
    if (!hover && expanded) {
      idle = Timer(
        const Duration(milliseconds: 1500),
        () => setExpanded(false),
      );
    }
  }

  void hide() {
    idle?.cancel();
    hoverDelay?.cancel();
    dismissal?.cancel();
    setExpanded(false);
    dismissal = Timer(
      Duration(milliseconds: reduced ? 0 : 180),
      () => Core.window.invokeMethod('hide-island'),
    );
  }

  void send(Json command) {
    Core.window.invokeMethod('island-command', command);
    resetIdle();
  }

  void player(String action) => send({'type': 'player', 'action': action});
  void openMain() {
    send({
      'type': 'navigate',
      'view': object(playback.value['track']).isEmpty ? 'search' : 'now',
    });
    Core.window.invokeMethod('show-main');
  }

  @override
  void dispose() {
    idle?.cancel();
    dismissal?.cancel();
    hoverDelay?.cancel();
    expansion.removeListener(syncContour);
    expansion.dispose();
    playback.dispose();
    super.dispose();
  }

  Widget cover(Json track, double size, double progress) {
    final radius =
        size / 2 + (SoundFlowCorners.inner(28, 14) - size / 2) * progress;
    return AnicryTap(
      radius: radius,
      onTap: expanded ? openMain : () => setExpanded(true),
      child: track.isNotEmpty
          ? Artwork(trackCover(track), size: size, radius: radius)
          : Container(
              width: size,
              height: size,
              decoration: BoxDecoration(
                color: const Color(0xff1d1d1d),
                borderRadius: BorderRadius.circular(radius),
              ),
              child: Center(child: Glyph('music', size: size * .6)),
            ),
    );
  }

  @override
  Widget build(BuildContext context) => MaterialApp(
    debugShowCheckedModeBanner: false,
    scrollBehavior: const MaterialScrollBehavior().copyWith(scrollbars: false),
    theme: soundflowTheme(true, reduced),
    builder: (context, child) => MediaQuery(
      data: MediaQuery.of(context).copyWith(
        disableAnimations: reduced || MediaQuery.disableAnimationsOf(context),
      ),
      child: child!,
    ),
    home: CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.escape): hide,
        const SingleActivator(LogicalKeyboardKey.space): () => player('toggle'),
        const SingleActivator(LogicalKeyboardKey.arrowRight): () =>
            player('next'),
        const SingleActivator(LogicalKeyboardKey.arrowLeft): () =>
            player('previous'),
        const SingleActivator(LogicalKeyboardKey.arrowDown): () =>
            setExpanded(!expanded),
      },
      child: Focus(
        autofocus: true,
        child: MouseRegion(
          onEnter: (_) {
            hover = true;
            idle?.cancel();
            hoverDelay?.cancel();
            if (!expanded) {
              hoverDelay = Timer(
                const Duration(milliseconds: 160),
                () => setExpanded(true),
              );
            }
          },
          onExit: (_) {
            hover = false;
            hoverDelay?.cancel();
            if (expanded) {
              hoverDelay = Timer(
                const Duration(milliseconds: 120),
                () => setExpanded(false),
              );
            }
          },
          child: RepaintBoundary(
            key: boundaryKey,
            child: Scaffold(
              backgroundColor: Colors.black,
              body: LayoutBuilder(
                builder: (context, bounds) {
                  final small = bounds.maxWidth < 300;
                  final compactWidth = small ? 136.0 : 144.0;
                  final compactHeight = small ? 32.0 : 34.0;
                  final artSize = small ? 40.0 : 44.0;
                  return ValueListenableBuilder<Json>(
                    valueListenable: playback,
                    builder: (context, pb, _) {
                      final track = object(pb['track']),
                          hasTrack = track.isNotEmpty;
                      final playing = pb['playing'] == true;
                      return AnimatedBuilder(
                        animation: curve,
                        builder: (context, _) {
                          final t = curve.value;
                          final compactLeft =
                              (bounds.maxWidth - compactWidth) / 2;
                          final coverSize = 24 + (artSize - 24) * t;
                          return Stack(
                            children: [
                              Positioned(
                                left: compactLeft + 7,
                                top: 0,
                                width: compactWidth - 14,
                                height: compactHeight,
                                child: IgnorePointer(
                                  ignoring: t > .2,
                                  child: GestureDetector(
                                    behavior: HitTestBehavior.opaque,
                                    onTap: () => setExpanded(true),
                                  ),
                                ),
                              ),
                              Positioned.fill(
                                child: IgnorePointer(
                                  ignoring: t < .8,
                                  child: Opacity(
                                    opacity: ((t - .25) / .65).clamp(0, 1),
                                    child: Padding(
                                      padding: const EdgeInsets.fromLTRB(
                                        16,
                                        14,
                                        16,
                                        10,
                                      ),
                                      child: Column(
                                        children: [
                                          SizedBox(
                                            height: artSize,
                                            child: Padding(
                                              padding: EdgeInsets.only(
                                                left: artSize + 11,
                                                right: 32,
                                              ),
                                              child: Align(
                                                alignment: Alignment.centerLeft,
                                                child: AnicryTap(
                                                  onTap: openMain,
                                                  child: Column(
                                                    mainAxisSize:
                                                        MainAxisSize.min,
                                                    crossAxisAlignment:
                                                        CrossAxisAlignment
                                                            .start,
                                                    children: [
                                                      Text(
                                                        hasTrack
                                                            ? trackTitle(track)
                                                            : 'Обери музику',
                                                        maxLines: 1,
                                                        overflow: TextOverflow
                                                            .ellipsis,
                                                        style: const TextStyle(
                                                          fontSize: 13,
                                                          height: 1.25,
                                                          fontWeight:
                                                              FontWeight.w600,
                                                        ),
                                                      ),
                                                      const SizedBox(height: 3),
                                                      Text(
                                                        hasTrack
                                                            ? trackArtist(track)
                                                            : 'Відкрити пошук',
                                                        maxLines: 1,
                                                        overflow: TextOverflow
                                                            .ellipsis,
                                                        style: const TextStyle(
                                                          fontSize: 11.5,
                                                          height: 1.25,
                                                          color: Color(
                                                            0xffa7a7a7,
                                                          ),
                                                        ),
                                                      ),
                                                    ],
                                                  ),
                                                ),
                                              ),
                                            ),
                                          ),
                                          const SizedBox(height: 8),
                                          PlaybackTimeline(
                                            position:
                                                (pb['position'] as num?)
                                                    ?.toDouble() ??
                                                0,
                                            duration:
                                                (pb['duration'] as num?)
                                                    ?.toDouble() ??
                                                0,
                                            inline: true,
                                            fontSize: 9,
                                            onSeek: hasTrack
                                                ? (v) => send({
                                                    'type': 'seek',
                                                    'value': v,
                                                  })
                                                : null,
                                          ),
                                          const Spacer(),
                                          Row(
                                            mainAxisAlignment:
                                                MainAxisAlignment.spaceBetween,
                                            children: [
                                              Control(
                                                'heart',
                                                'Улюблене',
                                                size: 30,
                                                glyphSize: 17,
                                                active: pb['liked'] == true,
                                                onTap: hasTrack
                                                    ? () => send({
                                                        'type': 'action',
                                                        'action': 'like',
                                                        'payload': track,
                                                      })
                                                    : null,
                                              ),
                                              Row(
                                                mainAxisSize: MainAxisSize.min,
                                                children: [
                                                  Control(
                                                    'skip-back',
                                                    'Попередній трек',
                                                    size: 36,
                                                    glyphSize: 22,
                                                    onTap: hasTrack
                                                        ? () =>
                                                              player('previous')
                                                        : null,
                                                  ),
                                                  const SizedBox(width: 8),
                                                  Control(
                                                    playing ? 'pause' : 'play',
                                                    playing
                                                        ? 'Пауза'
                                                        : 'Відтворити',
                                                    size: 40,
                                                    glyphSize: 27,
                                                    onTap: hasTrack
                                                        ? () => player('toggle')
                                                        : null,
                                                  ),
                                                  const SizedBox(width: 8),
                                                  Control(
                                                    'skip-forward',
                                                    'Наступний трек',
                                                    size: 36,
                                                    glyphSize: 22,
                                                    onTap: hasTrack
                                                        ? () => player('next')
                                                        : null,
                                                  ),
                                                ],
                                              ),
                                              Control(
                                                (pb['volume'] as num?) == 0
                                                    ? 'volume-x'
                                                    : 'volume-2',
                                                'Вимкнути звук',
                                                size: 30,
                                                glyphSize: 17,
                                                onTap: hasTrack
                                                    ? () => player('mute')
                                                    : null,
                                              ),
                                            ],
                                          ),
                                        ],
                                      ),
                                    ),
                                  ),
                                ),
                              ),
                              Positioned(
                                left:
                                    compactLeft +
                                    4 +
                                    (16 - compactLeft - 4) * t,
                                top: 4 + 10 * t,
                                child: cover(track, coverSize, t),
                              ),
                              Positioned(
                                left:
                                    compactLeft +
                                    compactWidth -
                                    27 +
                                    (bounds.maxWidth -
                                            38 -
                                            compactLeft -
                                            compactWidth +
                                            27) *
                                        t,
                                top:
                                    (compactHeight - 22) / 2 +
                                    (14 +
                                            (artSize - 22) / 2 -
                                            (compactHeight - 22) / 2) *
                                        t,
                                child: IgnorePointer(
                                  child: PlaybackPulse(
                                    playing: playing && visible,
                                    reduced: reduced,
                                    size: 22,
                                  ),
                                ),
                              ),
                            ],
                          );
                        },
                      );
                    },
                  );
                },
              ),
            ),
          ),
        ),
      ),
    ),
  );
}

class PlaybackPulse extends StatefulWidget {
  const PlaybackPulse({
    super.key,
    required this.playing,
    required this.reduced,
    this.size = 24,
  });
  final bool playing, reduced;
  final double size;
  @override
  State<PlaybackPulse> createState() => _PlaybackPulseState();
}

class _PlaybackPulseState extends State<PlaybackPulse>
    with SingleTickerProviderStateMixin {
  late final phase = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 1200),
  );
  void sync() {
    if (widget.playing && !widget.reduced) {
      phase.repeat();
    } else {
      phase.stop();
      phase.value = 0;
    }
  }

  @override
  void initState() {
    super.initState();
    sync();
  }

  @override
  void didUpdateWidget(covariant PlaybackPulse old) {
    super.didUpdateWidget(old);
    if (old.playing != widget.playing || old.reduced != widget.reduced) sync();
  }

  @override
  void dispose() {
    phase.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Semantics(
    label: widget.playing ? 'Відтворюється' : 'Пауза',
    child: RepaintBoundary(
      child: SizedBox.square(
        dimension: widget.size,
        child: CustomPaint(painter: _PulsePainter(phase, widget.playing)),
      ),
    ),
  );
}

class _PulsePainter extends CustomPainter {
  _PulsePainter(this.phase, this.playing) : super(repaint: phase);
  final Animation<double> phase;
  final bool playing;
  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = const Color(0xffc7d4c4)
      ..strokeWidth = 2.2
      ..strokeCap = StrokeCap.round;
    for (var i = 0; i < 5; i++) {
      final height = playing
          ? 4 +
                (math.sin(phase.value * math.pi * 2 + i * 1.7) + 1) *
                    (i == 2 ? 7 : 5)
          : 3.0;
      final x = 3 + i * (size.width - 6) / 4;
      canvas.drawLine(
        Offset(x, (size.height - height) / 2),
        Offset(x, (size.height + height) / 2),
        paint,
      );
    }
  }

  @override
  bool shouldRepaint(covariant _PulsePainter old) =>
      old.playing != playing || old.phase != phase;
}
