import 'dart:convert';

import 'package:animate_icons/animate_icons.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

const asset = 'packages/animate_icons/assets/catalog.json';

List<Object?> geometry(String name, double progress) => [
  for (final stroke in AnimateIcons.vectorFrame(name, progress))
    [
      [
        for (final point in stroke.points) [point.dx, point.dy],
      ],
      stroke.width,
      stroke.opacity,
      stroke.closed,
      stroke.filled,
      stroke.cap.index,
      stroke.join.index,
    ],
];

Object geometryOrUnsupported(String name, double progress) {
  try {
    return geometry(name, progress);
  } on UnsupportedError catch (error) {
    // Some upstream animations use nonuniform transforms which the existing
    // native vector exporter explicitly rejects; preserve that behavior too.
    return 'UnsupportedError: ${error.message}';
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  late Map<String, dynamic> original;

  setUpAll(() async {
    original = jsonDecode(
      await rootBundle.loadString(asset, cache: false),
    ) as Map<String, dynamic>;
  });

  tearDown(() => AnimateIcons.catalog = {});

  test(
    'complete catalog stays encoded, lookups produce independent trees',
    () async {
      await AnimateIcons.load();
      final lazy = AnimateIcons.catalog;
      expect(AnimateIcons.names.toList(), original.keys.toList());
      expect(lazy.length, 1170);
      for (final name in original.keys) {
        expect(lazy.containsKey(name), isTrue);
        final first = lazy[name];
        final second = lazy[name];
        expect(first, original[name], reason: name);
        expect(second, original[name], reason: name);
        expect(identical(first, second), isFalse, reason: name);
      }
      expect(lazy['absent'], isNull);
      final detached = lazy['lucide/info'] as Map;
      detached.clear();
      expect(lazy['lucide/info'], original['lucide/info']);
      lazy['custom/test'] = original['lucide/info'];
      expect(lazy.remove('custom/test'), original['lucide/info']);
      expect(lazy.containsKey('custom/test'), isFalse);
    },
  );

  test(
    'every icon matches original fixture duration and vector geometry',
    () async {
      await AnimateIcons.load();
      final lazy = AnimateIcons.catalog;
      for (final name in original.keys) {
        AnimateIcons.catalog = original;
        final duration = AnimateIcons.duration(name);
        final expected = [
          for (final t in [0.0, .25, .5, .75, 1.0])
            geometryOrUnsupported(name, t),
        ];
        AnimateIcons.catalog = lazy;
        expect(AnimateIcons.duration(name), duration, reason: name);
        final actual = [
          for (final t in [0.0, .25, .5, .75, 1.0])
            geometryOrUnsupported(name, t),
        ];
        expect(actual, expected, reason: name);
      }
    },
  );

  test(
    'installer exports match original fixture at every exported frame',
    () async {
      await AnimateIcons.load();
      final lazy = AnimateIcons.catalog;
      for (final name in [
        'folder-open',
        'download',
        'refresh-cw',
        'x',
        'arrow-left',
        'check',
        'circle-check',
      ]) {
        AnimateIcons.catalog = original;
        final duration = AnimateIcons.duration(name).inMilliseconds;
        final animated = [
          'folder-open',
          'refresh-cw',
          'circle-check',
        ].contains(name);
        final count = animated ? (duration * 60 / 1000).ceil() + 1 : 1;
        final expected = [
          for (var i = 0; i < count; i++)
            geometry(name, count == 1 ? 1 : i / (count - 1)),
        ];
        AnimateIcons.catalog = lazy;
        expect(AnimateIcons.duration(name).inMilliseconds, duration);
        expect(
          [
            for (var i = 0; i < count; i++)
              geometry(name, count == 1 ? 1 : i / (count - 1)),
          ],
          expected,
          reason: name,
        );
      }
      expect(geometry('missing-icon', .5), geometry('info', .5));
      expect(geometry('info', .5), geometry('lucide/info', .5));
    },
  );

  test(
    'reload replaces stale parsed icons and restores the full catalog',
    () async {
      AnimateIcons.catalog = {
        'lucide/info': {'tag': 'g'},
      };
      expect(geometry('info', 1), isEmpty);
      await AnimateIcons.load();
      final first = geometry('info', 1);
      expect(first, isNotEmpty);
      AnimateIcons.catalog.clear();
      await AnimateIcons.load();
      expect(AnimateIcons.names.length, original.length);
      expect(geometry('info', 1), first);
    },
  );
}
