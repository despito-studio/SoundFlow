import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:soundflow/anicry_motion.dart';
import 'package:soundflow/core.dart';
import 'package:soundflow/design.dart';
import 'package:soundflow/main.dart';
import 'package:soundflow/media_slider.dart';
import 'package:soundflow/player_dock.dart';
import 'package:soundflow/search_page.dart';
import 'package:soundflow/settings_page.dart';

class PreviewCore extends Core {
  PreviewCore({String scheme = 'dark'}) : super([]) {
    ready = true;
    settings = {
      'theme': scheme,
      'shuffle': false,
      'repeat': 'off',
      'desktop_icon': 'default',
    };
    final covers = '${Directory.current.parent.path}/resources/covers';
    tracksFixture = [
      for (var i = 0; i < 8; i++)
        {
          'id': 'preview-$i',
          'source': i.isEven ? 'soundcloud' : 'youtube',
          'title': [
            'Теплий вечір',
            'Night Drive',
            'Останній літній день',
            'Everything in Motion',
            'Тихо між нами',
            'Blue Moon',
            'Далі буде',
            'After Hours',
          ][i],
          'artist': [
            'Один в каное',
            'Kori',
            'The Hardwave',
            'BUDDHA OCEANS',
          ][i % 4],
          'thumbnail': '$covers/$i.jpg',
          'duration': 225,
          'duration_formatted': '3:45',
        },
    ];
    state = {
      'revision': 1,
      'view': 'search',
      'search_query': '',
      'search_source': 'all',
      'search_loading': false,
      'search_error': '',
      'search': <Json>[],
      'queue': tracksFixture,
      'queue_index': 0,
      'library': {
        'history': tracksFixture,
        'liked_tracks': [tracksFixture.first],
        'search_history': ['Один в каное', 'нічна музика', 'Kori', 'Blue Moon'],
        'profile': {'name': 'Слухач'},
        'playlists': {
          for (var i = 0; i < 3; i++)
            'list-$i': {
              'id': 'list-$i',
              'name': [
                'Для нічних прогулянок',
                'На повторі',
                'Теплі спогади',
              ][i],
              'tracks': tracksFixture.skip(i).take(4).toList(),
            },
        },
      },
    };
    route = 'search';
    playback.value = {
      'track': tracksFixture.first,
      'playing': false,
      'duration': 225,
      'position': 84,
      'volume': .6,
    };
  }

  late final List<Json> tracksFixture;
  final commands = <Json>[];
  double unmutedVolume = .6;

  @override
  void command(Json value) {
    commands.add(value);
    if (value['type'] == 'search') {
      state = {
        ...state,
        'revision': (state['revision'] as int) + 1,
        'search_query': value['query'],
        'search_source': value['source'],
        'search_loading': false,
        'search': value['query'] == '' ? <Json>[] : tracksFixture,
      };
    }
    if (value['type'] == 'settings') {
      settings = {...settings, ...object(value['patch'])};
    }
    if (value['action'] == 'mute') {
      final volume = playback.value['volume'] as double;
      if (volume > 0) unmutedVolume = volume;
      playback.value = {
        ...playback.value,
        'volume': volume > 0 ? 0.0 : unmutedVolume,
      };
    }
    notifyListeners();
  }

  @override
  void setVolume(double value) {
    commands.add({'type': 'volume', 'value': value});
    playback.value = {...playback.value, 'volume': value};
  }

  @override
  void dispose() {
    ready = false;
    super.dispose();
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    await Anicry.load();
    for (final family in ['Onest', 'Montserrat']) {
      final fonts = FontLoader(family);
      final faces = family == 'Onest'
          ? ['Regular', 'SemiBold']
          : ['SemiBold', 'Bold'];
      for (final face in faces) {
        fonts.addFont(
          Future.value(
            ByteData.sublistView(
              File('assets/fonts/$family-$face.ttf').readAsBytesSync(),
            ),
          ),
        );
      }
      await fonts.load();
    }
  });

  Future<void> mount(
    WidgetTester tester,
    PreviewCore core,
    Widget child, {
    Size size = const Size(1366, 715),
    double scale = 1,
    bool reduced = false,
    GlobalKey? boundary,
  }) async {
    tester.view.physicalSize = size;
    tester.view.devicePixelRatio = 1;
    await tester.pumpWidget(
      RepaintBoundary(
        key: boundary,
        child: ListenableBuilder(
          listenable: core,
          builder: (context, _) => MaterialApp(
            theme: soundflowTheme(core.dark, reduced, scheme: core.scheme),
            debugShowCheckedModeBanner: false,
            scrollBehavior: const MaterialScrollBehavior().copyWith(
              scrollbars: false,
            ),
            builder: (context, child) => MediaQuery(
              data: MediaQuery.of(context).copyWith(
                textScaler: TextScaler.linear(scale),
                disableAnimations: reduced,
              ),
              child: child!,
            ),
            home: child is Workspace ? Workspace(core) : child,
          ),
        ),
      ),
    );
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 120)),
    );
    await tester.pumpAndSettle();
  }

  Future<void> unmount(WidgetTester tester, PreviewCore core) async {
    await tester.pumpWidget(const SizedBox());
    core.dispose();
    tester.view.resetPhysicalSize();
    tester.view.resetDevicePixelRatio();
  }

  Future<TestGesture> hoverVolume(WidgetTester tester) async {
    final mouse = await tester.createGesture(kind: ui.PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    await mouse.moveTo(
      tester.getCenter(find.byKey(const ValueKey('dock-volume'))),
    );
    await tester.pump();
    return mouse;
  }

  testWidgets(
    'settings and search adapt to narrow, wide and enlarged text layouts',
    (tester) async {
      for (final width in [780.0, 1366.0]) {
        for (final scheme in ['dark', 'light']) {
          final core = PreviewCore(scheme: scheme);
          for (final scale in [1.0, 1.4]) {
            await mount(
              tester,
              core,
              Scaffold(
                body: ListView(
                  padding: const EdgeInsets.all(24),
                  children: [SettingsPage(core)],
                ),
              ),
              size: Size(width, 715),
              scale: scale,
            );
            expect(
              tester.takeException(),
              isNull,
              reason: 'Settings $width $scheme $scale',
            );
            await tester.pumpWidget(const SizedBox());
            final query = TextEditingController();
            final focus = FocusNode();
            await mount(
              tester,
              core,
              Scaffold(
                body: ListView(
                  padding: const EdgeInsets.all(24),
                  children: [
                    SearchPage(
                      core,
                      controller: query,
                      focusNode: focus,
                      onAddPlaylist: (_) {},
                    ),
                  ],
                ),
              ),
              size: Size(width, 715),
              scale: scale,
            );
            expect(
              tester.takeException(),
              isNull,
              reason: 'Search $width $scheme $scale',
            );
            await tester.pumpWidget(const SizedBox());
            query.dispose();
            focus.dispose();
          }
          await unmount(tester, core);
        }
      }
    },
  );

  testWidgets(
    'search supports history, source changes and honest Spotify validation',
    (tester) async {
      final core = PreviewCore();
      final query = TextEditingController(), focus = FocusNode();
      await mount(
        tester,
        core,
        Scaffold(
          body: ListView(
            padding: const EdgeInsets.all(24),
            children: [
              SearchPage(
                core,
                controller: query,
                focusNode: focus,
                onAddPlaylist: (_) {},
              ),
            ],
          ),
        ),
      );
      await tester.tap(find.text('нічна музика'));
      await tester.pumpAndSettle();
      expect(core.commands.last['query'], 'нічна музика');
      expect(find.text('Найкращий збіг'), findsOneWidget);
      await tester.tap(find.text('Spotify · посилання'));
      await tester.pumpAndSettle();
      final sent = core.commands.length;
      await tester.tap(find.text('Знайти'));
      await tester.pumpAndSettle();
      expect(core.commands.length, sent);
      expect(
        find.textContaining('Встав посилання на трек Spotify'),
        findsOneWidget,
      );
      await tester.enterText(
        find.byType(TextField),
        'https://open.spotify.com/track/example',
      );
      await tester.testTextInput.receiveAction(TextInputAction.search);
      await tester.pumpAndSettle();
      expect(core.commands.last['source'], 'spotify');
      expect(
        core.commands.last['query'],
        'https://open.spotify.com/track/example',
      );
      await tester.tap(find.byTooltip('Очистити пошук'));
      await tester.pumpAndSettle();
      expect(query.text, isEmpty);
      expect(find.text('Найкращий збіг'), findsNothing);
      await unmount(tester, core);
      query.dispose();
      focus.dispose();
    },
  );

  testWidgets('unrelated backend jobs do not show search progress on landing', (
    tester,
  ) async {
    final core = PreviewCore();
    core.state['pending'] = 3;
    final query = TextEditingController(), focus = FocusNode();
    await mount(
      tester,
      core,
      Scaffold(
        body: ListView(
          children: [
            SearchPage(
              core,
              controller: query,
              focusNode: focus,
              onAddPlaylist: (_) {},
            ),
          ],
        ),
      ),
    );
    expect(find.byType(LinearProgressIndicator), findsNothing);
    expect(find.text('Останні запити'), findsOneWidget);
    await unmount(tester, core);
    query.dispose();
    focus.dispose();
  });

  testWidgets(
    'dock swipes open, dispatches absolute queue index and collapses with Escape',
    (tester) async {
      for (final width in [780.0, 1366.0]) {
        final core = PreviewCore();
        // Duplicate occurrences must keep their absolute position in the queue.
        core.state['queue'] = [
          core.tracksFixture[0],
          core.tracksFixture[1],
          core.tracksFixture[1],
          core.tracksFixture[2],
        ];
        core.state['queue_index'] = 1;
        core.playback.value = {
          ...core.playback.value,
          'track': core.tracksFixture[1],
          'playing': true,
        };
        await mount(
          tester,
          core,
          Scaffold(body: PlayerDock(core, onAddPlaylist: (_) {})),
          size: Size(width, 715),
        );
        expect(tester.takeException(), isNull);
        await tester.drag(
          find.byTooltip('Потягни вгору, щоб відкрити пісню та чергу'),
          const Offset(0, -260),
        );
        await tester.pumpAndSettle();
        expect(find.byTooltip('Згорнути плеєр · Escape'), findsOneWidget);
        if (width < 840) {
          await tester.scrollUntilVisible(
            find.text('Далі в черзі'),
            250,
            scrollable: find.descendant(
              of: find.byType(CustomScrollView),
              matching: find.byType(Scrollable),
            ),
          );
        }
        expect(find.text('Далі в черзі'), findsOneWidget);
        expect(tester.takeException(), isNull);
        final row = find.byType(TrackRow).first;
        if (width < 840) await tester.ensureVisible(row);
        await tester.tap(
          find.descendant(of: row, matching: find.text('Night Drive')).first,
        );
        expect(core.commands.last['action'], 'queue_play');
        expect(object(core.commands.last['payload'])['index'], 2);
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pumpAndSettle();
        expect(find.text('Далі в черзі'), findsNothing);
        expect(
          find.byTooltip('Потягни вгору, щоб відкрити пісню та чергу'),
          findsOneWidget,
        );
        expect(tester.takeException(), isNull);
        await unmount(tester, core);
      }
    },
  );

  testWidgets(
    'separate dock buttons act while their gaps pass clicks through',
    (tester) async {
      for (final width in [360.0, 780.0, 1366.0]) {
        final core = PreviewCore();
        var backgroundTaps = 0;
        await mount(
          tester,
          core,
          Scaffold(
            body: Stack(
              children: [
                Positioned.fill(
                  child: GestureDetector(
                    behavior: HitTestBehavior.opaque,
                    onTap: () => backgroundTaps++,
                  ),
                ),
                PlayerDock(core, onAddPlaylist: (_) {}),
              ],
            ),
          ),
          size: Size(width, 715),
          scale: width == 780 ? 2 : 1,
        );
        final previous = tester.getRect(
          find.byKey(const ValueKey('dock-previous')),
        );
        final toggle = tester.getRect(
          find.byKey(const ValueKey('dock-toggle')),
        );
        expect(toggle.width, greaterThan(previous.width));
        for (final id in [
          'track',
          'previous',
          'next',
          'volume',
          'queue',
          if (width >= 780) 'island',
        ]) {
          final rect = tester.getRect(find.byKey(ValueKey('dock-$id')));
          if (id != 'track') {
            expect(rect.width, lessThanOrEqualTo(toggle.width));
          }
          expect(rect.height, lessThanOrEqualTo(toggle.height));
          expect(rect.left, greaterThanOrEqualTo(0));
          expect(rect.right, lessThanOrEqualTo(width));
        }
        final track = tester.getRect(find.byKey(const ValueKey('dock-track')));
        final last = tester.getRect(
          find.byKey(ValueKey(width >= 780 ? 'dock-island' : 'dock-queue')),
        );
        if (width >= 780) {
          final volume = tester.getRect(
            find.byKey(const ValueKey('dock-volume')),
          );
          final next = tester.getRect(find.byKey(const ValueKey('dock-next')));
          expect(track.width, closeTo(last.right - volume.left, .1));
          expect(toggle.center.dx, closeTo(width / 2, .1));
          expect((track.left + last.right) / 2, closeTo(toggle.center.dx, .1));
          expect(
            toggle.center.dx - previous.center.dx,
            closeTo(next.center.dx - toggle.center.dx, .1),
          );
        } else {
          expect(track.width, closeTo(last.right - previous.left, .1));
        }
        expect(find.byKey(const ValueKey('dock-track-title')), findsOneWidget);
        expect(find.byKey(const ValueKey('dock-track-artist')), findsOneWidget);
        expect(find.byType(MediaSlider), findsNothing);
        final gap = Offset(
          (previous.right + toggle.left) / 2,
          previous.center.dy,
        );
        await tester.tapAt(gap);
        expect(backgroundTaps, 1, reason: 'Transparent gap at width $width');
        expect(core.commands, isEmpty);
        for (final entry in {
          'previous': 'previous',
          'toggle': 'toggle',
          'next': 'next',
        }.entries) {
          await tester.tap(find.byKey(ValueKey('dock-${entry.key}')));
          expect(core.commands.last['action'], entry.value);
        }
        expect(
          tester.takeException(),
          isNull,
          reason: 'Dock layout at width $width',
        );
        await unmount(tester, core);
      }
    },
  );

  testWidgets('Onest is body text and Montserrat is limited to headings', (
    tester,
  ) async {
    final core = PreviewCore();
    await mount(tester, core, Workspace(core));
    final theme = Theme.of(tester.element(find.text('Пошук').first));
    expect(theme.textTheme.bodyMedium?.fontFamily, SoundFlowType.body);
    expect(theme.textTheme.titleMedium?.fontFamily, SoundFlowType.body);
    expect(theme.textTheme.titleLarge?.fontFamily, SoundFlowType.heading);
    expect(
      tester.widget<Text>(find.text('Останні запити')).style?.fontFamily,
      SoundFlowType.heading,
    );
    expect(tester.takeException(), isNull);
    await unmount(tester, core);
  });

  testWidgets('track block opens current track and seek above the dock', (
    tester,
  ) async {
    for (final width in [360.0, 780.0, 1366.0]) {
      final core = PreviewCore();
      await mount(
        tester,
        core,
        Scaffold(body: PlayerDock(core, onAddPlaylist: (_) {})),
        size: Size(width, 560),
        scale: width == 780 ? 2 : 1,
      );
      final cover = tester.getRect(find.byKey(const ValueKey('dock-track')));
      await tester.tap(find.byKey(const ValueKey('dock-track')));
      await tester.pumpAndSettle();
      final preview = tester.getRect(
        find.byKey(const ValueKey('dock-track-preview')),
      );
      expect(preview.bottom, lessThan(cover.top));
      expect(preview.left, greaterThanOrEqualTo(12));
      expect(preview.right, lessThanOrEqualTo(width - 12));
      Finder inPreview(Finder child) => find.descendant(
        of: find.byKey(const ValueKey('dock-track-preview')),
        matching: child,
      );
      expect(inPreview(find.text('Теплий вечір')), findsOneWidget);
      expect(inPreview(find.text('Один в каное')), findsOneWidget);
      await tester.tap(find.byType(MediaSlider));
      expect(core.commands.last['type'], 'seek');
      core.playback.value = {
        ...core.playback.value,
        'track': core.tracksFixture[2],
        'loading': true,
        'duration': 0,
      };
      await tester.pumpAndSettle();
      expect(inPreview(find.text('Останній літній день')), findsOneWidget);
      expect(inPreview(find.text('Завантажуємо…')), findsOneWidget);
      expect(tester.widget<Slider>(find.byType(Slider)).onChanged, isNull);
      if (width == 1366) {
        await tester.tap(find.byTooltip('Відкрити пісню та чергу'));
        await tester.pumpAndSettle();
        expect(find.byKey(const ValueKey('dock-track-preview')), findsNothing);
        expect(find.byTooltip('Згорнути плеєр · Escape'), findsOneWidget);
      }
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('dock-track-preview')), findsNothing);
      expect(tester.takeException(), isNull, reason: 'Preview at width $width');
      await unmount(tester, core);
    }
  });

  testWidgets('dock utilities and keyboard work in every theme', (
    tester,
  ) async {
    final windowCalls = <MethodCall>[];
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      Core.window,
      (call) async {
        windowCalls.add(call);
        return null;
      },
    );
    try {
      for (final scheme in ['dark', 'light', 'rose', 'violet', 'ocean']) {
        final core = PreviewCore(scheme: scheme);
        await mount(
          tester,
          core,
          Scaffold(body: PlayerDock(core, onAddPlaylist: (_) {})),
          reduced: scheme == 'ocean',
        );
        final mouse = await hoverVolume(tester);
        expect(
          find.byKey(const ValueKey('dock-volume-popover')),
          findsOneWidget,
        );
        await tester.pumpAndSettle();
        final anchor = tester.getRect(
          find.byKey(const ValueKey('dock-volume')),
        );
        final panel = tester.getRect(
          find.byKey(const ValueKey('dock-volume-popover')),
        );
        final slider = tester.getRect(
          find.byKey(const ValueKey('dock-volume-slider')),
        );
        expect(panel.bottom, lessThan(anchor.top));
        expect(panel.center.dx, closeTo(anchor.center.dx, .1));
        expect(slider.height, greaterThan(slider.width));
        expect(
          find.descendant(
            of: find.byKey(const ValueKey('dock-volume-popover')),
            matching: find.byType(Control),
          ),
          findsNothing,
        );
        await tester.tap(find.byKey(const ValueKey('dock-volume')));
        expect(core.commands.last['action'], 'mute');
        expect(core.playback.value['volume'], 0);
        await tester.pumpAndSettle();
        await tester.tap(find.byKey(const ValueKey('dock-volume')));
        expect(core.playback.value['volume'], .6);
        await tester.pumpAndSettle();
        await tester.dragFrom(
          Offset(slider.center.dx, slider.bottom - 28),
          const Offset(0, -80),
        );
        await tester.pumpAndSettle();
        expect(core.commands.last['type'], 'volume');
        expect(core.commands.last['value'], greaterThan(.6));
        tester.widget<Slider>(find.byType(Slider)).focusNode!.requestFocus();
        await tester.pump();
        expect(
          FocusManager.instance.primaryFocus?.context
              ?.findAncestorWidgetOfExactType<Slider>(),
          isNotNull,
        );
        final beforeKey = core.playback.value['volume'] as double;
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
        await tester.pumpAndSettle();
        expect(core.playback.value['volume'], lessThan(beforeKey));
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pumpAndSettle();
        expect(find.byKey(const ValueKey('dock-volume-popover')), findsNothing);
        expect(
          tester
              .widget<AnicryTap>(
                find.descendant(
                  of: find.byKey(const ValueKey('dock-volume')),
                  matching: find.byType(AnicryTap),
                ),
              )
              .focusNode!
              .hasFocus,
          isTrue,
        );
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
        await tester.pumpAndSettle();
        expect(
          find.byKey(const ValueKey('dock-volume-popover')),
          findsOneWidget,
        );
        expect(
          tester.widget<Slider>(find.byType(Slider)).focusNode!.hasFocus,
          isTrue,
        );
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pumpAndSettle();
        await mouse.removePointer();
        await tester.tap(find.byKey(const ValueKey('dock-queue')));
        await tester.pumpAndSettle();
        expect(find.text('Зараз звучить'), findsOneWidget);
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pumpAndSettle();
        expect(
          tester
              .widget<AnicryTap>(
                find.descendant(
                  of: find.byKey(const ValueKey('dock-volume')),
                  matching: find.byType(AnicryTap),
                ),
              )
              .focusNode!
              .hasFocus,
          isTrue,
        );
        await tester.tap(find.byKey(const ValueKey('dock-island')));
        expect(windowCalls.last.method, 'toggle-island');
        expect(windowCalls.last.arguments, false);
        await tester.tap(find.byKey(const ValueKey('dock-track')));
        await tester.pumpAndSettle();
        await tester.tap(find.byType(MediaSlider).first);
        expect(core.commands.last['type'], 'seek');
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pumpAndSettle();
        FocusManager.instance.primaryFocus?.unfocus();
        var keyboardActivated = false;
        for (var i = 0; i < 12; i++) {
          await tester.sendKeyEvent(LogicalKeyboardKey.tab);
          await tester.pump();
          final label = FocusManager.instance.primaryFocus?.context
              ?.findAncestorWidgetOfExactType<Tooltip>()
              ?.message;
          if (label == 'Попередній трек') {
            await tester.sendKeyEvent(LogicalKeyboardKey.space);
            expect(core.commands.last['action'], 'previous');
            keyboardActivated = true;
            break;
          }
        }
        expect(keyboardActivated, isTrue, reason: scheme);
        expect(tester.takeException(), isNull, reason: scheme);
        await unmount(tester, core);
      }
    } finally {
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        Core.window,
        null,
      );
    }
  });

  testWidgets('dock popovers dismiss on resize and reopen at the new anchor', (
    tester,
  ) async {
    final core = PreviewCore();
    await mount(tester, core, Workspace(core));
    for (final id in ['track', 'volume']) {
      TestGesture? mouse;
      if (id == 'volume') {
        mouse = await hoverVolume(tester);
      } else {
        await tester.tap(find.byKey(ValueKey('dock-$id')));
      }
      await tester.pumpAndSettle();
      final popupKey = ValueKey(
        id == 'track' ? 'dock-track-preview' : 'dock-volume-popover',
      );
      expect(find.byKey(popupKey), findsOneWidget);
      tester.view.physicalSize = id == 'track'
          ? const Size(780, 560)
          : const Size(1366, 715);
      await tester.pumpAndSettle();
      expect(find.byKey(popupKey), findsNothing);
      if (mouse != null) {
        await mouse.moveTo(Offset.zero);
        await mouse.moveTo(tester.getCenter(find.byKey(ValueKey('dock-$id'))));
      } else {
        await tester.tap(find.byKey(ValueKey('dock-$id')));
      }
      await tester.pumpAndSettle();
      expect(
        tester.getRect(find.byKey(popupKey)).bottom,
        lessThan(tester.getRect(find.byKey(ValueKey('dock-$id'))).top),
      );
      await tester.tapAt(const Offset(40, 40));
      await tester.pumpAndSettle();
      expect(find.byKey(popupKey), findsNothing);
      await mouse?.removePointer();
      expect(tester.takeException(), isNull);
    }
    await unmount(tester, core);
  });

  testWidgets('volume hover crosses the gap and leaves other controls usable', (
    tester,
  ) async {
    EditableText.debugDeterministicCursor = true;
    addTearDown(() => EditableText.debugDeterministicCursor = false);
    Future<void> settle(String step) async {
      try {
        await tester.pumpAndSettle(
          const Duration(milliseconds: 100),
          EnginePhase.sendSemanticsUpdate,
          const Duration(seconds: 3),
        );
      } catch (error) {
        throw TestFailure('$step: $error');
      }
    }

    final core = PreviewCore();
    await mount(tester, core, Workspace(core));
    await tester.tap(find.byType(TextField));
    final searchFocus = tester
        .widget<EditableText>(find.byType(EditableText))
        .focusNode;
    final mouse = await hoverVolume(tester);
    await settle('initial hover');
    final popup = find.byKey(const ValueKey('dock-volume-popover'));
    final button = find.byKey(const ValueKey('dock-volume'));
    expect(searchFocus.hasFocus, isTrue);
    final panelRect = tester.getRect(popup);
    final buttonRect = tester.getRect(button);
    await mouse.moveTo(
      Offset(buttonRect.center.dx, (panelRect.bottom + buttonRect.top) / 2),
    );
    await tester.pump(const Duration(milliseconds: 100));
    expect(popup, findsOneWidget);
    await mouse.moveTo(panelRect.center);
    await tester.pump(const Duration(milliseconds: 300));
    expect(popup, findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await settle('Escape from panel');
    expect(popup, findsNothing);
    expect(searchFocus.hasFocus, isTrue);
    await mouse.moveTo(tester.getCenter(button));
    await settle('reopen after Escape');
    await mouse.moveTo(tester.getCenter(popup));
    await mouse.moveTo(const Offset(100, 200));
    await tester.pump(Anicry.reveal);
    await settle('leave panel');
    expect(popup, findsNothing);
    await mouse.moveTo(tester.getCenter(button));
    await settle('reopen before transport');
    await tester.tap(find.byKey(const ValueKey('dock-toggle')));
    await settle('transport click');
    expect(core.commands.last['action'], 'toggle');
    expect(popup, findsNothing);
    await mouse.moveTo(const Offset(100, 200));
    await mouse.moveTo(tester.getCenter(button));
    await settle('reopen before queue');
    await tester.tap(find.byKey(const ValueKey('dock-queue')));
    await settle('expand queue');
    expect(popup, findsNothing);
    expect(find.byTooltip('Згорнути плеєр · Escape'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await settle('collapse queue');
    expect(find.byKey(const ValueKey('dock-island')), findsOneWidget);
    await mouse.moveTo(const Offset(100, 200));
    await mouse.moveTo(tester.getCenter(button));
    await settle('reopen before navigation');
    core.nav('settings');
    await settle('navigation');
    expect(popup, findsNothing);
    await mouse.moveTo(const Offset(100, 200));
    await mouse.moveTo(tester.getCenter(button));
    await settle('reopen before disposal');
    await mouse.moveTo(const Offset(100, 200));
    await mouse.removePointer();
    await unmount(tester, core);
    await tester.pump(const Duration(milliseconds: 500));
    expect(tester.takeException(), isNull);
  });

  testWidgets('dock controls work and settings invoke saved actions', (
    tester,
  ) async {
    final core = PreviewCore();
    await mount(
      tester,
      core,
      Scaffold(body: PlayerDock(core, onAddPlaylist: (_) {})),
    );
    await tester.tap(find.byKey(const ValueKey('dock-queue')));
    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip('Перемішувати'));
    expect(core.commands.last['action'], 'shuffle');
    await tester.tap(find.byTooltip('Повтор: вимкнено'));
    expect(core.commands.last['action'], 'repeat');
    await tester.tap(find.byTooltip('Прибрати з улюбленого'));
    expect(core.commands.last['action'], 'like');
    await tester.drag(find.text('Зараз звучить'), const Offset(0, 500));
    await tester.pumpAndSettle();
    expect(find.byTooltip('Згорнути плеєр · Escape'), findsNothing);
    await tester.pumpWidget(const SizedBox());
    await mount(
      tester,
      core,
      Scaffold(
        body: ListView(
          padding: const EdgeInsets.all(24),
          children: [SettingsPage(core)],
        ),
      ),
    );
    await tester.tap(find.text('Світла'));
    await tester.pumpAndSettle();
    expect(object(core.commands.last['patch'])['theme'], 'light');
    await tester.tap(find.text('Океан').last);
    expect(core.commands.last['method'], 'set_desktop_icon');
    expect(core.commands.last['args'], ['ocean']);
    await unmount(tester, core);
  });

  testWidgets(
    'full-distance gestures preserve modal focus and restore search focus',
    (tester) async {
      final core = PreviewCore();
      await mount(tester, core, Workspace(core));
      final searchFocus = tester
          .widget<EditableText>(find.byType(EditableText))
          .focusNode;
      await tester.tap(find.byType(TextField));
      await tester.pumpAndSettle();
      expect(searchFocus.hasFocus, isTrue);
      final gesture = await tester.startGesture(
        tester.getCenter(
          find.byTooltip('Потягни вгору, щоб відкрити пісню та чергу'),
        ),
      );
      for (var i = 0; i < 7; i++) {
        await gesture.moveBy(const Offset(0, -100));
        await tester.pump();
      }
      await gesture.up();
      await tester.pumpAndSettle();
      expect(find.byTooltip('Згорнути плеєр · Escape'), findsOneWidget);
      for (var i = 0; i < 35; i++) {
        await tester.sendKeyEvent(LogicalKeyboardKey.tab);
        await tester.pump();
      }
      expect(searchFocus.hasFocus, isFalse);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(find.byTooltip('Згорнути плеєр · Escape'), findsNothing);
      expect(searchFocus.hasFocus, isTrue);
      await tester.tap(find.byKey(const ValueKey('dock-queue')));
      await tester.pumpAndSettle();
      final downward = await tester.startGesture(
        tester.getCenter(find.text('Зараз звучить')),
      );
      for (var i = 0; i < 7; i++) {
        await downward.moveBy(const Offset(0, 100));
        await tester.pump();
      }
      await downward.up();
      await tester.pumpAndSettle();
      expect(find.byTooltip('Згорнути плеєр · Escape'), findsNothing);
      expect(searchFocus.hasFocus, isTrue);
      await unmount(tester, core);
    },
  );

  testWidgets(
    'queue wraps for repeat-all and dismisses player on creator navigation',
    (tester) async {
      final core = PreviewCore();
      core.state['queue_index'] = core.queue.length - 1;
      core.settings['repeat'] = 'all';
      await mount(tester, core, Workspace(core));
      await tester.tap(find.byKey(const ValueKey('dock-queue')));
      await tester.pumpAndSettle();
      expect(find.byType(TrackRow), findsNWidgets(7));
      await tester.tap(find.byTooltip('Дії з треком').first);
      await tester.pumpAndSettle();
      await tester.tap(find.text('Перейти до автора'));
      await tester.pumpAndSettle();
      expect(core.route, 'creator');
      expect(find.byTooltip('Згорнути плеєр · Escape'), findsNothing);
      await unmount(tester, core);
    },
  );

  testWidgets('update actions reflect installation capability and progress', (
    tester,
  ) async {
    final core = PreviewCore();
    core.route = 'settings';
    core.state['update'] = {
      'status': 'ready',
      'current': '3.5.2',
      'version': '3.5.3',
      'installed': true,
      'message': 'Оновлення готове до встановлення',
    };
    await mount(tester, core, Workspace(core));
    await tester.tap(find.text('Оновлення'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Оновити'));
    expect(core.commands.last['action'], 'update-install');
    core.state['update'] = {
      ...core.update,
      'status': 'downloading',
      'total': 100,
      'received': 42,
    };
    core.notifyListeners();
    await tester.pump();
    expect(
      tester
          .widget<LinearProgressIndicator>(find.byType(LinearProgressIndicator))
          .value,
      .42,
    );
    await tester.tap(find.text('Скасувати'));
    expect(core.commands.last['action'], 'update-cancel');
    core.state['update'] = {
      ...core.update,
      'status': 'ready',
      'installed': false,
    };
    core.notifyListeners();
    await tester.pump();
    expect(find.text('Встановити'), findsOneWidget);
    expect(find.text('Перезапустити'), findsNothing);
    await unmount(tester, core);
  });

  if (const bool.fromEnvironment('CAPTURE_UI')) {
    testWidgets('export actual Flutter redesign frames for visual review', (
      tester,
    ) async {
      final output = Directory(
        '${Directory.current.parent.path}/output/flutter-design/minimal-20261023',
      );
      output.createSync(recursive: true);
      Future<void> capture(GlobalKey key, String name) async {
        await tester.runAsync(
          () => Future<void>.delayed(const Duration(milliseconds: 150)),
        );
        await tester.pumpAndSettle();
        await tester.runAsync(() async {
          final boundary =
              key.currentContext!.findRenderObject()! as RenderRepaintBoundary;
          final image = await boundary.toImage(pixelRatio: 1);
          final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
          await File('${output.path}/$name.png')
              .writeAsBytes(bytes!.buffer.asUint8List());
          image.dispose();
        });
      }

      for (final scheme in ['dark', 'light']) {
        final core = PreviewCore(scheme: scheme);
        final key = GlobalKey();
        core.route = 'settings';
        await mount(tester, core, Workspace(core), boundary: key);
        await capture(key, 'settings-$scheme');
        core.nav('search');
        await tester.pumpAndSettle();
        await capture(key, 'search-$scheme');
        await tester.enterText(find.byType(TextField), 'Night Drive');
        await tester.testTextInput.receiveAction(TextInputAction.search);
        await tester.pumpAndSettle();
        await capture(key, 'results-$scheme');
        await tester.tap(find.byKey(const ValueKey('dock-track')));
        await tester.pumpAndSettle();
        await capture(key, 'track-preview-$scheme');
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pumpAndSettle();
        final mouse = await hoverVolume(tester);
        await tester.pumpAndSettle();
        await capture(key, 'volume-$scheme');
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pumpAndSettle();
        await mouse.removePointer();
        await tester.tap(find.byKey(const ValueKey('dock-queue')));
        await tester.pumpAndSettle();
        await capture(key, 'player-$scheme');
        await tester.tap(find.byTooltip('Згорнути плеєр · Escape'));
        await tester.pumpAndSettle();
        tester.view.physicalSize = const Size(780, 560);
        await tester.pumpAndSettle();
        await capture(key, 'narrow-$scheme');
        await unmount(tester, core);
      }
    });
  }
}
