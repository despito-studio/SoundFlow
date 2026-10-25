import 'dart:math' as math;

import 'package:flutter/material.dart';

import 'core.dart';
import 'design.dart';
import 'anicry_motion.dart';

Future<void> editProfile(BuildContext context, Core core) async {
  final result = await showAnicryDialog<Json>(
    context: context,
    builder: (_) => ProfileEditor(core),
  );
  if (result != null) core.call('update_profile', [result]);
}

class ProfileEditor extends StatefulWidget {
  const ProfileEditor(this.core, {super.key});
  final Core core;
  @override
  State<ProfileEditor> createState() => _ProfileEditorState();
}

class _ProfileEditorState extends State<ProfileEditor> {
  late final name = TextEditingController(
    text: widget.core.profile['name']?.toString() ?? '',
  );
  late final bio = TextEditingController(
    text: widget.core.profile['bio']?.toString() ?? '',
  );
  late String avatar = widget.core.profile['avatar_hidden'] == true
      ? ''
      : firstText([
          widget.core.profile['avatar_url'],
          object(object(widget.core.state['cloud'])['user'])['avatar_url'],
        ]);
  late String banner = widget.core.profile['banner_url']?.toString() ?? '';
  bool choosing = false;
  String error = '';
  @override
  void dispose() {
    name.dispose();
    bio.dispose();
    super.dispose();
  }

  Future<void> choose(bool cover) async {
    setState(() => choosing = true);
    final path = await widget.core.pickProfileImage();
    if (!mounted) return;
    setState(() {
      choosing = false;
      if (path != null) {
        if (cover) {
          banner = path;
        } else {
          avatar = path;
        }
      }
    });
  }

  Future<void> link(bool cover) async {
    final field = TextEditingController(text: cover ? banner : avatar);
    final value = await showAnicryDialog<String>(
      context: context,
      builder: (ctx) => Dialog(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 420),
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Heading(
                  cover ? 'Обкладинка за посиланням' : 'Фото за посиланням',
                ),
                const SizedBox(height: 20),
                TextField(
                  controller: field,
                  autofocus: true,
                  decoration: InputDecoration(
                    fillColor: ctx.paints.canvas,
                    hintText: 'https://…',
                  ),
                ),
                const SizedBox(height: 20),
                Align(
                  alignment: Alignment.centerRight,
                  child: ActionButton(
                    'Застосувати',
                    primary: true,
                    onTap: () => Navigator.pop(ctx, field.text.trim()),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
    if (mounted && value != null) {
      final uri = Uri.tryParse(value);
      setState(() {
        if (value.isEmpty || (uri?.scheme == 'https' && uri!.host.isNotEmpty)) {
          error = '';
          if (cover) {
            banner = value;
          } else {
            avatar = value;
          }
        } else {
          error = 'Потрібне посилання HTTPS на зображення.';
        }
      });
    }
    await Future<void>.delayed(const Duration(milliseconds: 320));
    field.dispose();
  }

  Future<void> imageOptions(bool cover) async {
    final hasImage = (cover ? banner : avatar).isNotEmpty;
    final action = await showAnicryDialog<String>(
      context: context,
      builder: (ctx) => Dialog(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 300),
          child: Padding(
            padding: const EdgeInsets.all(12),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                AnicryTap(
                  onTap: () => Navigator.pop(ctx, 'link'),
                  icon: 'link',
                  child: const Padding(
                    padding: EdgeInsets.symmetric(horizontal: 12, vertical: 14),
                    child: Row(
                      children: [
                        Glyph('link', size: 18),
                        SizedBox(width: 12),
                        Text('Вставити посилання'),
                      ],
                    ),
                  ),
                ),
                if (hasImage)
                  AnicryTap(
                    onTap: () => Navigator.pop(ctx, 'remove'),
                    icon: 'trash-2',
                    child: Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 12,
                        vertical: 14,
                      ),
                      child: Row(
                        children: [
                          const Glyph('trash-2', size: 18),
                          const SizedBox(width: 12),
                          Text(cover ? 'Прибрати обкладинку' : 'Прибрати фото'),
                        ],
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
    if (!mounted) return;
    if (action == 'link') {
      await link(cover);
    } else if (action == 'remove') {
      setState(() {
        if (cover) {
          banner = '';
        } else {
          avatar = '';
        }
      });
    }
  }

  void save() {
    if (name.text.trim().isEmpty) {
      setState(() => error = 'Вкажи ім’я профілю.');
      return;
    }
    Navigator.pop(context, {
      'name': name.text.trim(),
      'bio': bio.text.trim(),
      'avatar_url': avatar,
      'avatar_hidden': avatar.isEmpty,
      'banner_url': banner,
    });
  }

  @override
  Widget build(BuildContext context) {
    final p = context.paints;
    return Dialog(
      child: ConstrainedBox(
        constraints: BoxConstraints(
          maxWidth: 540,
          maxHeight: MediaQuery.sizeOf(context).height - 48,
        ),
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  const Expanded(child: Heading('Редагувати профіль')),
                  Control('x', 'Закрити', onTap: () => Navigator.pop(context)),
                ],
              ),
              const SizedBox(height: 18),
              SizedBox(
                height: 148,
                child: Stack(
                  clipBehavior: Clip.none,
                  children: [
                    SizedBox(
                      height: 108,
                      width: double.infinity,
                      child: ClipRRect(
                        borderRadius: BorderRadius.circular(16),
                        child: banner.isEmpty
                            ? DecoratedBox(
                                decoration: BoxDecoration(
                                  color: p.canvas,
                                  border: Border.all(
                                    color: p.ink.withValues(alpha: .06),
                                  ),
                                  borderRadius: BorderRadius.circular(16),
                                ),
                                child: Padding(
                                  padding: const EdgeInsets.all(16),
                                  child: Align(
                                    alignment: Alignment.bottomRight,
                                    child: Text(
                                      'Обкладинка профілю',
                                      style: TextStyle(
                                        fontSize: 12,
                                        color: p.muted.withValues(alpha: .6),
                                      ),
                                    ),
                                  ),
                                ),
                              )
                            : Artwork(banner, radius: 0),
                      ),
                    ),
                    Positioned(
                      right: 10,
                      top: 10,
                      child: Row(
                        children: [
                          ActionButton(
                            banner.isEmpty
                                ? 'Додати обкладинку'
                                : 'Змінити обкладинку',
                            compact: true,
                            icon: 'image',
                            onTap: choosing ? null : () => choose(true),
                          ),
                          const SizedBox(width: 6),
                          Control(
                            'more',
                            'Дії з обкладинкою',
                            size: 32,
                            glyphSize: 18,
                            surface: true,
                            onTap: choosing ? null : () => imageOptions(true),
                          ),
                        ],
                      ),
                    ),
                    Positioned(
                      left: 16,
                      bottom: 0,
                      child: Container(
                        padding: const EdgeInsets.all(4),
                        decoration: BoxDecoration(
                          color: p.surface,
                          shape: BoxShape.circle,
                        ),
                        child: avatar.isNotEmpty
                            ? Artwork(avatar, size: 76, radius: 38)
                            : Container(
                                width: 76,
                                height: 76,
                                decoration: BoxDecoration(
                                  color: p.canvas,
                                  shape: BoxShape.circle,
                                ),
                                child: Center(
                                  child: Text(
                                    name.text.trim().isEmpty
                                        ? ''
                                        : name.text
                                              .trim()
                                              .substring(0, 1)
                                              .toUpperCase(),
                                    style: TextStyle(
                                      fontSize: 28,
                                      fontWeight: FontWeight.w500,
                                      color: p.muted,
                                    ),
                                  ),
                                ),
                              ),
                      ),
                    ),
                    Positioned(
                      left: 116,
                      bottom: 4,
                      child: Row(
                        children: [
                          ActionButton(
                            'Змінити фото',
                            compact: true,
                            icon: 'camera',
                            onTap: choosing ? null : () => choose(false),
                          ),
                          const SizedBox(width: 6),
                          Control(
                            'more',
                            'Дії з фото',
                            size: 32,
                            glyphSize: 18,
                            surface: true,
                            onTap: choosing ? null : () => imageOptions(false),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 20),
              Text('Ім’я', style: TextStyle(fontSize: 12, color: p.muted)),
              const SizedBox(height: 7),
              TextField(
                controller: name,
                maxLength: 80,
                textCapitalization: TextCapitalization.words,
                decoration: InputDecoration(
                  fillColor: p.canvas,
                  counterText: '',
                  hintText: 'Як до тебе звертатися',
                ),
              ),
              const SizedBox(height: 16),
              Text('Про тебе', style: TextStyle(fontSize: 12, color: p.muted)),
              const SizedBox(height: 7),
              TextField(
                controller: bio,
                maxLength: 1000,
                minLines: 3,
                maxLines: 4,
                decoration: InputDecoration(
                  fillColor: p.canvas,
                  counterText: '',
                  hintText: 'Кілька слів про себе й музику',
                ),
              ),
              if (error.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.only(top: 12),
                  child: Text(
                    error,
                    style: const TextStyle(
                      fontSize: 13,
                      color: Color(0xffedb0a7),
                    ),
                  ),
                ),
              const SizedBox(height: 22),
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  ActionButton(
                    'Скасувати',
                    flat: true,
                    onTap: () => Navigator.pop(context),
                  ),
                  const SizedBox(width: 10),
                  ActionButton(
                    'Зберегти',
                    primary: true,
                    onTap: choosing ? null : save,
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class ProfileHeader extends StatelessWidget {
  const ProfileHeader({
    super.key,
    required this.name,
    required this.avatar,
    required this.label,
    this.banner = '',
    this.details = '',
    required this.actions,
  });
  final String name, avatar, label, banner, details;
  final List<Widget> actions;
  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      if (banner.isNotEmpty) ...[
        SizedBox(
          height: 180,
          width: double.infinity,
          child: Artwork(banner, radius: 20),
        ),
        const SizedBox(height: 24),
      ],
      Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          if (avatar.isNotEmpty)
            Artwork(avatar, size: 132, radius: 66)
          else
            Container(
              width: 132,
              height: 132,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: context.paints.surface,
              ),
              child: Center(
                child: Text(
                  initials(name),
                  style: TextStyle(
                    fontSize: 40,
                    fontWeight: FontWeight.w600,
                    color: context.paints.ink,
                  ),
                ),
              ),
            ),
          const SizedBox(width: 28),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  label,
                  style: TextStyle(fontSize: 13, color: context.paints.muted),
                ),
                const SizedBox(height: 8),
                Text(
                  name,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(
                    fontSize: 36,
                    fontFamily: SoundFlowType.heading,
                    height: 1.12,
                    fontWeight: FontWeight.w700,
                  ),
                ),
                if (details.isNotEmpty) ...[
                  const SizedBox(height: 12),
                  Text(
                    details,
                    style: TextStyle(fontSize: 13, color: context.paints.muted),
                  ),
                ],
                const SizedBox(height: 20),
                Wrap(spacing: 12, runSpacing: 8, children: actions),
              ],
            ),
          ),
        ],
      ),
    ],
  );
}

String firstText(Iterable<dynamic> values, [String fallback = '']) => values
    .map((v) => v?.toString().trim() ?? '')
    .firstWhere((v) => v.isNotEmpty, orElse: () => fallback);

class UserProfile extends StatefulWidget {
  const UserProfile(this.core, {super.key, required this.trackRows});
  final Core core;
  final Widget Function(List<Json>) trackRows;
  @override
  State<UserProfile> createState() => _UserProfileState();
}

class _UserProfileState extends State<UserProfile> {
  String tab = 'playlists';
  Core get core => widget.core;
  @override
  Widget build(BuildContext context) {
    final p = context.paints, profile = core.profile;
    final user = object(object(core.state['cloud'])['user']);
    final name = firstText([profile['name'], user['name']], 'Твій профіль');
    final avatar = profile['avatar_hidden'] == true
        ? ''
        : firstText([
            profile['avatar_url'],
            user['avatar_url'],
            user['picture'],
          ]);
    final banner = firstText([profile['banner_url']]);
    final bio = firstText([profile['bio']]);
    final seen = <String>{};
    final saved = [
      ...core.liked,
      ...core.playlists.expand((p) => objects(p['tracks'])),
    ].where((t) => seen.add(trackKey(t))).toList();
    final authorKeys = <String>{};
    final authors =
        saved
            .where((t) => authorKeys.add('${t['source']}:${trackArtist(t)}'))
            .toList()
          ..sort(
            (a, b) =>
                trackArtist(a)
                    .toLowerCase()
                    .compareTo(trackArtist(b).toLowerCase()),
          );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        ConstrainedBox(
          constraints: BoxConstraints(minHeight: bio.isEmpty ? 234 : 266),
          child: Stack(
            children: [
              if (banner.isNotEmpty)
                Positioned.fill(
                  child: ShaderMask(
                    blendMode: BlendMode.dstIn,
                    shaderCallback: (rect) => const LinearGradient(
                      begin: Alignment.topCenter,
                      end: Alignment.bottomCenter,
                      colors: [Colors.white, Colors.white, Colors.transparent],
                      stops: [0, .25, 1],
                    ).createShader(rect),
                    child: SizedBox.expand(child: Artwork(banner, radius: 0)),
                  ),
                ),
              Padding(
                padding: const EdgeInsets.fromLTRB(12, 22, 12, 22),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.center,
                  children: [
                    AnicryTap(
                      onTap: () => editProfile(context, core),
                      circle: true,
                      child: avatar.isNotEmpty
                          ? Artwork(avatar, size: 144, radius: 72)
                          : Container(
                              width: 144,
                              height: 144,
                              decoration: BoxDecoration(
                                color: p.hover,
                                shape: BoxShape.circle,
                              ),
                              child: Center(
                                child: Text(
                                  initials(name),
                                  style: const TextStyle(
                                    fontSize: 44,
                                    fontWeight: FontWeight.w600,
                                  ),
                                ),
                              ),
                            ),
                    ),
                    const SizedBox(width: 28),
                    Expanded(
                      child: Column(
                        mainAxisAlignment: MainAxisAlignment.center,
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            'Профіль слухача',
                            style: TextStyle(fontSize: 12, color: p.muted),
                          ),
                          const SizedBox(height: 8),
                          Text(
                            name,
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(
                              fontSize: 44,
                              fontFamily: SoundFlowType.heading,
                              height: 1.1,
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                          const SizedBox(height: 12),
                          Text(
                            '${core.playlists.length} плейлістів · ${trackCount(saved.length)} у медіатеці',
                            style: TextStyle(fontSize: 13, color: p.muted),
                          ),
                          if (bio.isNotEmpty) ...[
                            const SizedBox(height: 12),
                            Text(
                              bio,
                              maxLines: 2,
                              overflow: TextOverflow.ellipsis,
                              style: TextStyle(
                                fontSize: 14,
                                height: 1.5,
                                color: p.ink,
                              ),
                            ),
                          ],
                          const SizedBox(height: 18),
                          Wrap(
                            spacing: 10,
                            runSpacing: 8,
                            children: [
                              ActionButton(
                                'Редагувати профіль',
                                icon: 'pencil',
                                onTap: () => editProfile(context, core),
                              ),
                              ActionButton(
                                'Підключення',
                                icon: 'link',
                                flat: true,
                                onTap: () => core.nav('connections'),
                              ),
                            ],
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        Row(
          children: [
            for (final entry in [
              ('playlists', 'Плейлісти'),
              ('artists', 'Автори'),
              ('recent', 'Нещодавні'),
            ])
              Padding(
                padding: const EdgeInsets.only(right: 8),
                child: Material(
                  color: tab == entry.$1 ? p.surface : Colors.transparent,
                  borderRadius: BorderRadius.circular(10),
                  child: AnicryTap(
                    radius: 10,
                    onTap: () => setState(() => tab = entry.$1),
                    child: Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 16,
                        vertical: 11,
                      ),
                      child: Text(
                        entry.$2,
                        style: TextStyle(
                          fontSize: 14,
                          fontWeight: FontWeight.w600,
                          color: tab == entry.$1 ? p.ink : p.muted,
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            const Spacer(),
            ActionButton(
              'Медіатека',
              icon: 'arrow-right',
              flat: true,
              onTap: () => core.nav('library'),
            ),
          ],
        ),
        const SizedBox(height: 22),
        AnimatedSwitcher(
          duration: core.reduced ? Duration.zero : Anicry.reveal,
          switchInCurve: Anicry.easeOut,
          child: KeyedSubtree(
            key: ValueKey(tab),
            child: switch (tab) {
              'artists' =>
                authors.isEmpty
                    ? const EmptyState(
                        'Поки немає авторів',
                        'Збережи музику в медіатеку.',
                      )
                    : LayoutBuilder(
                        builder: (context, box) => GridView.builder(
                          shrinkWrap: true,
                          physics: const NeverScrollableScrollPhysics(),
                          itemCount: authors.length,
                          gridDelegate:
                              SliverGridDelegateWithFixedCrossAxisCount(
                                crossAxisCount: math.max(
                                  3,
                                  box.maxWidth ~/ 170,
                                ),
                                mainAxisExtent: 214,
                                crossAxisSpacing: 12,
                                mainAxisSpacing: 18,
                              ),
                          itemBuilder: (context, i) =>
                              ArtistCard(authors[i], core: core),
                        ),
                      ),
              'recent' =>
                core.recent.isEmpty
                    ? const EmptyState(
                        'Історія ще порожня',
                        'Тут з’явиться музика, яку ти слухаєш.',
                      )
                    : widget.trackRows(core.recent.take(30).toList()),
              _ =>
                core.playlists.isEmpty
                    ? const EmptyState(
                        'Твої плейлісти',
                        'Створи плейліст або збережи його з музичного сервісу.',
                      )
                    : LayoutBuilder(
                        builder: (context, box) => GridView.builder(
                          shrinkWrap: true,
                          physics: const NeverScrollableScrollPhysics(),
                          itemCount: core.playlists.length,
                          gridDelegate:
                              SliverGridDelegateWithFixedCrossAxisCount(
                                crossAxisCount: math.max(
                                  2,
                                  box.maxWidth ~/ 230,
                                ),
                                childAspectRatio: .82,
                                crossAxisSpacing: 12,
                                mainAxisSpacing: 18,
                              ),
                          itemBuilder: (context, i) => PlaylistCard(
                            core.playlists[i],
                            onTap: () => core.selectPlaylist(core.playlists[i]),
                          ),
                        ),
                      ),
            },
          ),
        ),
      ],
    );
  }
}

class CreatorProfile extends StatelessWidget {
  const CreatorProfile(this.core, {super.key, required this.trackRows});
  final Core core;
  final Widget Function(List<Json>) trackRows;
  @override
  Widget build(BuildContext context) {
    final profile = core.creator, seed = core.creatorTrack;
    final source =
        profile['source']?.toString() ?? seed['source']?.toString() ?? '';
    final tracks = objects(profile['tracks']).where(onlineTrack).toList();
    final description = profile['description']?.toString() ?? '';
    final url =
        profile['url']?.toString() ?? seed['artist_url']?.toString() ?? '';
    final fallback = core.allTracks
        .where(
          (t) => t['source'] == source && trackArtist(t) == trackArtist(seed),
        )
        .toList();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        ProfileHeader(
          name: firstText([
            profile['name'],
            core.artistData(seed)['name'],
          ], trackArtist(seed)),
          avatar: firstText([
            profile['avatar'],
            seed['artist_avatar'],
            core.artistData(seed)['avatar'],
          ]),
          banner: profile['banner']?.toString() ?? '',
          label: sourceName({'source': source}),
          details: [
            if (profile['followers'] is num)
              '${profile['followers']} підписників',
            if (profile['location']?.toString().isNotEmpty == true)
              profile['location'].toString(),
          ].join(' · '),
          actions: [
            if (tracks.isNotEmpty)
              ActionButton(
                'Слухати',
                icon: 'play',
                primary: true,
                onTap: () => core.play(tracks.first, tracks),
              ),
            if (url.isNotEmpty)
              ActionButton(
                'Відкрити в ${sourceName({'source': source})}',
                onTap: () => core.call('open_external_url', [url]),
              ),
          ],
        ),
        if (profile.isEmpty)
          Padding(
            padding: const EdgeInsets.only(top: 24),
            child: core.pending > 0
                ? const LinearProgressIndicator(minHeight: 2)
                : Row(
                    children: [
                      Expanded(
                        child: Text(
                          'Не вдалося завантажити дані автора.',
                          style: TextStyle(color: context.paints.muted),
                        ),
                      ),
                      ActionButton(
                        'Повторити',
                        onTap: () => core.call('get_creator_profile', [seed]),
                      ),
                    ],
                  ),
          ),
        const SizedBox(height: 36),
        Heading(tracks.isEmpty ? 'У твоїй медіатеці' : 'Треки автора'),
        const SizedBox(height: 16),
        if (tracks.isNotEmpty || fallback.isNotEmpty)
          trackRows(tracks.isEmpty ? fallback : tracks)
        else if (profile.isNotEmpty)
          Text(
            'Джерело не надало доступних треків.',
            style: TextStyle(color: context.paints.muted),
          ),
        if (description.isNotEmpty) ...[
          const SizedBox(height: 36),
          const Heading('Про автора'),
          const SizedBox(height: 16),
          ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 800),
            child: SelectableText(
              description,
              style: TextStyle(
                fontSize: 15,
                height: 1.7,
                color: context.paints.muted,
              ),
            ),
          ),
        ],
      ],
    );
  }
}
