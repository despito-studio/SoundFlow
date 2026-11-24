import 'l10n.dart';

import 'dart:math' as math;

import 'package:flutter/material.dart';

import 'cider_shell.dart';
import 'core.dart';
import 'design.dart';
import 'visualizer.dart';

class DiscoveryPage extends StatefulWidget {
  const DiscoveryPage(this.core, {super.key});
  final Core core;
  @override
  State<DiscoveryPage> createState() => _DiscoveryPageState();
}

class _DiscoveryPageState extends State<DiscoveryPage> {
  String source = 'all';
  Core get c => widget.core;
  List<Json> filtered(List<Json> tracks) => tracks
      .where(
        (t) => onlineTrack(t) && (source == 'all' || t['source'] == source),
      )
      .toList();

  Widget shelf(String title, List<Json> tracks, {String? subtitle}) {
    if (tracks.isEmpty) return const SizedBox.shrink();
    return Padding(
      padding: const EdgeInsets.only(top: 32),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Heading(title),
          if (subtitle != null) ...[
            const SizedBox(height: 6),
            Text(
              subtitle,
              style: TextStyle(color: context.paints.muted, fontSize: 13),
            ),
          ],
          const SizedBox(height: 16),
          LayoutBuilder(
            builder: (context, box) {
              final count = math.max(3, (box.maxWidth / 210).floor());
              final width = (box.maxWidth - (count - 1) * 16) / count;
              return SizedBox(
                height: width * 1.28,
                child: ListView.separated(
                  scrollDirection: Axis.horizontal,
                  itemCount: tracks.length,
                  separatorBuilder: (_, _) => const SizedBox(width: 16),
                  itemBuilder: (context, i) => SizedBox(
                    width: width,
                    child: AlbumCard(
                      tracks[i],
                      reduced: c.reduced,
                      playing:
                          trackKey(c.current) == trackKey(tracks[i]) &&
                          c.playing,
                      onTap: () => c.play(tracks[i], tracks),
                    ),
                  ),
                ),
              );
            },
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final recent = filtered(c.homeTracks).take(6).toList();
    final feed = c.feed;
    final youtube = filtered(objects(feed['youtube']));
    final soundcloud = filtered(objects(feed['soundcloud']));
    final shown = {...youtube.map(trackKey), ...soundcloud.map(trackKey)};
    final recommendations = filtered(objects(feed['recommendations']))
        .where((t) => !shown.contains(trackKey(t)))
        .toList();
    final artists = <String, Json>{};
    for (final t in [...c.homeTracks, ...youtube, ...soundcloud]) {
      if (source == 'all' || t['source'] == source) {
        artists.putIfAbsent('${t['source']}:${trackArtist(t, context)}', () => t);
      }
    }
    final p = context.paints;
    final allHeroPool = <Json>[
      if (c.hasTrack) c.current,
      ...recent,
      ...recommendations,
      ...youtube,
      ...soundcloud,
    ];
    final heroTrack = allHeroPool.isNotEmpty ? allHeroPool.first : null;
    final heroQueue = allHeroPool.isNotEmpty ? allHeroPool : <Json>[];
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (heroTrack != null) ...[
          CiderHeroHeader(
            core: c,
            cover: trackCover(heroTrack),
            title: trackTitle(heroTrack, context),
            subtitle: trackArtist(heroTrack, context),
            ambientKey: trackKey(heroTrack),
            pills: [
              'Single',
              sourceName(heroTrack, context),
              if (heroTrack['duration_formatted'] != null)
                heroTrack['duration_formatted'].toString(),
            ],
            onPlay: () => c.play(heroTrack, heroQueue),
            onShuffle: () {
              final shuffled = [...heroQueue]..shuffle();
              if (shuffled.isNotEmpty) c.play(shuffled.first, shuffled);
            },
            onAdd: () => c.action('like', heroTrack),
            onMore: redesignPrefs.toggleImmersive,
          ),
          const SizedBox(height: 22),
        ],
        Wrap(
          spacing: 10,
          runSpacing: 10,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            Container(
              padding: const EdgeInsets.all(3),
              decoration: BoxDecoration(
                color: p.surface,
                borderRadius: BorderRadius.circular(11),
                border: Border.all(color: p.ink.withValues(alpha: .08)),
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  for (final s in [
                    ('all', 'Усе'.tr(context)),
                    ('youtube', 'YouTube'),
                    ('soundcloud', 'SoundCloud'),
                  ])
                    Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 1),
                      child: Material(
                        color: source == s.$1
                            ? p.ink.withValues(alpha: .11)
                            : Colors.transparent,
                        borderRadius: BorderRadius.circular(8),
                        child: InkWell(
                          onTap: () => setState(() => source = s.$1),
                          borderRadius: BorderRadius.circular(8),
                          child: Padding(
                            padding: const EdgeInsets.symmetric(
                              horizontal: 12,
                              vertical: 6,
                            ),
                            child: Text(
                              s.$2,
                              style: TextStyle(
                                fontSize: 12,
                                fontWeight: source == s.$1
                                    ? FontWeight.w600
                                    : FontWeight.w500,
                                color: source == s.$1 ? p.ink : p.muted,
                              ),
                            ),
                          ),
                        ),
                      ),
                    ),
                ],
              ),
            ),
            ActionButton(
              'Інші треки'.tr(context),
              compact: true,
              onTap: c.pending > 0
                  ? null
                  : () => c.call('get_home_feed', [true]),
            ),
          ],
        ),
        if (recent.isNotEmpty) ...[
          const SizedBox(height: 24),
          Heading(
            'Продовжити слухати'.tr(context),
            action: ActionButton(
              'Історія'.tr(context),
              flat: true,
              onTap: () => c.nav('history'),
            ),
          ),
          const SizedBox(height: 14),
          LayoutBuilder(
            builder: (context, box) {
              final columns = box.maxWidth > 940 ? 3 : 2;
              return GridView.count(
                crossAxisCount: columns,
                childAspectRatio:
                    (box.maxWidth - (columns - 1) * 12) / columns / 68,
                crossAxisSpacing: 12,
                mainAxisSpacing: 12,
                shrinkWrap: true,
                physics: const NeverScrollableScrollPhysics(),
                children: [
                  for (final t in recent)
                    Builder(
                      builder: (context) {
                        final isCurrent = trackKey(c.current) == trackKey(t);
                        final isPlaying = isCurrent && c.playing;
                        return Container(
                          decoration: BoxDecoration(
                            color: isCurrent ? p.hover : p.surface,
                            borderRadius: BorderRadius.circular(13),
                            border: Border.all(
                              color: isCurrent
                                  ? p.accent.withValues(alpha: .45)
                                  : p.ink.withValues(alpha: .06),
                            ),
                          ),
                          child: Material(
                            color: Colors.transparent,
                            borderRadius: BorderRadius.circular(13),
                            child: InkWell(
                              onTap: () => c.play(t, recent),
                              borderRadius: BorderRadius.circular(13),
                              child: Padding(
                                padding: const EdgeInsets.fromLTRB(
                                  9,
                                  8,
                                  12,
                                  8,
                                ),
                                child: Row(
                                  children: [
                                    Artwork(trackCover(t), size: 50, radius: 9),
                                    const SizedBox(width: 12),
                                    Expanded(
                                      child: Column(
                                        mainAxisAlignment:
                                            MainAxisAlignment.center,
                                        crossAxisAlignment:
                                            CrossAxisAlignment.start,
                                        children: [
                                          Text(
                                            trackTitle(t, context),
                                            maxLines: 1,
                                            overflow: TextOverflow.ellipsis,
                                            style: TextStyle(
                                              fontWeight: FontWeight.w600,
                                              fontSize: 13.5,
                                              color: isCurrent
                                                  ? p.accent
                                                  : p.ink,
                                            ),
                                          ),
                                          const SizedBox(height: 3),
                                          Text(
                                            trackArtist(t, context),
                                            maxLines: 1,
                                            overflow: TextOverflow.ellipsis,
                                            style: TextStyle(
                                              fontSize: 12,
                                              color: p.muted,
                                            ),
                                          ),
                                        ],
                                      ),
                                    ),
                                    const SizedBox(width: 8),
                                    if (isPlaying)
                                      MiniEqBars(
                                        playing: true,
                                        color: p.accent,
                                      )
                                    else
                                      Container(
                                        width: 30,
                                        height: 30,
                                        decoration: BoxDecoration(
                                          color: isCurrent
                                              ? p.accent.withValues(alpha: .16)
                                              : p.ink.withValues(alpha: .05),
                                          shape: BoxShape.circle,
                                        ),
                                        child: Center(
                                          child: Glyph(
                                            'play',
                                            filled: true,
                                            size: 14,
                                            color: isCurrent
                                                ? p.accent
                                                : p.ink.withValues(alpha: .82),
                                          ),
                                        ),
                                      ),
                                  ],
                                ),
                              ),
                            ),
                          ),
                        );
                      },
                    ),
                ],
              );
            },
          ),
        ],
        shelf(
          feed['personalized'] == true
              ? 'Більше того, що подобається'.tr(context)
              : 'Відкрити для себе'.tr(context),
          recommendations,
          subtitle: feed['personalized'] == true
              ? 'За твоїми останніми прослуховуваннями'.tr(context)
              : 'Музика з YouTube та SoundCloud'.tr(context),
        ),
        shelf('З YouTube'.tr(context), youtube),
        shelf('Із SoundCloud'.tr(context), soundcloud),
        if (feed['loaded'] != true)
          Padding(
            padding: EdgeInsets.symmetric(vertical: 32),
            child: Row(
              children: [
                SizedBox(
                  width: 16,
                  height: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
                SizedBox(width: 12),
                Text('Знаходимо музику для тебе…'.tr(context)),
              ],
            ),
          ),
        if (feed['loaded'] == true &&
            youtube.isEmpty &&
            soundcloud.isEmpty &&
            recommendations.isEmpty)
          Padding(
            padding: const EdgeInsets.only(top: 32),
            child: EmptyState(
              'Добірки поки недоступні'.tr(context),
              'Перевір підключення або знайди музику через пошук.'.tr(context),
              action: ActionButton(
                'Оновити добірки'.tr(context),
                onTap: () => c.call('get_home_feed', [true]),
              ),
            ),
          ),
        if (artists.isNotEmpty) ...[
          const SizedBox(height: 32),
          Heading('Автори'.tr(context)),
          const SizedBox(height: 16),
          SizedBox(
            height: 214,
            child: ListView.separated(
              scrollDirection: Axis.horizontal,
              itemCount: math.min(artists.length, 12),
              separatorBuilder: (_, _) => const SizedBox(width: 12),
              itemBuilder: (context, i) => SizedBox(
                width: 160,
                child: ArtistCard(artists.values.elementAt(i), core: c),
              ),
            ),
          ),
        ],
        if (c.playlists.isNotEmpty) ...[
          const SizedBox(height: 32),
          Heading(
            'З твоєї медіатеки'.tr(context),
            action: ActionButton(
              'Усе'.tr(context),
              flat: true,
              onTap: () => c.nav('library'),
            ),
          ),
          const SizedBox(height: 16),
          Wrap(
            spacing: 16,
            runSpacing: 16,
            children: [
              for (final p in c.playlists.take(6))
                SizedBox(
                  width: 280,
                  height: 88,
                  child: PlaylistCard(
                    p,
                    compact: true,
                    onTap: () => c.selectPlaylist(p),
                  ),
                ),
            ],
          ),
        ],
      ],
    );
  }
}

class LibraryPage extends StatelessWidget {
  const LibraryPage(this.core, {super.key, required this.onCreate});
  final Core core;
  final VoidCallback onCreate;
  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      Wrap(
        spacing: 12,
        runSpacing: 12,
        children: [
          ActionButton(
            'Новий плейліст'.tr(context),
            icon: 'plus',
            primary: true,
            onTap: onCreate,
          ),
          ActionButton(
            'Імпорт із сервісів'.tr(context),
            onTap: () => core.nav('connections'),
          ),
        ],
      ),
      const SizedBox(height: 28),
      Material(
        color: context.paints.surface,
        borderRadius: BorderRadius.circular(18),
        child: InkWell(
          onTap: () => core.nav('liked'),
          borderRadius: BorderRadius.circular(18),
          child: Padding(
            padding: const EdgeInsets.all(20),
            child: Row(
              children: [
                Container(
                  width: 64,
                  height: 64,
                  decoration: BoxDecoration(
                    color: context.paints.accent.withValues(alpha: .16),
                    borderRadius: BorderRadius.circular(14),
                  ),
                  child: Center(
                    child: Glyph(
                      'heart',
                      size: 28,
                      color: context.paints.accent,
                    ),
                  ),
                ),
                const SizedBox(width: 20),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'Улюблене'.tr(context),
                        style: TextStyle(
                          fontSize: 20,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                      const SizedBox(height: 6),
                      Text(
                        trackCount(core.liked.length, context),
                        style: TextStyle(
                          color: context.paints.muted,
                          fontSize: 13,
                        ),
                      ),
                    ],
                  ),
                ),
                const Glyph('chevron-down', size: 18),
              ],
            ),
          ),
        ),
      ),
      const SizedBox(height: 32),
      Heading('Твої плейлісти'.tr(context)),
      const SizedBox(height: 18),
      if (core.playlists.isEmpty)
        EmptyState(
          'Збережи свою першу добірку'.tr(context),
          'Створи плейліст або імпортуй його з музичного сервісу.'.tr(context),
          action: ActionButton(
            'Створити плейліст'.tr(context),
            onTap: onCreate,
          ),
        )
      else
        LayoutBuilder(
          builder: (context, box) => GridView.builder(
            shrinkWrap: true,
            physics: const NeverScrollableScrollPhysics(),
            gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
              crossAxisCount: math.max(2, box.maxWidth ~/ 230),
              crossAxisSpacing: 24,
              mainAxisSpacing: 24,
              childAspectRatio: .82,
            ),
            itemCount: core.playlists.length,
            itemBuilder: (context, i) => PlaylistCard(
              core.playlists[i],
              onTap: () => core.selectPlaylist(core.playlists[i]),
            ),
          ),
        ),
    ],
  );
}
