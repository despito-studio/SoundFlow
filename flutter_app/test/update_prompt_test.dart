import 'dart:io';
import 'dart:ui' as ui;

import 'package:animate_icons/animate_icons.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:soundflow/design.dart';
import 'package:soundflow/anicry_motion.dart';
import 'package:soundflow/update_prompt.dart';
import 'package:soundflow/main.dart' show Workspace;

import 'redesign_test.dart' show PreviewCore;

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    await AnimateIcons.load();
    final fonts = FontLoader('Onest')
      ..addFont(rootBundle.load('assets/fonts/Onest-Regular.ttf'))
      ..addFont(rootBundle.load('assets/fonts/Onest-SemiBold.ttf'));
    await fonts.load();
  });
  PreviewCore fixture({String status = 'available'}) {
    final core = PreviewCore();
    core.state['update'] = {
      'status': status,
      'sequence': 1,
      'installed': true,
      'current': '1.0.0',
      'version': '1.0.1',
      'total': 40000000,
      'received': 0,
    };
    return core;
  }

  void emit(
    PreviewCore core,
    String status, {
    int received = 0,
    String? message,
  }) {
    core.state['update'] = {
      ...core.update,
      'status': status,
      'received': received,
      'sequence': (core.update['sequence'] as int) + 1,
      'message': ?message,
    };
    core.notifyListeners();
  }

  Future<void> mount(
    WidgetTester tester,
    PreviewCore core, {
    bool reduced = false,
    double textScale = 1,
    GlobalKey? boundary,
  }) async {
    tester.view.physicalSize = const Size(1366, 768);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await tester.pumpWidget(
      RepaintBoundary(
        key: boundary,
        child: MaterialApp(
          theme: soundflowTheme(true, reduced),
          builder: (context, child) => MediaQuery(
            data: MediaQuery.of(context).copyWith(
              disableAnimations: reduced,
              textScaler: TextScaler.linear(textScale),
            ),
            child: child!,
          ),
          home: UpdatePromptHost(
            core: core,
            child: const Scaffold(body: Center(child: Text('Музика'))),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
  }

  Future<void> unmount(WidgetTester tester, PreviewCore core) async {
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump();
    core.dispose();
  }

  test(
    'startup checks once before downloading and preserves restored readiness',
    () {
      final core = fixture(status: 'idle');
      final now = DateTime(2026, 10, 23);
      core.checkForStartupUpdate(now: now);
      core.checkForStartupUpdate(now: now);
      expect(
        core.commands.where((v) => v['action'] == 'update-check'),
        hasLength(1),
      );
      expect(core.commands.last['payload'], {'download': false});
      core.dispose();
      final cached = fixture(status: 'ready');
      cached.checkForStartupUpdate(now: now);
      expect(cached.commands, isEmpty);
      cached.dispose();
      final offline = fixture(status: 'idle')..arguments.add('--offline');
      offline.checkForStartupUpdate(now: now);
      expect(offline.commands, isEmpty);
      offline.dispose();
    },
  );
  test(
    'startup retries a failed check without duplicating an active check',
    () {
      final core = fixture(status: 'idle');
      final now = DateTime(2026, 10, 25);
      core.checkForStartupUpdate(now: now);
      core.state['update'] = {...core.update, 'status': 'checking'};
      core.checkForStartupUpdate(now: now.add(const Duration(hours: 13)));
      expect(core.commands, hasLength(1));
      core.state['update'] = {...core.update, 'status': 'error'};
      core.checkForStartupUpdate(now: now.add(const Duration(seconds: 59)));
      expect(core.commands, hasLength(1));
      core.checkForStartupUpdate(now: now.add(const Duration(minutes: 1)));
      expect(core.commands, hasLength(2));
      core.checkForStartupUpdate(now: now.add(const Duration(seconds: 61)));
      expect(core.commands, hasLength(2));
      core.dispose();
    },
  );
  testWidgets(
    'one startup action owns asynchronous progress and cancellation',
    (tester) async {
      final core = fixture();
      await mount(tester, core);
      expect(find.text('Оновити'), findsOneWidget);
      final initial = tester.getSize(
        find.byKey(const Key('update-prompt-card')),
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 90));
      final middle = tester.getSize(
        find.byKey(const Key('update-prompt-card')),
      );
      expect(middle.width, greaterThan(initial.width));
      expect(middle.height, lessThan(initial.height));
      for (var i = 0; i < 4; i++) {
        core.notifyListeners();
      }
      expect(
        core.commands.where((v) => v['action'] == 'update-install'),
        hasLength(1),
      );
      emit(core, 'downloading', received: 16800000);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.text('Завантаження · 42%'), findsOneWidget);
      expect(
        tester
            .widget<CircularProgressIndicator>(
              find.byType(CircularProgressIndicator),
            )
            .value,
        .42,
      );
      await tester.tap(find.text('Скасувати'));
      await tester.pump();
      expect(core.commands.last['action'], 'update-cancel');
      emit(core, 'cancelled');
      await tester.pump();
      await tester.pumpAndSettle();
      expect(find.byType(UpdatePrompt), findsNothing);
      emit(core, 'available');
      await tester.pumpAndSettle();
      expect(find.byType(UpdatePrompt), findsNothing);
      expect(
        core.commands.where((v) => v['action'] == 'update-apply'),
        isEmpty,
      );
      await unmount(tester, core);
    },
  );
  testWidgets(
    'ready update requires one click, retries errors and locks accepted handoff',
    (tester) async {
      final core = fixture(status: 'ready');
      await mount(tester, core);
      await tester.tap(find.text('Оновити'));
      await tester.pump(const Duration(milliseconds: 400));
      emit(
        core,
        'error',
        message: 'Не вдалося завантажити оновлення. Перевір підключення до інтернету.',
      );
      await tester.pumpAndSettle();
      expect(find.text('Спробувати ще раз'), findsOneWidget);
      await tester.tap(find.text('Спробувати ще раз'));
      await tester.pump();
      emit(core, 'verifying');
      await tester.pump(const Duration(milliseconds: 400));
      emit(core, 'installing');
      await tester.pump();
      expect(
        tester
            .widget<AnicryTap>(
              find
                  .ancestor(
                    of: find.byKey(const Key('update-shared-action')),
                    matching: find.byType(AnicryTap),
                  )
                  .first,
            )
            .onTap,
        isNull,
      );
      await tester.pump(const Duration(milliseconds: 400));
      expect(
        tester
            .widget<Opacity>(
              find
                  .ancestor(
                    of: find.byKey(const Key('update-shared-action')),
                    matching: find.byType(Opacity),
                  )
                  .first,
            )
            .opacity,
        0,
      );
      expect(
        core.commands.where((v) => v['action'] == 'update-install'),
        hasLength(2),
      );
      expect(
        core.commands.where((v) => v['action'] == 'update-apply'),
        isEmpty,
      );
      emit(core, 'restarting', message: 'Готово');
      await tester.pump();
      final check = find.byKey(const Key('update-complete-check'));
      expect(check, findsOneWidget);
      expect(tester.widget<AnimateIcon>(check).animation!.value, 0);
      await tester.pump(const Duration(milliseconds: 320));
      expect(
        tester.widget<AnimateIcon>(check).animation!.value,
        inExclusiveRange(0, 1),
      );
      await tester.pump(AnimateIcons.duration('circle-check'));
      expect(tester.widget<AnimateIcon>(check).animation!.value, 1);
      await unmount(tester, core);
    },
  );
  testWidgets(
    'shared elements stay visible and move continuously through phases',
    (tester) async {
      final core = fixture();
      await mount(tester, core);
      final logo = find.byKey(const Key('update-shared-logo'));
      final title = find.byKey(const Key('update-shared-title'));
      final card = find.byKey(const Key('update-prompt-card'));
      final retainedLogo = tester.element(logo);
      final retainedTitle = tester.element(title);
      var previousLogo = tester.getRect(logo);
      var previousTitle = tester.getRect(title);
      var previousCard = tester.getSize(card);
      await tester.tap(find.text('Оновити'));
      await tester.pump();
      for (var frame = 1; frame <= 24; frame++) {
        if (frame == 5) {
          emit(core, 'downloading', received: 4000000);
        }
        if (frame == 11) {
          emit(core, 'verifying', message: 'Перевіряємо оновлення…');
        }
        await tester.pump(const Duration(milliseconds: 16));
        if (frame == 1) {
          expect(
            tester
                .widget<Opacity>(find.byKey(const Key('update-metadata')))
                .opacity,
            lessThan(.04),
          );
        }
        expect(identical(tester.element(logo), retainedLogo), isTrue);
        expect(identical(tester.element(title), retainedTitle), isTrue);
        final nextLogo = tester.getRect(logo);
        final nextTitle = tester.getRect(title);
        final nextCard = tester.getSize(card);
        expect((nextLogo.center - previousLogo.center).distance, lessThan(24));
        expect(
          (nextTitle.center - previousTitle.center).distance,
          lessThan(24),
        );
        expect(nextLogo.width, lessThanOrEqualTo(previousLogo.width + .01));
        expect(nextCard.width, greaterThanOrEqualTo(previousCard.width - .01));
        expect(nextCard.height, lessThanOrEqualTo(previousCard.height + .01));
        for (final element in [logo, title]) {
          for (final opacity in tester.widgetList<Opacity>(
            find.ancestor(of: element, matching: find.byType(Opacity)),
          )) {
            expect(opacity.opacity, 1);
          }
        }
        previousLogo = nextLogo;
        previousTitle = nextTitle;
        previousCard = nextCard;
        expect(tester.takeException(), isNull);
      }
      expect(previousLogo.width, closeTo(26 * 1.8, .01));
      expect(previousCard, const Size(282 * 1.8, 78 * 1.8));
      emit(core, 'error', message: 'Перевір підключення до інтернету.');
      await tester.pump();
      for (var frame = 1; frame <= 24; frame++) {
        await tester.pump(const Duration(milliseconds: 16));
        final opacity = tester
            .widget<Opacity>(find.byKey(const Key('update-metadata')))
            .opacity;
        if (opacity > .01) {
          final action = tester.getRect(
            find.byKey(const Key('update-shared-action')),
          );
          expect(action.top - tester.getRect(card).top, greaterThan(132 * 1.8));
        }
        expect(identical(tester.element(logo), retainedLogo), isTrue);
        expect(tester.takeException(), isNull);
      }
      await unmount(tester, core);
    },
  );
  testWidgets(
    'large text stays in the card and reduced motion has no repeating frames',
    (tester) async {
      final core = fixture();
      await mount(tester, core, reduced: true, textScale: 2);
      expect(tester.takeException(), isNull);
      final title = tester.renderObject<RenderParagraph>(
        find.text('SoundFlow').first,
      );
      expect(title.didExceedMaxLines, isFalse);
      await tester.tap(find.text('Оновити'));
      await tester.pump();
      emit(core, 'verifying', message: 'Перевіряємо оновлення…');
      await tester.pump();
      await tester.pumpAndSettle();
      expect(tester.binding.hasScheduledFrame, isFalse);
      expect(tester.takeException(), isNull);
      await unmount(tester, core);
    },
  );
  testWidgets('queued match picker waits for the startup offer', (
    tester,
  ) async {
    final core = fixture(status: 'idle');
    core.state['pending_track'] = core.tracksFixture.first;
    core.state['matches'] = [];
    tester.view.physicalSize = const Size(1366, 768);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      emit(core, 'available');
    });
    await tester.pumpWidget(
      MaterialApp(
        theme: soundflowTheme(true, false),
        home: UpdatePromptHost(
          core: core,
          child: ListenableBuilder(
            listenable: core,
            builder: (context, _) => Workspace(core),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.byType(UpdatePrompt), findsOneWidget);
    expect(find.text('Вибери версію треку'), findsNothing);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.byType(UpdatePrompt), findsNothing);
    expect(find.text('Вибери версію треку'), findsOneWidget);
    core.state.remove('pending_track');
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    await unmount(tester, core);
  });
  test('native vector frames preserve the library completion motion', () {
    final beginning = AnimateIcons.vectorFrame('circle-check', 0);
    final middle = AnimateIcons.vectorFrame('circle-check', .5);
    final end = AnimateIcons.vectorFrame('circle-check', 1);
    expect(beginning.length, lessThan(end.length));
    expect(middle.last.opacity, lessThanOrEqualTo(1));
    expect(end.last.opacity, 1);
    expect(middle.first.points.first, isNot(equals(end.first.points.first)));
    for (final stroke in end) {
      expect(
        stroke.points.every((p) => p.dx.isFinite && p.dy.isFinite),
        isTrue,
      );
    }
  });
  if (const bool.fromEnvironment('CAPTURE_UI')) {
    testWidgets('export update reference states', (tester) async {
      final core = fixture();
      final key = GlobalKey();
      await mount(tester, core, boundary: key);
      final folder = Directory(
        '${Directory.current.parent.path}/output/installer-design/update-prompt',
      );
      folder.createSync(recursive: true);
      Future<void> capture(String name) async {
        await tester.runAsync(() async {
          final image =
              await (key.currentContext!.findRenderObject()!
                      as RenderRepaintBoundary)
                  .toImage();
          final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
          await File('${folder.path}/$name.png')
              .writeAsBytes(bytes!.buffer.asUint8List());
          image.dispose();
        });
      }

      await capture('ready');
      await tester.tap(find.text('Оновити'));
      await tester.pump();
      var elapsed = 0;
      for (final step in [16, 24, 40, 40, 40, 60, 80, 60]) {
        await tester.pump(Duration(milliseconds: step));
        elapsed += step;
        await capture('forward-$elapsed');
        if (elapsed == 120) await capture('morph');
      }
      emit(core, 'downloading', received: 16800000);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));
      await capture('progress');
      emit(
        core,
        'error',
        message: 'Не вдалося завантажити оновлення. Перевір підключення до інтернету.',
      );
      await tester.pump();
      elapsed = 0;
      for (final step in [16, 24, 40, 40, 40, 60, 80, 60]) {
        await tester.pump(Duration(milliseconds: step));
        elapsed += step;
        await capture('reverse-$elapsed');
      }
      await tester.pumpAndSettle();
      await capture('error');
      await unmount(tester, core);
    });
  }
}
