import 'dart:math' as math;

import 'package:flutter/material.dart';

import 'core.dart';
import 'design.dart';

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
                height: width * 1.22,
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
        artists.putIfAbsent('${t['source']}:${trackArtist(t)}', () => t);
      }
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            for (final s in [
              ('all', 'Усе'),
              ('youtube', 'YouTube'),
              ('soundcloud', 'SoundCloud'),
            ])
              ChoiceChip(
                label: Text(s.$2),
                selected: source == s.$1,
                onSelected: (_) => setState(() => source = s.$1),
              ),
          ],
        ),
        if (recent.isNotEmpty) ...[
          const SizedBox(height: 24),
          Heading(
            'Продовжити слухати',
            action: ActionButton(
              'Історія',
              flat: true,
              onTap: () => c.nav('history'),
            ),
          ),
          const SizedBox(height: 16),
          LayoutBuilder(
            builder: (context, box) {
              final columns = box.maxWidth > 960 ? 3 : 2;
              return GridView.count(
                crossAxisCount: columns,
                childAspectRatio:
                    (box.maxWidth - (columns - 1) * 12) / columns / 72,
                crossAxisSpacing: 12,
                mainAxisSpacing: 12,
                shrinkWrap: true,
                physics: const NeverScrollableScrollPhysics(),
                children: [
                  for (final t in recent)
                    Material(
                      color: context.paints.surface,
                      borderRadius: BorderRadius.circular(12),
                      child: InkWell(
                        onTap: () => c.play(t, recent),
                        borderRadius: BorderRadius.circular(12),
                        child: Padding(
                          padding: const EdgeInsets.all(10),
                          child: Row(
                            children: [
                              Artwork(trackCover(t), size: 52, radius: 8),
                              const SizedBox(width: 12),
                              Expanded(
                                child: Column(
                                  mainAxisAlignment: MainAxisAlignment.center,
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    Text(
                                      trackTitle(t),
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: const TextStyle(
                                        fontWeight: FontWeight.w600,
                                        fontSize: 14,
                                      ),
                                    ),
                                    const SizedBox(height: 4),
                                    Text(
                                      trackArtist(t),
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: TextStyle(
                                        fontSize: 12,
                                        color: context.paints.muted,
                                      ),
                                    ),
                                  ],
                                ),
                              ),
                              const SizedBox(width: 8),
                              const Glyph('play', size: 18),
                            ],
                          ),
                        ),
                      ),
                    ),
                ],
              );
            },
          ),
        ],
        shelf(
          feed['personalized'] == true
              ? 'Більше того, що подобається'
              : 'Відкрити для себе',
          recommendations,
          subtitle: feed['personalized'] == true
              ? 'За твоїми останніми прослуховуваннями'
              : 'Музика з YouTube та SoundCloud',
        ),
        shelf('З YouTube', youtube),
        shelf('Із SoundCloud', soundcloud),
        if (feed['loaded'] != true)
          const Padding(
            padding: EdgeInsets.symmetric(vertical: 32),
            child: Row(
              children: [
                SizedBox(
                  width: 16,
                  height: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
                SizedBox(width: 12),
                Text('Знаходимо музику для тебе…'),
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
              'Добірки поки недоступні',
              'Перевір підключення або знайди музику через пошук.',
              action: ActionButton(
                'Оновити добірки',
                onTap: () => c.call('get_home_feed', [true]),
              ),
            ),
          ),
        if (artists.isNotEmpty) ...[
          const SizedBox(height: 32),
          const Heading('Автори'),
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
            'З твоєї медіатеки',
            action: ActionButton(
              'Усе',
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
            'Новий плейліст',
            icon: 'plus',
            primary: true,
            onTap: onCreate,
          ),
          ActionButton(
            'Імпорт із сервісів',
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
                      const Text(
                        'Улюблене',
                        style: TextStyle(
                          fontSize: 20,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                      const SizedBox(height: 6),
                      Text(
                        trackCount(core.liked.length),
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
      const Heading('Твої плейлісти'),
      const SizedBox(height: 18),
      if (core.playlists.isEmpty)
        EmptyState(
          'Збережи свою першу добірку',
          'Створи плейліст або імпортуй його з музичного сервісу.',
          action: ActionButton('Створити плейліст', onTap: onCreate),
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
