import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';

import 'package:soundflow/core.dart';

// Mirrors the wire contract, with sorted keys like serde_json::Map.
Json full(Json state) => object(
  jsonDecode(
    jsonEncode({
      for (final key in state.keys.toList()..sort()) key: state[key],
    }),
  ),
);
Json patch(Json previous, Json next) => {
  'ok': true,
  'state_patch': {
    'set': {
      for (final key in next.keys)
        if (!previous.containsKey(key) ||
            jsonEncode(previous[key]) != jsonEncode(next[key]))
          key: next[key],
    },
    'remove': previous.keys.where((key) => !next.containsKey(key)).toList(),
  },
  'events': [],
};

class CountedJson {
  int encodes = 0;
  Json toJson() {
    encodes++;
    return {
      'tracks': [1, 2, 3],
    };
  }
}

void main() {
  test('field comparison detects semantic changes without a joined snapshot string', () {
    final signature = SnapshotSignature();
    final shared = {'tracks': List.generate(5000, (i) => {'id': i})};
    expect(signature.update({'library': shared, 'playing': false}), isTrue);
    expect(signature.update({'library': shared, 'playing': false}), isFalse);
    expect(signature.update({'playing': false, 'library': shared}), isFalse);
    expect(signature.update({'library': shared, 'playing': true}), isTrue);
    expect(signature.update({'library': shared, 'playing': null}), isTrue);
    expect(signature.update({'library': shared}), isTrue);
    expect(signature.update({'library': jsonDecode(jsonEncode(shared))}), isFalse);
  });
  test(
    'cached signature equals JSON across replacement, deletion and order',
    () {
      final signature = SnapshotSignature();
      final shared = {
        'tracks': [1, 2, 3],
      };
      final snapshots = <Json>[
        {
          'library': shared,
          'player': {'position': 1},
          'nullable': null,
        },
        {
          'library': shared,
          'player': {'position': 2},
          'nullable': null,
        },
        {'nullable': null, 'library': shared},
        {
          'library': {
            'profile': {'id': 'other'},
          },
          'added': [],
        },
        {},
        {'library': shared, 'quoted"key': 'line\nvalue'},
      ];
      for (final snapshot in snapshots) {
        expect(signature.encode(snapshot), jsonEncode(snapshot));
      }
    },
  );

  test('unchanged fields are encoded once until replaced', () {
    final signature = SnapshotSignature();
    final library = CountedJson();
    signature.encode({'library': library, 'revision': 1});
    signature.encode({'library': library, 'revision': 2});
    expect(library.encodes, 1);
    final replacement = CountedJson();
    signature.encode({'library': replacement, 'revision': 3});
    expect(replacement.encodes, 1);
    signature.encode({});
    signature.encode({'library': library});
    expect(library.encodes, 2);
  });

  test('reconstructs removals, explicit null, arrays and account swaps', () {
    final snapshots = <Json>[
      {
        'library': {
          'profile': {'id': 'a'},
          'tracks': [1, 2],
          'private': true,
        },
        'player': {'position': 1, 'volume': .8},
        'temporary': true,
      },
      {
        'library': {
          'profile': {'id': 'a'},
          'tracks': [1, 2],
          'private': true,
        },
        'player': {'position': 2, 'volume': .8},
        'temporary': null,
      },
      {
        'library': {
          'profile': {'id': 'b'},
        },
        'player': null,
        'added': null,
      },
      {'library': {}, 'player': null},
      {'library': {}, 'player': null},
      {'library': {}, 'player': null, 'temporary': []},
    ].map(full).toList();
    var state = reconstructSnapshot({}, {'state': snapshots.first});
    expect(state, snapshots.first);
    for (var i = 1; i < snapshots.length; i++) {
      final previous = state;
      final before = jsonEncode(previous);
      state = reconstructSnapshot(state, patch(snapshots[i - 1], snapshots[i]));
      expect(state, snapshots[i]);
      // Signature serialization also matches a fresh full native snapshot.
      expect(jsonEncode(state), jsonEncode(snapshots[i]));
      expect(jsonEncode(previous), before);
    }
  });

  test(
    'old native full snapshots replace state and reset any patch baseline',
    () {
      var state = reconstructSnapshot(
        {'old': true},
        {
          'state': {'library': {}},
        },
      );
      expect(state, {'library': {}});
      state = reconstructSnapshot(state, {
        'state_patch': {
          'set': {'player': null},
          'remove': [],
        },
      });
      expect(state.containsKey('player'), isTrue);
      expect(state['player'], isNull);
      state = reconstructSnapshot(state, {'state': {}});
      expect(state, isEmpty);
    },
  );

  test(
    'position update retains library and reduces large fixture wire bytes',
    () {
      final initial = full({
        'library': {
          'tracks': List.generate(
            5000,
            (id) => {
              'id': id,
              'title': 'Track $id',
              'artist': 'Example artist',
            },
          ),
        },
        'player': {'position': 10},
        'revision': 1,
      });
      final next = full({
        ...initial,
        'player': {'position': 11},
        'revision': 2,
      });
      final response = patch(initial, next);
      final result = reconstructSnapshot(initial, response);
      expect(result, next);
      expect(identical(result['library'], initial['library']), isTrue);
      final patchBytes = utf8.encode(jsonEncode(response)).length;
      final fullBytes = utf8
          .encode(jsonEncode({'ok': true, 'state': next, 'events': []}))
          .length;
      expect(patchBytes * 100, lessThan(fullBytes));
      // ignore: avoid_print
      print('state transfer: full=$fullBytes bytes, patch=$patchBytes bytes');
    },
  );
}
