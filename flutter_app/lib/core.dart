import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

import 'package:ffi/ffi.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter/services.dart';
import 'package:animate_icons/animate_icons.dart';

import 'anicry_motion.dart';
import 'l10n.dart';

typedef Json = Map<String, dynamic>;
typedef _RequestNative = Pointer<Utf8> Function(Pointer<Uint8>, IntPtr);
typedef _Request = Pointer<Utf8> Function(Pointer<Uint8>, int);
typedef _PollNative = Pointer<Utf8> Function();
typedef _Poll = Pointer<Utf8> Function();
typedef _FreeNative = Void Function(Pointer<Utf8>);
typedef _Free = void Function(Pointer<Utf8>);

Json object(dynamic value) =>
    value is Map ? Map<String, dynamic>.from(value) : {};
List<Json> objects(dynamic value) =>
    value is List ? value.map(object).toList() : [];
String trackKey(Json t) => '${t['source']}:${t['id']}';
bool onlineTrack(Json t) => t['source'] != 'local' && t['is_local'] != true;
String trackTitle(Json t, [BuildContext? context]) =>
    t['title']?.toString() ??
    (context == null ? 'Без назви' : 'Без назви'.tr(context));
String trackArtist(Json t, [BuildContext? context]) =>
    (t['artist'] ?? t['uploader'])?.toString() ??
    (context == null
        ? 'Невідомий виконавець'
        : 'Невідомий виконавець'.tr(context));
String trackCover(Json t) => (t['thumbnail'] ?? t['cover'] ?? '').toString();
String sourceName(Json t, [BuildContext? context]) => switch (t['source']) {
  'local' => context == null ? 'Локально' : 'Локально'.tr(context),
  'youtube' => 'YouTube',
  'soundcloud' => 'SoundCloud',
  'spotify' => 'Spotify',
  _ => t['source']?.toString() ?? '',
};
String clock(num seconds) {
  final n = seconds.isFinite ? seconds.toInt().clamp(0, 9999999) : 0;
  return '${n ~/ 60}:${(n % 60).toString().padLeft(2, '0')}';
}

String trackCount(int n, [BuildContext? context]) =>
    context != null && Localizations.localeOf(context).languageCode == 'en'
    ? (n == 1 ? '{count} трек' : '{count} треків')
          .tr(context)
          .replaceAll('{count}', '$n')
    : '$n ${n % 100 >= 11 && n % 100 <= 14 ? 'треків' : switch (n % 10) {
              1 => 'трек',
              2 || 3 || 4 => 'треки',
              _ => 'треків',
            }}';

/// Reconstruct a poll before consumers compute signatures or read state.
/// Each set value replaces the complete top-level field, including nested maps.
/// Explicit removals keep null values distinct from absent keys.
Json reconstructSnapshot(Json previous, Json response) {
  if (response.containsKey('state')) return object(response['state']);
  final patch = object(response['state_patch']);
  final next = Map<String, dynamic>.from(previous);
  for (final key in (patch['remove'] as List? ?? const [])) {
    next.remove(key);
  }
  next.addAll(object(patch['set']));
  // Rust's JSON objects are key-sorted. Preserve full-snapshot signature order
  // even when a patch introduces a new field or removes and re-adds one.
  return {for (final key in next.keys.toList()..sort()) key: next[key]};
}

/// Native snapshot fields are immutable between replacements. Cache their JSON
/// separately so a playback tick need not traverse the unchanged library.
class SnapshotSignature {
  Json _previous = {};
  Map<String, String> _encoded = {};

  bool update(Json snapshot) {
    final encoded = <String, String>{};
    var changed = snapshot.length != _encoded.length;
    for (final entry in snapshot.entries) {
      final value = _previous.containsKey(entry.key) &&
              identical(_previous[entry.key], entry.value)
          ? _encoded[entry.key]!
          : jsonEncode(entry.value);
      encoded[entry.key] = value;
      if (_encoded[entry.key] != value) changed = true;
    }
    _previous = Map<String, dynamic>.from(snapshot);
    _encoded = encoded;
    return changed;
  }

  String encode(Json snapshot) {
    update(snapshot);
    return '{${snapshot.keys.map((key) => '${jsonEncode(key)}:${_encoded[key]}').join(',')}}';
  }
}

class Core extends ChangeNotifier {
  Core(this.arguments);
  final List<String> arguments;
  static const window = MethodChannel('soundflow/window');
  final playback = ValueNotifier<Json>({});
  Json state = {}, settings = {};
  String error = '', route = 'home', libraryTab = 'playlists';
  bool ready = false;
  late final DynamicLibrary _library;
  late final _Request _open, _send;
  late final _Poll _poll;
  late final _Free _free;
  late final void Function() _close;
  Timer? _timer, _volumeCommit, _updateExit;
  double? _volumeIntent;
  VoidCallback? openConnections;
  String _assetsKey = "";
  int _assetsAttempts = 0;
  DateTime _assetsPoll = DateTime.fromMillisecondsSinceEpoch(0);
  final List<String> _history = [];
  Json? _addAfterCreate;
  String _oldPlaylist = '';
  final _snapshotSignature = SnapshotSignature();
  String? _captionTheme;
  String? _windowIcon;
  bool _homeRequested = false;
  DateTime _feedPoll = DateTime.fromMillisecondsSinceEpoch(0);
  bool? _trayEnabled, _islandShortcuts;
  Json creatorTrack = {};
  DateTime _updatePoll = DateTime.now();
  bool _updateQuitting = false;
  bool _startupUpdateChecked = false;
  Json get update => object(state['update']);

  /// Background update install: download without asking and apply at next launch.
  bool get autoUpdateInstall => settings['auto_update_install'] != false;
  bool updatePromptOpen = false;
  void closeUpdatePrompt() {
    updatePromptOpen = false;
    notifyListeners();
  }

  void checkForStartupUpdate({DateTime? now}) {
    if (arguments.contains('--offline') ||
        settings.isEmpty ||
        update['installed'] != true ||
        settings['auto_update_enabled'] == false) {
      return;
    }
    final time = now ?? DateTime.now();
    final status = update['status'];
    if ([
      'checking',
      'available',
      'ready',
      'downloading',
      'verifying',
      'installing',
      'restarting',
    ].contains(status)) {
      return;
    }
    final interval = status == 'error'
        ? const Duration(minutes: 1)
        : const Duration(hours: 12);
    if (_startupUpdateChecked && time.difference(_updatePoll) < interval) {
      return;
    }
    _startupUpdateChecked = true;
    _updatePoll = time;
    action('update-check', {
      'download': autoUpdateInstall,
      'auto': autoUpdateInstall,
    });
  }

  Json get library => object(state['library']);
  Json get current => object(playback.value['track']);
  List<Json> get recent =>
      objects(library['history']).where(onlineTrack).toList();
  List<Json> get liked =>
      objects(library['liked_tracks']).where(onlineTrack).toList();
  List<Json> get playlists => object(library['playlists']).values.map((value) {
    final p = object(value);
    return {...p, 'tracks': objects(p['tracks']).where(onlineTrack).toList()};
  }).toList();
  List<Json> get queue => objects(state['queue']);
  int get queueIndex => (state['queue_index'] as num?)?.toInt() ?? 0;
  String get selectedPlaylist => state['selected_playlist']?.toString() ?? '';
  Json get playlist {
    final p = object(object(library['playlists'])[selectedPlaylist]);
    return {...p, 'tracks': objects(p['tracks']).where(onlineTrack).toList()};
  }

  String get language => settings['language'] == 'en' ? 'en' : 'uk';
  String get scheme => settings['theme']?.toString() ?? 'dark';
  bool get dark => scheme != 'light';
  bool get reduced => settings['reduced_motion'] == true;
  bool get playing => playback.value['playing'] == true;
  String get playerStatus => playback.value['status']?.toString() ?? '';
  bool get hasTrack => current.isNotEmpty;
  bool get canBack => _history.isNotEmpty;
  int get pending => (state['pending'] as num?)?.toInt() ?? 0;
  List<Json> get allTracks {
    final seen = <String>{};
    return [
          ...liked,
          ...playlists.expand((p) => objects(p['tracks'])),
          ...object(library['track_catalog']).values.map(object),
        ]
        .where(
          (t) => onlineTrack(t) && t['id'] != null && seen.add(trackKey(t)),
        )
        .toList();
  }

  List<Json> get homeTracks {
    final seen = <String>{};
    return (recent.isEmpty ? liked : recent)
        .where((t) => seen.add(trackKey(t)))
        .take(12)
        .toList();
  }

  List<Json> get tracks => switch (route) {
    'search' => objects(state['search']).where(onlineTrack).toList(),
    'liked' => liked,
    'history' => recent,
    'queue' => queue.skip(hasTrack ? queueIndex + 1 : 0).toList(),
    'playlist' => objects(playlist['tracks']).where(onlineTrack).toList(),
    'creator' => objects(creator['tracks']).where(onlineTrack).toList(),
    _ => allTracks,
  };
  bool isLiked(Json t) => liked.any((l) => trackKey(l) == trackKey(t));

  Json get profile => object(library['profile']);
  Json get feed => object(state['feed']);
  Json get creator {
    final value = object(state['creator']);
    return value['track_key'] == trackKey(creatorTrack) ? value : {};
  }

  void showCreator(Json track) {
    creatorTrack = track;
    nav('creator');
    call('get_creator_profile', [track]);
  }

  String argument(String key, String fallback) {
    final i = arguments.indexOf(key);
    return i >= 0 && i + 1 < arguments.length ? arguments[i + 1] : fallback;
  }

  Future<void> start() async {
    try {
      final executableDir = File(Platform.resolvedExecutable).parent.path;
      final corePath = argument('--core', '$executableDir/soundflow_lib.dll');
      final resources = argument('--resources', executableDir);
      _library = DynamicLibrary.open(corePath);
      _open = _library.lookupFunction<_RequestNative, _Request>('sf_open');
      _send = _library.lookupFunction<_RequestNative, _Request>('sf_command');
      _poll = _library.lookupFunction<_PollNative, _Poll>('sf_poll');
      _free = _library.lookupFunction<_FreeNative, _Free>('sf_free');
      _close = _library.lookupFunction<Void Function(), void Function()>(
        'sf_close',
      );
      final result = _request(_open, {
        'resources': resources,
        'offline': arguments.contains('--offline'),
        'state_patch_v1': true,
      });
      if (result['secondary'] == true) {
        await window.invokeMethod('quit');
        return;
      }
      if (result['ok'] != true) {
        throw StateError(
          result['error']?.toString() ?? 'Не вдалося запустити аудіоядро',
        );
      }
      ready = true;
      Future<void>.delayed(const Duration(milliseconds: 700), () {
        if (ready) {
          window.invokeMethod('prepare-island');
          if (arguments.contains('--export-island')) {
            Future<void>.delayed(const Duration(seconds: 2), () {
              if (ready) window.invokeMethod('show-island', true);
            });
          }
        }
      });
      window.setMethodCallHandler((call) async {
        if (call.method == 'core-command') command(object(call.arguments));
        if (call.method == 'closing') {
          _timer?.cancel();
          _close();
        }
      });
      _tick();
      final initialView = argument('--view', 'home');
      if (['library', 'profile', 'settings', 'search'].contains(initialView)) {
        nav(initialView);
      }
      _timer = Timer.periodic(
        const Duration(milliseconds: 250),
        (_) => _tick(),
      );
    } catch (e) {
      error = 'Не вдалося відкрити SoundFlow. $e';
      notifyListeners();
    }
  }

  Json _request(_Request function, Json request) {
    final bytes = utf8.encode(jsonEncode(request));
    final memory = calloc<Uint8>(bytes.length);
    try {
      memory.asTypedList(bytes.length).setAll(0, bytes);
      return _read(function(memory, bytes.length));
    } finally {
      calloc.free(memory);
    }
  }

  Json _read(Pointer<Utf8> p) {
    try {
      return object(jsonDecode(p.toDartString()));
    } finally {
      _free(p);
    }
  }

  void _tick() {
    if (!ready) return;
    final result = _read(_poll());
    if (result['ok'] != true) {
      error = result['error']?.toString() ?? '';
      notifyListeners();
      return;
    }
    final next = reconstructSnapshot(state, result);
    final player = object(next['player']);
    final reportedVolume = (player['volume'] as num?)?.toDouble() ?? .8;
    if (_volumeIntent != null &&
        _volumeCommit == null &&
        (reportedVolume - _volumeIntent!).abs() < .0005) {
      _volumeIntent = null;
    }
    player['volume'] = _volumeIntent ?? reportedVolume;
    playback.value = player;
    final stable = Map<String, dynamic>.from(next)
      ..remove('player')
      ..remove('revision');
    stable['playback_ui'] = {
      'track': trackKey(object(playback.value['track'])),
      'playing': playing,
      'loading': playback.value['loading'],
      'status': playback.value['status'] ?? '',
    };
    stable['settings'] = Map<String, dynamic>.from(object(stable['settings']))
      ..remove('volume');
    final snapshotChanged = _snapshotSignature.update(stable);
    state = next;
    settings = object(next['settings']);
    if (update['status'] == 'restarting' && !_updateQuitting) {
      _updateQuitting = true;
      _timer?.cancel();
      _volumeCommit?.cancel();
      notifyListeners();
      _updateExit = Timer(
        reduced
            ? Duration.zero
            : Anicry.layout + AnimateIcons.duration('circle-check'),
        () {
          _close();
          ready = false;
          window.invokeMethod('quit');
        },
      );
      return;
    }
    checkForStartupUpdate();
    final nextRoute = next['view']?.toString() ?? route;
    final routeChanged = route != nextRoute;
    route = nextRoute;
    if (snapshotChanged || routeChanged) {
      notifyListeners();
    }
    final theme = dark ? 'dark' : 'light';
    if (_captionTheme != theme) {
      _captionTheme = theme;
      command({'type': 'caption', 'dark': dark});
      window.invokeMethod('theme', dark);
    }
    final shortcuts = settings['hotkeys_enabled'] != false;
    if (_islandShortcuts != shortcuts) {
      _islandShortcuts = shortcuts;
      window
          .invokeMapMethod<String, dynamic>('island-shortcuts', shortcuts)
          .then((keys) {
            if (arguments.contains('--export-design')) {
              debugPrint('Island shortcuts: $keys');
            }
            if (shortcuts &&
                keys != null &&
                keys['quick'] != true &&
                keys['fallback'] != true) {
              error = 'Клавіші Island зайняті іншою програмою. Відкрий його кнопкою в Dock.';
              notifyListeners();
            }
          });
    }
    final tray = settings['tray_enabled'] == true;
    final icon = settings['desktop_icon']?.toString() ?? 'default';
    if (_windowIcon != icon) {
      _windowIcon = icon;
      window.invokeMethod('app-icon', icon);
    }
    if (_trayEnabled != tray) {
      _trayEnabled = tray;
      window.invokeMethod('close-to-tray', tray);
    }
    window.invokeMethod('island-state', {
      'player': {...playback.value, 'liked': isLiked(current)},
      'reduced': reduced,
      'language': language,
    });
    if ((!_homeRequested ||
            (route == 'home' &&
                feed['loaded'] != true &&
                DateTime.now().difference(_feedPoll).inMilliseconds > 1200)) &&
        library.isNotEmpty) {
      _feedPoll = DateTime.now();
      _homeRequested = true;
      call('get_home_feed');
    }
    if (['home', 'profile', 'creator'].contains(route) && library.isNotEmpty) {
      final seeds = artistSeeds;
      final connected = object(profile['connected_accounts']);
      final key = jsonEncode([
        seeds.map((t) => '${t['source']}:${trackArtist(t)}').toList(),
        connected,
      ]);
      if (key != _assetsKey) {
        _assetsKey = key;
        _assetsAttempts = 0;
      }
      if ((profileAssets['loaded'] != true || _assetsAttempts == 0) &&
          _assetsAttempts < 30 &&
          DateTime.now().difference(_assetsPoll).inMilliseconds > 1600) {
        _assetsPoll = DateTime.now();
        _assetsAttempts++;
        call('get_profile_assets', [
          {'artists': seeds, 'connections': connected},
        ]);
      }
    }
    for (final event in objects(result['events'])) {
      switch (event['type']) {
        case 'show':
          window.invokeMethod('show');
        case 'island':
          window.invokeMethod('toggle-island', event['keyboard'] == true);
        case 'quit':
          _timer?.cancel();
          _close();
          window.invokeMethod('quit');
        case 'error':
          error = event['message']?.toString() ?? '';
          notifyListeners();
      }
    }
    if (_addAfterCreate != null && next['last_method'] == 'create_playlist') {
      if (selectedPlaylist.isNotEmpty && selectedPlaylist != _oldPlaylist) {
        final track = _addAfterCreate!;
        _addAfterCreate = null;
        call('add_to_playlist', [selectedPlaylist, track]);
      } else if (object(next['last_result'])['ok'] == false) {
        _addAfterCreate = null;
      }
    }
  }

  void command(Json value) {
    if (!ready) return;
    if (value['type'] == 'volume') {
      setVolume((value['value'] as num).toDouble());
      return;
    }
    final result = _request(_send, value);
    if (result['ok'] != true) {
      error = result['error']?.toString() ?? '';
      notifyListeners();
    }
  }

  void nav(String view) {
    if (view == 'connections') {
      openConnections?.call();
      return;
    }
    if (view == route) return;
    _history.add(route);
    route = view;
    command({'type': 'navigate', 'view': view});
    notifyListeners();
  }

  void back() {
    if (_history.isNotEmpty) {
      route = _history.removeLast();
      command({'type': 'navigate', 'view': route});
      notifyListeners();
    }
  }

  void selectPlaylist(Json p) {
    _history.add(route);
    route = 'playlist';
    command({'type': 'playlist', 'id': p['id']});
    notifyListeners();
  }

  void play(Json track, [List<Json>? context]) {
    if (hasTrack && trackKey(current) == trackKey(track)) {
      player('toggle');
      return;
    }
    command({'type': 'play', 'track': track, 'queue': context ?? tracks});
  }

  void player(String action) => command({'type': 'player', 'action': action});
  void action(String action, [dynamic payload]) =>
      command({'type': 'action', 'action': action, 'payload': payload});
  void call(String method, [List<dynamic> args = const []]) =>
      command({'type': 'call', 'method': method, 'args': args});
  void setting(String key, dynamic value) {
    command({
      'type': 'settings',
      'patch': {key: value},
    });
    if ((key == 'auto_update_enabled' || key == 'auto_update_install') &&
        value == false) {
      action('update-cancel');
    }
  }

  void search(String query, String source) =>
      command({'type': 'search', 'query': query.trim(), 'source': source});
  void createPlaylist(String name, [Json? track]) {
    _oldPlaylist = selectedPlaylist;
    _addAfterCreate = track;
    call('create_playlist', [name.trim()]);
  }

  Json get profileAssets => object(state['profile_assets']);
  List<Json> get artistSeeds {
    final seen = <String>{};
    return [
          ...homeTracks,
          ...objects(feed['youtube']),
          ...objects(feed['soundcloud']),
          ...allTracks,
        ]
        .where(
          (t) => onlineTrack(t) && seen.add('${t['source']}:${trackArtist(t)}'),
        )
        .take(16)
        .toList();
  }

  Json artistData(Json track) => objects(profileAssets['artists']).firstWhere(
    (a) => a['key'] == '${track['source']}:${trackArtist(track)}',
    orElse: () => {},
  );
  void setVolume(double value) {
    if (!ready || !value.isFinite) return;
    value = value.clamp(0.0, 1.0);
    _volumeIntent = value;
    playback.value = {...playback.value, 'volume': value};
    _request(_send, {'type': 'volume', 'value': value});
    _volumeCommit?.cancel();
    _volumeCommit = Timer(const Duration(milliseconds: 300), () {
      _volumeCommit = null;
      command({'type': 'volume-commit', 'value': _volumeIntent ?? value});
    });
  }

  Future<String?> pickProfileImage() async {
    final path = await window.invokeMethod<String>('pick-image');
    if (path == null || !ready) return null;
    final result = _request(_send, {'type': 'profile-image', 'path': path});
    if (result['ok'] == true) return result['url']?.toString();
    error = result['error']?.toString() ?? 'Не вдалося зберегти фото';
    notifyListeners();
    return null;
  }

  void island() {
    window.invokeMethod('toggle-island', false);
  }

  void clearError() {
    error = '';
    action('dismiss-message');
    notifyListeners();
  }

  @override
  void dispose() {
    _timer?.cancel();
    _volumeCommit?.cancel();
    _updateExit?.cancel();
    if (ready) {
      if (_volumeIntent != null) {
        command({"type": "volume-commit", "value": _volumeIntent});
      }
      _close();
    }
    playback.dispose();
    super.dispose();
  }
}
