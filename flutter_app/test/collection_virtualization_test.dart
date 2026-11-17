import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:soundflow/anicry_motion.dart';
import 'package:soundflow/core.dart';
import 'package:soundflow/design.dart';
import 'package:soundflow/main.dart';

class CollectionCore extends Core {
  CollectionCore(String view, {bool playing = false}) : super([]) {
    ready = true;
    route = view;
    settings = {'theme': 'dark'};
    state = {
      'queue': fixture,
      'queue_index': 7,
      'selected_playlist': 'large',
      'library': {
        'history': fixture,
        'liked_tracks': fixture,
        'playlists': {
          'large': {'id': 'large', 'name': 'Large playlist', 'tracks': fixture},
        },
      },
    };
    if (playing) playback.value = {'track': fixture[7]};
  }

  final fixture = List<Json>.generate(
    5000,
    (i) => {
      'id': 'track-$i',
      'source': 'youtube',
      'title': 'Track $i',
      'artist': 'Artist',
      'duration': 180,
    },
  );
  final commands = <Json>[];

  @override
  void command(Json value) => commands.add(value);

  void changeRoute(String value) {
    route = value;
    notifyListeners();
  }

  @override
  void dispose() {
    ready = false;
    super.dispose();
  }
}

void main() {
  setUpAll(() async {
    await Anicry.load();
    for (final family in ['Onest', 'Montserrat']) {
      final loader = FontLoader(family);
      final faces = family == 'Onest'
          ? ['Regular', 'SemiBold']
          : ['SemiBold', 'Bold'];
      for (final face in faces) {
        loader.addFont(rootBundle.load('assets/fonts/$family-$face.ttf'));
      }
      await loader.load();
    }
  });

  Future<void> mount(WidgetTester tester, CollectionCore core) async {
    tester.view.physicalSize = const Size(1200, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(core.dispose);
    await tester.pumpWidget(SoundFlow(core));
    await tester.pumpAndSettle();
  }

  ScrollPosition position(WidgetTester tester) => tester
      .state<ScrollableState>(
        find.descendant(
          of: find.byType(CustomScrollView),
          matching: find.byType(Scrollable),
        ),
      )
      .position;

  void expectBoundedRows(WidgetTester tester) {
    final count = find.byType(TrackRow).evaluate().length;
    debugPrint('Mounted TrackRows in 800px viewport: $count / 5000');
    expect(count, greaterThan(0));
    expect(count, lessThan(25), reason: 'Only viewport and cache rows mount');
  }

  for (final route in ['liked', 'history', 'playlist', 'flow']) {
    testWidgets('$route virtualizes 5000 rows and plays the full collection', (
      tester,
    ) async {
      final core = CollectionCore(route);
      await mount(tester, core);
      expectBoundedRows(tester);
      expect(find.text('Track 4999'), findsNothing);
      expect(find.byType(CustomScrollView), findsOneWidget);
      final first = tester.widget<TrackRow>(find.byType(TrackRow).first);
      expect(first.key, ValueKey('${trackKey(core.fixture.first)}:0'));
      position(tester).jumpTo(position(tester).maxScrollExtent);
      await tester.pumpAndSettle();
      expectBoundedRows(tester);
      expect(find.text('Track 4999'), findsOneWidget);
      await tester.tap(find.text('Track 4999'));
      final play = core.commands.last;
      expect(play['type'], 'play');
      expect(object(play['track'])['id'], 'track-4999');
      expect(objects(play['queue']), core.fixture);
      expect(tester.takeException(), isNull);
    });
  }

  for (final playing in [false, true]) {
    testWidgets(
      'queue uses absolute last-row index (current track: $playing)',
      (tester) async {
        final core = CollectionCore('queue', playing: playing);
        await mount(tester, core);
        expectBoundedRows(tester);
        expect(
          tester.widget<TrackRow>(find.byType(TrackRow).first).queueIndex,
          playing ? 8 : 0,
        );
        position(tester).jumpTo(position(tester).maxScrollExtent);
        await tester.pumpAndSettle();
        expectBoundedRows(tester);
        final last = tester.widget<TrackRow>(find.byType(TrackRow).last);
        expect(last.queueIndex, 4999);
        expect(
          last.key,
          ValueKey('${trackKey(core.fixture.last)}:${playing ? 4991 : 4999}'),
        );
        await tester.tap(find.text('Track 4999'));
        expect(core.commands.last, {
          'type': 'action',
          'action': 'queue_play',
          'payload': {'index': 4999},
        });
        core.changeRoute('liked');
        await tester.pump();
        expect(find.byType(FadeTransition), findsWidgets);
        await tester.pumpAndSettle();
        expect(position(tester).pixels, 0);
        expect(find.text('Track 0'), findsOneWidget);
        expectBoundedRows(tester);
        expect(tester.takeException(), isNull);
      },
    );
  }

  testWidgets('collection empty and flow loading states remain visible', (
    tester,
  ) async {
    final core = CollectionCore('liked');
    core.state = {};
    await mount(tester, core);
    expect(find.byType(TrackRow), findsNothing);
    expect(find.byType(EmptyState), findsOneWidget);
    core.state = {'pending': 1};
    core.changeRoute('flow');
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.byType(TrackRow), findsNothing);
    expect(find.byType(EmptyState), findsNothing);
    expect(find.byType(LinearProgressIndicator), findsOneWidget);
  });
}
