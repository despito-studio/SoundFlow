import 'l10n.dart';

import 'package:flutter/material.dart';

import 'anicry_motion.dart';
import 'core.dart';
import 'design.dart';

class SearchPage extends StatefulWidget {
  const SearchPage(
    this.core, {
    super.key,
    required this.controller,
    required this.focusNode,
    required this.onAddPlaylist,
  });
  final Core core;
  final TextEditingController controller;
  final FocusNode focusNode;
  final ValueChanged<Json> onAddPlaylist;
  @override
  State<SearchPage> createState() => _SearchPageState();
}

class _SearchPageState extends State<SearchPage> {
  Core get c => widget.core;
  String source = 'all', submitted = '', validation = '';
  int? requestedRevision;
  String get draft => widget.controller.text.trim();
  bool get active => submitted.isNotEmpty && draft == submitted;
  bool spotifyLink(String value) {
    final uri = Uri.tryParse(value);
    return uri?.scheme == 'spotify' ||
        (uri?.scheme == 'https' && uri?.host == 'open.spotify.com');
  }

  @override
  void initState() {
    super.initState();
    source = c.state['search_source']?.toString() ?? 'all';
    submitted = c.state['search_query']?.toString() ?? '';
    widget.controller.addListener(changed);
    widget.focusNode.addListener(changed);
  }

  void changed() {
    if (mounted) setState(() => validation = '');
  }

  @override
  void dispose() {
    widget.controller.removeListener(changed);
    widget.focusNode.removeListener(changed);
    super.dispose();
  }

  void search([String? value]) {
    if (value != null) {
      widget.controller.value = TextEditingValue(
        text: value,
        selection: TextSelection.collapsed(offset: value.length),
      );
    }
    if (source == 'spotify' && draft.isNotEmpty && !spotifyLink(draft)) {
      setState(
        () => validation = 'Встав посилання на трек Spotify або обери YouTube чи SoundCloud для пошуку за назвою.'
            .tr(context),
      );
      return;
    }
    setState(() {
      submitted = draft;
      requestedRevision = (c.state['revision'] as num?)?.toInt();
      validation = '';
    });
    c.search(draft, source);
  }

  void chooseSource(String value) {
    setState(() {
      source = value;
      validation = '';
      if (value == 'spotify' && !spotifyLink(draft)) submitted = '';
    });
    if (draft.isNotEmpty && (value != 'spotify' || spotifyLink(draft))) {
      search();
    }
  }

  @override
  Widget build(BuildContext context) {
    final p = context.paints;
    final matchesRequest =
        c.state['search_query'] == submitted &&
        c.state['search_source'] == source;
    final results = active && matchesRequest
        ? objects(c.state['search']).where(onlineTrack).toList()
        : <Json>[];
    final busy =
        active &&
        (c.state['search_loading'] == true ||
            (requestedRevision != null &&
                c.state['revision'] == requestedRevision));
    final error = active && matchesRequest
        ? c.state['search_error']?.toString() ?? ''
        : '';
    final history =
        (c.library['search_history'] is List
                ? c.library['search_history'] as List
                : const [])
            .whereType<String>()
            .where((q) => q.isNotEmpty)
            .take(8)
            .toList();
    final local = draft.isEmpty
        ? <Json>[]
        : c.allTracks
              .where(
                (t) => '${trackTitle(t, context)} ${trackArtist(t, context)}'
                    .toLowerCase()
                    .contains(draft.toLowerCase()),
              )
              .take(6)
              .toList();
    final playlists = c.playlists
        .where(
          (v) =>
              draft.isEmpty ||
              (v['name']?.toString() ?? '').toLowerCase().contains(
                draft.toLowerCase(),
              ),
        )
        .take(4)
        .toList();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Container(
          padding: const EdgeInsets.all(6),
          decoration: BoxDecoration(
            color: p.surface,
            borderRadius: BorderRadius.circular(18),
            border: Border.all(
              color: widget.focusNode.hasFocus
                  ? p.accent.withValues(alpha: .5)
                  : p.ink.withValues(alpha: .07),
            ),
          ),
          child: Row(
            children: [
              const SizedBox(width: 12),
              Glyph('search', size: 21, color: p.muted),
              const SizedBox(width: 8),
              Expanded(
                child: TextField(
                  controller: widget.controller,
                  focusNode: widget.focusNode,
                  maxLength: 500,
                  textInputAction: TextInputAction.search,
                  style: const TextStyle(fontSize: 14),
                  onSubmitted: (_) => search(),
                  // Playback controls should not discard the desktop search focus.
                  onTapOutside: (_) {},
                  decoration: InputDecoration(
                    hintText: source == 'spotify'
                        ? 'Посилання на трек Spotify'.tr(context)
                        : 'Назва треку, автор або посилання'.tr(context),
                    counterText: '',
                    filled: false,
                    border: InputBorder.none,
                    enabledBorder: InputBorder.none,
                    focusedBorder: InputBorder.none,
                    contentPadding: const EdgeInsets.symmetric(
                      horizontal: 8,
                      vertical: 10,
                    ),
                  ),
                ),
              ),
              if (draft.isNotEmpty)
                Control(
                  'x',
                  'Очистити пошук'.tr(context),
                  size: 32,
                  glyphSize: 17,
                  onTap: () {
                    widget.controller.clear();
                    search();
                    widget.focusNode.requestFocus();
                  },
                ),
              const SizedBox(width: 8),
              ActionButton(
                'Знайти'.tr(context),
                primary: true,
                onTap: draft.isEmpty ? null : search,
              ),
            ],
          ),
        ),
        const SizedBox(height: 14),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            for (final option in [
              ('all', 'Усюди'.tr(context)),
              ('youtube', 'YouTube'),
              ('soundcloud', 'SoundCloud'),
              ('spotify', 'Spotify · посилання'.tr(context)),
            ])
              _SourceChip(
                option.$2,
                selected: source == option.$1,
                onTap: () => chooseSource(option.$1),
              ),
          ],
        ),
        if (source == 'spotify' || validation.isNotEmpty) ...[
          const SizedBox(height: 12),
          Text(
            validation.isNotEmpty
                ? validation
                : 'Spotify: встав посилання на пісню. Для пошуку за назвою обери інше джерело.'
                      .tr(context),
            style: TextStyle(
              fontSize: 12,
              height: 1.5,
              color: validation.isNotEmpty
                  ? Theme.of(context).colorScheme.error
                  : p.muted,
            ),
          ),
        ],
        if (busy) ...[
          const SizedBox(height: 24),
          LinearProgressIndicator(
            minHeight: 2,
            color: p.accent,
            backgroundColor: p.surface,
          ),
          const SizedBox(height: 12),
          Text(
            'Шукаємо «{query}»…'.tr(context).replaceAll('{query}', submitted),
            style: TextStyle(fontSize: 13, color: p.muted),
          ),
        ],
        if (active && !busy && error.isNotEmpty) ...[
          const SizedBox(height: 28),
          EmptyState(
            'Не вдалося виконати пошук'.tr(context),
            error.tr(context),
            action: ActionButton(
              'Спробувати ще раз'.tr(context),
              icon: 'rotate-cw',
              onTap: search,
            ),
          ),
        ] else if (active && results.isNotEmpty) ...[
          const SizedBox(height: 28),
          _resultHighlights(context, results),
          const SizedBox(height: 30),
          _sectionHeading(
            context,
            'Треки'.tr(context),
            subtitle: '{count} знайдено'
                .tr(context)
                .replaceAll('{count}', results.length.toString()),
          ),
          const SizedBox(height: 12),
          for (final t in results)
            TrackRow(
              t,
              key: ValueKey(trackKey(t)),
              core: c,
              quiet: true,
              onPlay: () => c.play(t, results),
              onAddPlaylist: () => widget.onAddPlaylist(t),
            ),
        ] else if (active && !busy && error.isEmpty) ...[
          const SizedBox(height: 28),
          EmptyState(
            'За запитом «{query}» нічого не знайшли'
                .tr(context)
                .replaceAll('{query}', submitted),
            'Спробуй коротшу назву, ім’я автора або інше джерело.'.tr(context),
          ),
        ] else if (!active) ...[
          if (history.isNotEmpty) ...[
            const SizedBox(height: 26),
            _sectionHeading(
              context,
              'Останні запити'.tr(context),
              action: ActionButton(
                'Очистити'.tr(context),
                flat: true,
                compact: true,
                onTap: () => c.call('clear_search_history'),
              ),
            ),
            const SizedBox(height: 10),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                for (final q in history)
                  _HistoryChip(
                    q,
                    onTap: () => search(q),
                    onRemove: () => c.call('remove_search_query', [q]),
                  ),
              ],
            ),
          ],
          const SizedBox(height: 28),
          LayoutBuilder(
            builder: (context, box) {
              final recent = draft.isEmpty
                  ? _unique(c.recent).take(6).toList()
                  : local;
              final left = Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  _sectionHeading(
                    context,
                    draft.isEmpty
                        ? 'Нещодавно слухали'.tr(context)
                        : 'З твоєї медіатеки'.tr(context),
                    subtitle: draft.isEmpty
                        ? null
                        : 'Збіги за назвою або автором'.tr(context),
                  ),
                  const SizedBox(height: 16),
                  if (recent.isEmpty)
                    _HintCard(
                      icon: 'audio-lines',
                      title: draft.isEmpty
                          ? 'Нещодавніх треків ще немає'.tr(context)
                          : 'Шукай більше музики'.tr(context),
                      description: draft.isEmpty
                          ? 'Введи назву, ім’я автора або встав посилання на трек.'
                                .tr(context)
                          : 'Натисни Enter або «Знайти», щоб переглянути результати з музичних сервісів.'
                                .tr(context),
                    )
                  else
                    LayoutBuilder(
                      builder: (context, inner) {
                        final columns = inner.maxWidth >= 620 ? 2 : 1;
                        return Wrap(
                          spacing: 12,
                          runSpacing: 12,
                          children: [
                            for (final t in recent)
                              SizedBox(
                                width:
                                    (inner.maxWidth - (columns - 1) * 12) /
                                    columns,
                                child: _QuickTrack(
                                  t,
                                  core: c,
                                  onPlay: () => c.play(t, recent),
                                ),
                              ),
                          ],
                        );
                      },
                    ),
                ],
              );
              final right = _librarySuggestions(context, playlists);
              return box.maxWidth >= 1020
                  ? Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Expanded(flex: 7, child: left),
                        const SizedBox(width: 28),
                        Expanded(flex: 3, child: right),
                      ],
                    )
                  : Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [left, const SizedBox(height: 26), right],
                    );
            },
          ),
        ],
      ],
    );
  }

  List<Json> _unique(List<Json> tracks) {
    final seen = <String>{};
    return tracks.where((t) => seen.add(trackKey(t))).toList();
  }

  Widget _sectionHeading(
    BuildContext context,
    String title, {
    String? subtitle,
    Widget? action,
  }) => Row(
    children: [
      Expanded(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              title,
              style: const TextStyle(
                fontFamily: SoundFlowType.heading,
                fontSize: 16,
                fontWeight: FontWeight.w600,
              ),
            ),
            if (subtitle != null) ...[
              const SizedBox(height: 5),
              Text(
                subtitle,
                style: TextStyle(fontSize: 12, color: context.paints.muted),
              ),
            ],
          ],
        ),
      ),
      ?action,
    ],
  );

  Widget _librarySuggestions(BuildContext context, List<Json> playlists) =>
      Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _sectionHeading(
            context,
            'Твої добірки'.tr(context),
            action: Control(
              'arrow-up-right',
              'Відкрити медіатеку'.tr(context),
              size: 30,
              glyphSize: 17,
              onTap: () => c.nav('library'),
            ),
          ),
          const SizedBox(height: 16),
          if (playlists.isEmpty)
            _HintCard(
              icon: 'list',
              title: draft.isEmpty
                  ? 'Плейлістів ще немає'.tr(context)
                  : 'Плейлістів із такою назвою немає'.tr(context),
              description: 'Збирай знахідки в плейлісти у своїй медіатеці.'.tr(
                context,
              ),
              action: ActionButton(
                'Медіатека'.tr(context),
                flat: true,
                compact: true,
                icon: 'arrow-right',
                onTap: () => c.nav('library'),
              ),
            )
          else
            for (final playlist in playlists) ...[
              Material(
                color: Colors.transparent,
                borderRadius: BorderRadius.circular(14),
                child: AnicryTap(
                  radius: 14,
                  onTap: () => c.selectPlaylist(playlist),
                  child: Padding(
                    padding: const EdgeInsets.all(12),
                    child: Row(
                      children: [
                        Artwork(
                          objects(playlist['tracks']).isEmpty
                              ? ''
                              : trackCover(objects(playlist['tracks']).first),
                          size: 44,
                          radius: 9,
                        ),
                        const SizedBox(width: 12),
                        Expanded(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(
                                playlist['name']?.toString() ??
                                    'Плейліст'.tr(context),
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: const TextStyle(
                                  fontSize: 13,
                                  fontWeight: FontWeight.w600,
                                ),
                              ),
                              const SizedBox(height: 5),
                              Text(
                                trackCount(
                                  objects(playlist['tracks']).length,
                                  context,
                                ),
                                style: TextStyle(
                                  fontSize: 12,
                                  color: context.paints.muted,
                                ),
                              ),
                            ],
                          ),
                        ),
                        const SizedBox(width: 8),
                        Glyph(
                          'chevron-right',
                          size: 16,
                          color: context.paints.muted,
                        ),
                      ],
                    ),
                  ),
                ),
              ),
              const SizedBox(height: 10),
            ],
        ],
      );

  Widget _resultHighlights(BuildContext context, List<Json> results) {
    final top = results.first;
    final authors = <String, Json>{};
    for (final t in results) {
      authors.putIfAbsent('${t['source']}:${trackArtist(t, context)}', () => t);
    }
    final best = Container(
      padding: const EdgeInsets.all(20),
      decoration: BoxDecoration(
        color: context.paints.surface.withValues(alpha: .5),
        borderRadius: BorderRadius.circular(22),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            'Найкращий збіг'.tr(context),
            style: TextStyle(
              fontSize: 13,
              fontWeight: FontWeight.w600,
              color: context.paints.accent,
            ),
          ),
          const SizedBox(height: 18),
          Row(
            children: [
              Artwork(trackCover(top), size: 88, radius: 15),
              const SizedBox(width: 18),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      trackTitle(top, context),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                        fontSize: 21,
                        height: 1.2,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    const SizedBox(height: 7),
                    Text(
                      trackArtist(top, context),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        fontSize: 13,
                        color: context.paints.muted,
                      ),
                    ),
                    const SizedBox(height: 7),
                    Text(
                      sourceName(top, context),
                      style: TextStyle(
                        fontSize: 11,
                        color: context.paints.muted,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
          const SizedBox(height: 18),
          Row(
            children: [
              ActionButton(
                'Слухати'.tr(context),
                icon: 'play',
                primary: true,
                onTap: () => c.play(top, results),
              ),
              const Spacer(),
              Control(
                'heart',
                c.isLiked(top)
                    ? 'Прибрати з улюбленого'.tr(context)
                    : 'Додати в улюблене'.tr(context),
                active: c.isLiked(top),
                onTap: () => c.action('like', top),
              ),
            ],
          ),
        ],
      ),
    );
    final artists = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _sectionHeading(context, 'Автори в результатах'.tr(context)),
        const SizedBox(height: 16),
        Wrap(
          spacing: 10,
          runSpacing: 10,
          children: [
            for (final t in authors.values.take(6))
              _SourceChip(
                trackArtist(t, context),
                selected: false,
                onTap: () => c.showCreator(t),
              ),
          ],
        ),
      ],
    );
    return LayoutBuilder(
      builder: (context, box) => box.maxWidth >= 900
          ? Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                SizedBox(width: box.maxWidth * .42, child: best),
                const SizedBox(width: 28),
                Expanded(child: artists),
              ],
            )
          : Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [best, const SizedBox(height: 24), artists],
            ),
    );
  }
}

class _SourceChip extends StatelessWidget {
  const _SourceChip(this.label, {required this.selected, required this.onTap});
  final String label;
  final bool selected;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) {
    final p = context.paints;
    return Semantics(
      button: true,
      selected: selected,
      label: label,
      child: Material(
        color: selected
            ? p.accent.withValues(alpha: .14)
            : p.surface.withValues(alpha: .6),
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(11),
          side: BorderSide(
            color: selected
                ? p.accent.withValues(alpha: .3)
                : p.ink.withValues(alpha: .06),
          ),
        ),
        child: AnicryTap(
          radius: 11,
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 13, vertical: 10),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Flexible(
                  child: Text(
                    label,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      fontSize: 12,
                      fontWeight: selected ? FontWeight.w600 : FontWeight.w400,
                      color: selected ? p.accent : p.ink,
                    ),
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

class _HistoryChip extends StatelessWidget {
  const _HistoryChip(this.query, {required this.onTap, required this.onRemove});
  final String query;
  final VoidCallback onTap, onRemove;
  @override
  Widget build(BuildContext context) => ConstrainedBox(
    constraints: const BoxConstraints(maxWidth: 280),
    child: Material(
      color: Colors.transparent,
      borderRadius: BorderRadius.circular(10),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Flexible(
            child: AnicryTap(
              radius: 10,
              onTap: onTap,
              child: Padding(
                padding: const EdgeInsets.fromLTRB(12, 9, 4, 9),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Glyph('history', size: 13, color: context.paints.muted),
                    const SizedBox(width: 8),
                    Flexible(
                      child: Text(
                        query,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(fontSize: 12),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
          Control(
            'x',
            'Прибрати запит «{query}»'.tr(context).replaceAll('{query}', query),
            size: 28,
            glyphSize: 13,
            onTap: onRemove,
          ),
          const SizedBox(width: 3),
        ],
      ),
    ),
  );
}

class _QuickTrack extends StatelessWidget {
  const _QuickTrack(this.track, {required this.core, required this.onPlay});
  final Json track;
  final Core core;
  final VoidCallback onPlay;
  @override
  Widget build(BuildContext context) => Material(
    color: Colors.transparent,
    borderRadius: BorderRadius.circular(15),
    child: AnicryTap(
      radius: 15,
      onTap: onPlay,
      child: Padding(
        padding: const EdgeInsets.all(10),
        child: Row(
          children: [
            Artwork(trackCover(track), size: 44, radius: 10),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    trackTitle(track, context),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                      fontSize: 13,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                  const SizedBox(height: 5),
                  Text(
                    trackArtist(track, context),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(fontSize: 12, color: context.paints.muted),
                  ),
                ],
              ),
            ),
            const SizedBox(width: 8),
            Glyph(
              trackKey(core.current) == trackKey(track) && core.playing
                  ? 'pause'
                  : 'play',
              size: 18,
              color: context.paints.accent,
            ),
          ],
        ),
      ),
    ),
  );
}

class _HintCard extends StatelessWidget {
  const _HintCard({
    required this.icon,
    required this.title,
    required this.description,
    this.action,
  });
  final String icon, title, description;
  final Widget? action;
  @override
  Widget build(BuildContext context) => Container(
    width: double.infinity,
    padding: const EdgeInsets.all(24),
    decoration: BoxDecoration(
      color: context.paints.surface.withValues(alpha: .5),
      borderRadius: BorderRadius.circular(18),
      border: Border.all(color: context.paints.ink.withValues(alpha: .06)),
    ),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Glyph(icon, size: 24, color: context.paints.accent),
        const SizedBox(height: 14),
        Text(
          title,
          style: const TextStyle(
            fontFamily: SoundFlowType.heading,
            fontSize: 16,
            fontWeight: FontWeight.w600,
          ),
        ),
        const SizedBox(height: 8),
        Text(
          description,
          style: TextStyle(
            fontSize: 13,
            height: 1.5,
            color: context.paints.muted,
          ),
        ),
        if (action != null) ...[const SizedBox(height: 14), action!],
      ],
    ),
  );
}
