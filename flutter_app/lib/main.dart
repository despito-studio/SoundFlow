import 'dart:math' as math;
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter/services.dart';
import 'package:flutter/rendering.dart';

import 'core.dart';
import 'l10n.dart';
import 'connections.dart';
import 'anicry_motion.dart';
import 'cider_shell.dart';
import 'discovery.dart';
import 'profiles.dart';
import 'design.dart';
import 'island.dart';
import 'media_slider.dart';
import 'search_page.dart';
import 'settings_page.dart';
import 'update_prompt.dart';
import 'portrait_background.dart';
import 'visualizer.dart';

final designBoundary = GlobalKey();

void main(List<String> arguments) async {
  WidgetsFlutterBinding.ensureInitialized();
  // Tight low-RAM image cache cap (8 MiB main, 3 MiB island).
  PaintingBinding.instance.imageCache
    ..maximumSizeBytes = arguments.contains('--island')
        ? 3 * 1024 * 1024
        : 8 * 1024 * 1024
    ..maximumSize = arguments.contains('--island') ? 16 : 64;
  await Anicry.load();
  if (arguments.contains('--island')) {
    final exportIndex = arguments.indexOf('--export-island');
    runApp(
      IslandApp(
        exportPath: exportIndex >= 0 && exportIndex + 1 < arguments.length
            ? arguments[exportIndex + 1]
            : null,
      ),
    );
    return;
  }
  final core = Core(arguments);
  runApp(RepaintBoundary(key: designBoundary, child: SoundFlow(core)));
  // Paint the startup portrait before synchronous native initialization begins.
  await WidgetsBinding.instance.endOfFrame;
  await Future<void>.delayed(Duration.zero);
  core.start();
  final exportIndex = arguments.indexOf('--export-design');
  if (exportIndex >= 0 && exportIndex + 1 < arguments.length) {
    final delay = int.tryParse(core.argument('--export-after', '5')) ?? 5;
    Future<void>.delayed(Duration(seconds: delay.clamp(1, 60)), () async {
      final boundary =
          designBoundary.currentContext?.findRenderObject()
              as RenderRepaintBoundary?;
      if (boundary == null) return;
      final image = await boundary.toImage(pixelRatio: 1);
      final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
      if (bytes != null) {
        final file = File(arguments[exportIndex + 1]);
        await file.parent.create(recursive: true);
        await file.writeAsBytes(bytes.buffer.asUint8List());
      }
      image.dispose();
    });
  }
}

class SoundFlow extends StatelessWidget {
  const SoundFlow(this.core, {super.key});
  final Core core;
  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: Listenable.merge([core, redesignPrefs]),
    builder: (context, _) => MaterialApp(
      title: 'SoundFlow',
      locale: Locale(core.language),
      supportedLocales: const [Locale('uk'), Locale('en')],
      localizationsDelegates: GlobalMaterialLocalizations.delegates,
      debugShowCheckedModeBanner: false,
      scrollBehavior: const MaterialScrollBehavior().copyWith(
        scrollbars: false,
      ),
      theme: soundflowTheme(core.dark, core.reduced, scheme: core.scheme),
      themeAnimationDuration: Duration(milliseconds: core.reduced ? 0 : 180),
      builder: (context, child) => MediaQuery(
        data: MediaQuery.of(context).copyWith(
          disableAnimations:
              core.reduced || MediaQuery.disableAnimationsOf(context),
        ),
        child: Stack(
          children: [
            Positioned.fill(
              child: ColoredBox(
                color: Theme.of(context).scaffoldBackgroundColor,
              ),
            ),
            Positioned.fill(child: child!),
          ],
        ),
      ),
      home: core.ready
          ? UpdatePromptHost(core: core, child: Workspace(core))
          : PortraitStartup(error: core.error),
    ),
  );
}

class Workspace extends StatefulWidget {
  const Workspace(this.core, {super.key});
  final Core core;
  @override
  State<Workspace> createState() => _WorkspaceState();
}

class _WorkspaceState extends State<Workspace> {
  final query = TextEditingController(), searchFocus = FocusNode();
  final scroll = ScrollController();
  String _route = 'home';
  bool _matchShown = false;
  Core get c => widget.core;
  bool _connectionsShown = false;
  @override
  void initState() {
    super.initState();
    c.openConnections = showConnections;
    if (c.arguments.contains('--edit-profile')) {
      Future<void>.delayed(const Duration(milliseconds: 900), () {
        if (mounted) editProfile(context, c);
      });
    }
    if (c.argument('--view', '') == 'connections') {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) showConnections();
      });
    }
  }

  void showConnections() {
    if (_connectionsShown || c.updatePromptOpen || !mounted) return;
    _connectionsShown = true;
    c.call('list_accounts');
    showAnicryDialog<void>(
      context: context,
      builder: (_) => ConnectionsPopup(c),
    ).whenComplete(() => _connectionsShown = false);
  }

  void showMatchPicker() {
    if (_matchShown || c.updatePromptOpen || c.state['pending_track'] == null) {
      return;
    }
    _matchShown = true;
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mounted) return;
      if (c.updatePromptOpen) {
        _matchShown = false;
        return;
      }
      var selected = false;
      await showDialog<void>(
        context: context,
        builder: (ctx) => Dialog(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 620, maxHeight: 480),
            child: Padding(
              padding: const EdgeInsets.all(24),
              child: ListenableBuilder(
                listenable: c,
                builder: (context, _) {
                  final matches = objects(c.state['matches']);
                  return Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          Expanded(
                            child: Text(
                              'Вибери версію треку'.tr(context),
                              style: TextStyle(
                                fontFamily: SoundFlowType.heading,
                                fontSize: 24,
                                fontWeight: FontWeight.w600,
                              ),
                            ),
                          ),
                          Control(
                            'x',
                            'Закрити вибір'.tr(context),
                            onTap: () => Navigator.pop(ctx),
                          ),
                        ],
                      ),
                      const SizedBox(height: 12),
                      Text(
                        'Вибери запис із YouTube або SoundCloud для відтворення.'
                            .tr(context),
                        style: TextStyle(
                          color: context.paints.muted,
                          fontSize: 14,
                        ),
                      ),
                      const SizedBox(height: 18),
                      if (matches.isEmpty && c.pending > 0)
                        const Padding(
                          padding: EdgeInsets.symmetric(vertical: 24),
                          child: LinearProgressIndicator(minHeight: 2),
                        ),
                      if (matches.isEmpty && c.pending == 0)
                        Padding(
                          padding: EdgeInsets.symmetric(vertical: 24),
                          child: Text(
                            'Відповідний запис не знайдено. Спробуй пошук в іншому джерелі.'
                                .tr(context),
                          ),
                        ),
                      if (matches.isNotEmpty)
                        Flexible(
                          child: ListView.builder(
                            shrinkWrap: true,
                            itemCount: matches.length,
                            itemBuilder: (context, index) {
                              final t = matches[index];
                              return ListTile(
                                contentPadding: const EdgeInsets.symmetric(
                                  vertical: 4,
                                ),
                                leading: Artwork(
                                  trackCover(t),
                                  size: 44,
                                  radius: 8,
                                ),
                                title: Text(
                                  trackTitle(t, context),
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                ),
                                subtitle: Text(
                                  '${trackArtist(t, context)} · ${sourceName(t, context)}',
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                ),
                                trailing: Text(
                                  t['duration_formatted']?.toString() ?? '',
                                  style: TextStyle(
                                    color: context.paints.muted,
                                    fontSize: 12,
                                  ),
                                ),
                                onTap: () {
                                  selected = true;
                                  c.action('choose_match', t);
                                  Navigator.pop(ctx);
                                },
                              );
                            },
                          ),
                        ),
                    ],
                  );
                },
              ),
            ),
          ),
        ),
      );
      _matchShown = false;
      if (!selected) c.action('dismiss_match');
    });
  }

  @override
  void dispose() {
    c.openConnections = null;
    query.dispose();
    searchFocus.dispose();
    scroll.dispose();
    super.dispose();
  }

  String get title => switch (c.route) {
    'home' => 'Discovery',
    'library' => 'Медіатека'.tr(context),
    'playlist' => 'Плейліст'.tr(context),
    'search' => 'Пошук'.tr(context),
    'liked' => 'Улюблене'.tr(context),
    'queue' => 'Черга'.tr(context),
    'now' => 'Зараз звучить'.tr(context),
    'settings' => 'Налаштування'.tr(context),
    'downloads' => 'Завантаження'.tr(context),
    'history' => 'Історія'.tr(context),
    'profile' => 'Твій профіль'.tr(context),
    'creator' => 'Автор'.tr(context),
    'connections' => 'Підключення'.tr(context),
    'stats' => 'Прослуховування'.tr(context),
    _ => 'SoundFlow',
  };
  void navigate(String route) {
    c.nav(route);
  }

  Future<String?> textDialog(
    String title,
    String label, {
    String value = '',
    String confirm = 'Створити',
  }) async {
    final input = TextEditingController(text: value);
    final result = await showAnicryDialog<String>(
      context: context,
      builder: (ctx) => Dialog(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 460),
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Expanded(
                      child: Text(
                        title,
                        style: const TextStyle(
                          fontFamily: SoundFlowType.heading,
                          fontSize: 24,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ),
                    Control(
                      'x',
                      'Закрити'.tr(context),
                      onTap: () => Navigator.pop(ctx),
                    ),
                  ],
                ),
                const SizedBox(height: 24),
                TextField(
                  controller: input,
                  autofocus: true,
                  decoration: InputDecoration(
                    hintText: label,
                    suffixIcon: IconButton(
                      tooltip: 'Вставити'.tr(context),
                      icon: const Icon(Icons.content_paste_rounded, size: 18),
                      onPressed: () async {
                        final data = await Clipboard.getData(
                          Clipboard.kTextPlain,
                        );
                        if (!ctx.mounted || data?.text == null) return;
                        final selection = input.selection;
                        final start = selection.isValid
                            ? selection.start
                            : input.text.length;
                        final end = selection.isValid
                            ? selection.end
                            : input.text.length;
                        final text = data!.text!;
                        input.value = TextEditingValue(
                          text: input.text.replaceRange(start, end, text),
                          selection: TextSelection.collapsed(
                            offset: start + text.length,
                          ),
                        );
                      },
                    ),
                  ),
                  onSubmitted: (v) {
                    if (v.trim().isNotEmpty) Navigator.pop(ctx, v.trim());
                  },
                ),
                const SizedBox(height: 24),
                Row(
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: [
                    ActionButton(
                      'Скасувати'.tr(context),
                      flat: true,
                      onTap: () => Navigator.pop(ctx),
                    ),
                    const SizedBox(width: 12),
                    ActionButton(
                      confirm,
                      primary: true,
                      onTap: () {
                        if (input.text.trim().isNotEmpty) {
                          Navigator.pop(ctx, input.text.trim());
                        }
                      },
                    ),
                  ],
                ),
              ],
            ),
          ),
        ),
      ),
    );
    await Future<void>.delayed(const Duration(milliseconds: 320));
    input.dispose();
    return result;
  }

  Future<void> createPlaylist([Json? track]) async {
    final name = await textDialog(
      'Новий плейліст'.tr(context),
      'Назва плейліста'.tr(context),
    );
    if (name != null) c.createPlaylist(name, track);
  }

  Future<void> addPlaylist(Json track) async {
    final choice = await showAnicryDialog<String>(
      context: context,
      builder: (ctx) => Dialog(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 420, maxHeight: 480),
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Expanded(
                      child: Text(
                        'Додати до плейліста'.tr(context),
                        style: TextStyle(
                          fontSize: 22,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ),
                    Control(
                      'x',
                      'Закрити'.tr(context),
                      onTap: () => Navigator.pop(ctx),
                    ),
                  ],
                ),
                const SizedBox(height: 18),
                Flexible(
                  child: ListView(
                    shrinkWrap: true,
                    children: [
                      for (final p in c.playlists)
                        ListTile(
                          contentPadding: EdgeInsets.zero,
                          title: Text(p['name'] ?? 'Плейліст'.tr(context)),
                          subtitle: Text(
                            trackCount(objects(p['tracks']).length, context),
                          ),
                          onTap: () => Navigator.pop(ctx, p['id'].toString()),
                        ),
                    ],
                  ),
                ),
                const SizedBox(height: 16),
                ActionButton(
                  'Новий плейліст'.tr(context),
                  icon: 'plus',
                  onTap: () => Navigator.pop(ctx, '__new__'),
                ),
              ],
            ),
          ),
        ),
      ),
    );
    if (choice == '__new__') {
      await createPlaylist(track);
    } else if (choice != null) {
      c.call('add_to_playlist', [choice, track]);
    }
  }

  Widget trackRow(List<Json> tracks, int i, {bool queue = false}) => TrackRow(
    tracks[i],
    key: ValueKey('${trackKey(tracks[i])}:$i'),
    core: c,
    queueIndex: queue ? i + (c.hasTrack ? c.queueIndex + 1 : 0) : null,
    onPlay: queue
        ? () => c.action('queue_play', {
            'index': i + (c.hasTrack ? c.queueIndex + 1 : 0),
          })
        : () => c.play(tracks[i], tracks),
    onAddPlaylist: () => addPlaylist(tracks[i]),
  );

  Widget rows(List<Json> tracks, {bool queue = false}) => Column(
    children: [
      for (var i = 0; i < tracks.length; i++) trackRow(tracks, i, queue: queue),
    ],
  );
  Widget grid(
    List<Widget> children, {
    required int columns,
    required double ratio,
    double gap = 24,
  }) => GridView.count(
    shrinkWrap: true,
    physics: const NeverScrollableScrollPhysics(),
    crossAxisCount: columns,
    crossAxisSpacing: gap,
    mainAxisSpacing: gap,
    childAspectRatio: ratio,
    children: children,
  );
  Widget actions(List<Widget> children) => Wrap(
    spacing: 12,
    runSpacing: 12,
    crossAxisAlignment: WrapCrossAlignment.center,
    children: children,
  );

  @override
  Widget build(BuildContext context) {
    final screenW = MediaQuery.sizeOf(context).width;
    showMatchPicker();
    if (_route != c.route) {
      _route = c.route;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (scroll.hasClients) scroll.jumpTo(0);
        if (c.route == 'search' && !c.updatePromptOpen) {
          searchFocus.requestFocus();
        }
      });
    }
    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.keyK, control: true): () {
          redesignPrefs.setImmersive(false);
          navigate('search');
          searchFocus.requestFocus();
        },
        const SingleActivator(LogicalKeyboardKey.escape): () {
          if (redesignPrefs.immersiveOpen) {
            redesignPrefs.setImmersive(false);
          } else if (redesignPrefs.miniPlayerOpen) {
            redesignPrefs.toggleMiniPlayer();
          }
        },
        const SingleActivator(LogicalKeyboardKey.arrowLeft, alt: true): c.back,
        const SingleActivator(LogicalKeyboardKey.space): () =>
            c.player('toggle'),
      },
      child: Focus(
        autofocus: true,
        child: Scaffold(
          backgroundColor: const Color(0xff141417),
          body: Stack(
            children: [
              Column(
                children: [
                  Expanded(
                    child: Row(
                      children: [
                        CiderIconRail(
                          core: c,
                          onOpenConnections: showConnections,
                        ),
                        if (redesignPrefs.showLibrarySidebar && screenW >= 780)
                          CiderLibraryPanel(
                            core: c,
                            onCreatePlaylist: () => createPlaylist(),
                          ),
                        Expanded(
                          child: Container(
                            color: const Color(0xff18181c),
                            child: Stack(
                              children: [
                                LayoutBuilder(
                                  builder: (context, box) => AnimatedSwitcher(
                                    duration: Duration(
                                      milliseconds: c.reduced ? 0 : 180,
                                    ),
                                    switchInCurve: Anicry.easeOut,
                                    transitionBuilder: (child, animation) =>
                                        FadeTransition(
                                          opacity: animation,
                                          child: child,
                                        ),
                                    child: SizedBox.expand(
                                      key: ValueKey(c.route),
                                      child: pageViewport(box.maxWidth - 40),
                                    ),
                                  ),
                                ),
                                Positioned(
                                  top: 0,
                                  left: 0,
                                  right: 0,
                                  height: 56,
                                  child: IgnorePointer(
                                    child: DecoratedBox(
                                      decoration: BoxDecoration(
                                        gradient: LinearGradient(
                                          begin: Alignment.topCenter,
                                          end: Alignment.bottomCenter,
                                          colors: [
                                            const Color(
                                              0xff18181c,
                                            ).withValues(alpha: .94),
                                            const Color(
                                              0xff18181c,
                                            ).withValues(alpha: 0),
                                          ],
                                        ),
                                      ),
                                    ),
                                  ),
                                ),
                                Positioned(
                                  top: 0,
                                  left: 0,
                                  right: 0,
                                  child: CiderTopBar(
                                    core: c,
                                    onCreatePlaylist: () => createPlaylist(),
                                    onOpenConnections: showConnections,
                                  ),
                                ),
                                if (c.error.isNotEmpty ||
                                    (c.state['message_error'] == true &&
                                        !(c.route == 'search' &&
                                            c.state['search_error'] ==
                                                c.state['message'])))
                                  Positioned(
                                    left: 20,
                                    bottom: 16,
                                    child: ConstrainedBox(
                                      constraints: const BoxConstraints(
                                        maxWidth: 460,
                                      ),
                                      child: ErrorNotice(
                                        c.error.isNotEmpty
                                            ? c.error.tr(context)
                                            : c.state['message']
                                                  .toString()
                                                  .tr(context),
                                        key: ValueKey(
                                          c.error.isNotEmpty
                                              ? c.error
                                              : c.state['message'],
                                        ),
                                        onDismiss: c.clearError,
                                      ),
                                    ),
                                  ),
                              ],
                            ),
                          ),
                        ),
                        if (redesignPrefs.showQueuePanel && screenW >= 1040)
                          CiderQueuePanel(core: c),
                      ],
                    ),
                  ),
                  CiderBottomBar(core: c, onAddPlaylist: addPlaylist),
                ],
              ),
              if (redesignPrefs.miniPlayerOpen)
                CiderMiniPlayerCard(core: c, onAddPlaylist: addPlaylist),
              if (redesignPrefs.immersiveOpen) CiderImmersiveOverlay(core: c),
            ],
          ),
        ),
      ),
    );
  }

  Widget pageViewport(double width) {
    const padding = EdgeInsets.fromLTRB(20, 60, 20, 28);
    // Match the collection fallback in page(), leaving other page layouts intact.
    final collection =
        c.ready &&
        !const {
          'home',
          'library',
          'profile',
          'creator',
          'search',
          'now',
          'settings',
          'downloads',
          'stats',
        }.contains(c.route);
    if (!collection) {
      return ListView(
        controller: scroll,
        padding: padding,
        children: [page(width)],
      );
    }
    final tracks = c.tracks;
    final queue = c.route == 'queue';
    return CustomScrollView(
      controller: scroll,
      slivers: [
        SliverPadding(
          padding: padding,
          sliver: SliverMainAxisGroup(
            slivers: [
              SliverToBoxAdapter(child: collectionHeader(width, tracks)),
              SliverFixedExtentList(
                itemExtent:
                    64, // TrackRow's explicit height at every text scale.
                delegate: SliverChildBuilderDelegate(
                  (context, index) => trackRow(tracks, index, queue: queue),
                  childCount: tracks.length,
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }

  Widget page(double width) {
    if (!c.ready) {
      return EmptyState(
        c.error.isEmpty
            ? 'Відкриваємо медіатеку…'.tr(context)
            : 'Не вдалося відкрити медіатеку'.tr(context),
        c.error.tr(context),
      );
    }
    switch (c.route) {
      case 'home':
        return DiscoveryPage(c);
      case 'library':
        return LibraryPage(c, onCreate: createPlaylist);
      case 'profile':
        return UserProfile(c, trackRows: (ts) => rows(ts));
      case 'creator':
        return CreatorProfile(c, trackRows: (ts) => rows(ts));
      case 'search':
        return SearchPage(
          c,
          controller: query,
          focusNode: searchFocus,
          onAddPlaylist: addPlaylist,
        );
      case 'now':
        return nowPlaying(width);
      case 'settings':
        return SettingsPage(c);
      case 'downloads':
        return downloads();
      case 'stats':
        return statistics();
      default:
        return collectionHeader(width, c.tracks);
    }
  }

  Widget collectionHeader(double width, List<Json> ts) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (c.route == 'playlist' || c.route == 'liked') ...[
          CiderHeroHeader(
            core: c,
            cover: ts.isEmpty ? '' : trackCover(ts.first),
            title: c.route == 'liked'
                ? 'Favorite Songs'
                : (c.playlist['name']?.toString() ?? 'Playlist'),
            subtitle: c.profile['name']?.toString() ?? 'SoundFlow Library',
            ambientKey: c.route == 'liked'
                ? 'favorite_songs'
                : c.selectedPlaylist,
            pills: [
              'Playlist',
              '${ts.length} tracks',
            ],
            onPlay: ts.isEmpty ? null : () => c.play(ts.first, ts),
            onShuffle: ts.isEmpty
                ? null
                : () {
                    final s = [...ts]..shuffle();
                    c.play(s.first, s);
                  },
            onMore: c.route == 'playlist'
                ? () async {
                    final value = await textDialog(
                      'Rename playlist',
                      'Name',
                      value: c.playlist['name'] ?? '',
                      confirm: 'Save',
                    );
                    if (value != null) {
                      c.call('rename_playlist', [c.selectedPlaylist, value]);
                    }
                  }
                : null,
          ),
          const SizedBox(height: 22),
        ],
        if (c.route == 'queue' && c.hasTrack) ...[
          Row(
            children: [
              Artwork(trackCover(c.current), size: 84, radius: 12),
              const SizedBox(width: 20),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      'Зараз звучить'.tr(context),
                      style: TextStyle(
                        color: context.paints.muted,
                        fontSize: 13,
                      ),
                    ),
                    const SizedBox(height: 7),
                    Text(
                      trackTitle(c.current, context),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                        fontSize: 24,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    const SizedBox(height: 4),
                    Text(
                      trackArtist(c.current, context),
                      style: TextStyle(color: context.paints.muted),
                    ),
                  ],
                ),
              ),
              Control(
                c.playing ? 'pause' : 'play',
                c.playing ? 'Пауза'.tr(context) : 'Продовжити'.tr(context),
                primary: true,
                size: 44,
                onTap: () => c.player('toggle'),
              ),
            ],
          ),
          const SizedBox(height: 30),
          Heading(
            'Далі звучатиме'.tr(context),
            action: ActionButton(
              'Очистити наступні'.tr(context),
              flat: true,
              onTap: () => c.action('queue_clear'),
            ),
          ),
          const SizedBox(height: 16),
        ],
        if (c.route != 'playlist' && c.route != 'queue') ...[
          actions([
            ActionButton(
              'Відтворити'.tr(context),
              icon: 'play',
              primary: true,
              onTap: ts.isEmpty ? null : () => c.play(ts.first, ts),
            ),
            if (c.route == 'history')
              ActionButton(
                'Статистика'.tr(context),
                flat: true,
                onTap: () => navigate('stats'),
              ),
          ]),
          const SizedBox(height: 20),
        ],
        if (c.pending > 0 && c.route == 'flow')
          const LinearProgressIndicator(minHeight: 2),
        if (ts.isEmpty && c.pending == 0)
          EmptyState(
            c.route == 'queue'
                ? 'Наступних треків ще немає'.tr(context)
                : 'Тут поки немає треків'.tr(context),
            'Знайди музику або додай трек із медіатеки.'.tr(context),
            action: ActionButton(
              'Знайти музику'.tr(context),
              onTap: () => navigate('search'),
            ),
          ),
      ],
    );
  }

  Widget nowPlaying(double width) => ValueListenableBuilder<Json>(
    valueListenable: c.playback,
    builder: (context, pb, _) {
      final t = object(pb['track']);
      if (t.isEmpty) {
        return EmptyState(
          'Оберіть трек'.tr(context),
          'Знайди музику або відкрий медіатеку.'.tr(context),
          action: ActionButton(
            'Знайти музику'.tr(context),
            primary: true,
            onTap: () => navigate('search'),
          ),
        );
      }
      final cover = math.min(480.0, width * .44),
          duration = (pb['duration'] as num?)?.toDouble() ?? 0,
          position = (pb['position'] as num?)?.toDouble() ?? 0;
      final status = pb['status']?.toString() ?? '';
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Artwork(trackCover(t), size: cover, radius: 22),
              SizedBox(width: width > 950 ? 48 : 28),
              Expanded(
                child: Padding(
                  padding: EdgeInsets.only(right: width > 950 ? 24 : 0),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        trackTitle(t, context),
                        style: TextStyle(
                          fontFamily: SoundFlowType.heading,
                          fontSize: width > 950 ? 36 : 27,
                          height: 1.12,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                      const SizedBox(height: 12),
                      InkWell(
                        onTap: () => c.showCreator(t),
                        child: Text(
                          trackArtist(t, context),
                          style: TextStyle(
                            fontSize: 18,
                            color: context.paints.muted,
                          ),
                        ),
                      ),
                      const SizedBox(height: 26),
                      PlaybackTimeline(
                        key: ValueKey(trackKey(t)),
                        position: position,
                        duration: duration,
                        fontSize: 12,
                        onSeek: duration > 0
                            ? (value) =>
                                  c.command({'type': 'seek', 'value': value})
                            : null,
                      ),
                      const SizedBox(height: 22),
                      Row(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          Control(
                            'shuffle',
                            'Перемішати'.tr(context),
                            active: c.settings['shuffle'] == true,
                            onTap: () => c.player('shuffle'),
                          ),
                          const Spacer(),
                          Control(
                            'skip-back',
                            'Попередній трек'.tr(context),
                            onTap: () => c.player('previous'),
                          ),
                          SizedBox(width: width < 850 ? 8 : 20),
                          Control(
                            pb['playing'] == true ? 'pause' : 'play',
                            pb['playing'] == true
                                ? 'Пауза'.tr(context)
                                : 'Відтворити'.tr(context),
                            primary: true,
                            size: 62,
                            onTap: () => c.player('toggle'),
                          ),
                          SizedBox(width: width < 850 ? 8 : 20),
                          Control(
                            'skip-forward',
                            'Наступний трек'.tr(context),
                            onTap: () => c.player('next'),
                          ),
                          const Spacer(),
                          Control(
                            'repeat',
                            'Повтор'.tr(context),
                            active: c.settings['repeat'] != 'off',
                            onTap: () => c.player('repeat'),
                          ),
                        ],
                      ),
                      const SizedBox(height: 22),
                      Row(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          Control(
                            'heart',
                            c.isLiked(t)
                                ? 'Прибрати з улюбленого'.tr(context)
                                : 'Додати в улюблене'.tr(context),
                            active: c.isLiked(t),
                            onTap: () => c.action('like', t),
                          ),
                          const SizedBox(width: 12),
                          Glyph(
                            'volume-2',
                            size: 18,
                            color: context.paints.muted,
                          ),
                          SizedBox(
                            width: width < 850 ? 90 : 150,
                            child: MediaSlider(
                              continuous: true,
                              value: ((pb['volume'] as num?)?.toDouble() ?? .8)
                                  .clamp(0, 1),
                              onChanged: (v) =>
                                  c.command({'type': 'volume', 'value': v}),
                            ),
                          ),
                          Control(
                            'expand',
                            'Мініплеєр'.tr(context),
                            onTap: c.island,
                          ),
                        ],
                      ),
                      const SizedBox(height: 12),
                      Center(
                        child: Text(
                          status.isNotEmpty
                              ? status.tr(context)
                              : pb['loading'] == true
                              ? 'Завантажуємо…'.tr(context)
                              : sourceName(t, context),
                          style: TextStyle(
                            fontSize: 12,
                            color: context.paints.muted,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 32),
          Heading(
            'Далі в черзі'.tr(context),
            action: ActionButton(
              'Уся черга'.tr(context),
              flat: true,
              onTap: () => navigate('queue'),
            ),
          ),
          const SizedBox(height: 16),
          rows(c.queue.skip(c.queueIndex + 1).take(4).toList(), queue: true),
        ],
      );
    },
  );

  Widget downloads() {
    final jobs = objects(c.state['downloads']);
    return jobs.isEmpty
        ? EmptyState(
            'Музика без мережі'.tr(context),
            'Збережені завантаження з’являться тут.'.tr(context),
            action: ActionButton(
              'Відкрити папку'.tr(context),
              onTap: () => c.call('open_downloads_directory'),
            ),
          )
        : Column(
            children: [
              for (final job in jobs)
                Padding(
                  padding: const EdgeInsets.only(bottom: 20),
                  child: Row(
                    children: [
                      Artwork(trackCover(object(job['track'])), size: 52),
                      const SizedBox(width: 16),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              trackTitle(
                                job['track'] is Map
                                    ? object(job['track'])
                                    : job,
                                context,
                              ),
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ),
                            const SizedBox(height: 7),
                            Text(
                              (job['status']?.toString() ?? '').tr(context),
                              style: TextStyle(
                                color: context.paints.muted,
                                fontSize: 12,
                              ),
                            ),
                            const SizedBox(height: 8),
                            LinearProgressIndicator(
                              value:
                                  (((job['progress'] as num?)?.toDouble() ??
                                              0) /
                                          100)
                                      .clamp(0, 1),
                              minHeight: 3,
                            ),
                          ],
                        ),
                      ),
                      const SizedBox(width: 20),
                      Control(
                        job['status'] == 'completed' ? 'arrow-up-right' : 'x',
                        job['status'] == 'completed'
                            ? 'Відкрити'.tr(context)
                            : 'Скасувати'.tr(context),
                        onTap: () => c.call(
                          job['status'] == 'completed'
                              ? 'reveal_download'
                              : 'cancel_download',
                          [job['id']],
                        ),
                      ),
                    ],
                  ),
                ),
            ],
          );
  }

  Widget statistics() {
    final stats = object(c.state['stats']);
    if (stats.isEmpty) {
      return EmptyState(
        c.pending > 0
            ? 'Завантажуємо статистику…'.tr(context)
            : 'Ще немає статистики'.tr(context),
        'Тут з’являться дані прослуховування.'.tr(context),
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (stats['locked'] == true)
          Text(
            stats['message']?.toString() ??
                stats['error']?.toString() ??
                'Статистика доступна з SoundFlow Plus'.tr(context),
            style: const TextStyle(fontSize: 18),
          ),
        if (stats['locked'] != true)
          Text(
            '{plays} прослуховувань\n{tracks} треків · {artists} виконавців'
                .tr(context)
                .replaceAll('{plays}', '${stats['plays'] ?? 0}')
                .replaceAll('{tracks}', '${stats['unique_tracks'] ?? 0}')
                .replaceAll('{artists}', '${stats['unique_artists'] ?? 0}'),
            style: const TextStyle(fontSize: 24, height: 1.8),
          ),
        const SizedBox(height: 32),
        Heading('Нещодавно слухали'.tr(context)),
        const SizedBox(height: 16),
        rows(c.recent.take(20).toList()),
      ],
    );
  }
}
