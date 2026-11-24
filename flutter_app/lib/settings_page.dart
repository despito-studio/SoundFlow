import 'package:flutter/material.dart';

import 'anicry_motion.dart';
import 'core.dart';
import 'l10n.dart';
import 'design.dart';

class SettingsPage extends StatefulWidget {
  const SettingsPage(this.core, {super.key});
  final Core core;
  @override
  State<SettingsPage> createState() => _SettingsPageState();
}

class _SettingsPageState extends State<SettingsPage> {
  String section = 'appearance';
  Core get core => widget.core;

  @override
  Widget build(BuildContext context) => Align(
    alignment: Alignment.topLeft,
    child: ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 900),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Wrap(
            spacing: 4,
            runSpacing: 8,
            children: [
              for (final tab in [
                ('appearance', 'Вигляд'.tr(context)),
                ('playback', 'Відтворення'.tr(context)),
                ('controls', 'Керування'.tr(context)),
                ('library', 'Медіатека'.tr(context)),
                ('updates', 'Оновлення'.tr(context)),
              ])
                Semantics(
                  button: true,
                  selected: section == tab.$1,
                  child: Material(
                    color: section == tab.$1
                        ? context.paints.surface
                        : Colors.transparent,
                    borderRadius: BorderRadius.circular(9),
                    child: AnicryTap(
                      radius: 9,
                      onTap: () {
                        setState(() => section = tab.$1);
                        if (section == 'library') {
                          core.call('list_library_backups');
                        }
                      },
                      child: Padding(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 16,
                          vertical: 10,
                        ),
                        child: Text(
                          tab.$2,
                          style: TextStyle(
                            fontSize: 13,
                            fontWeight: section == tab.$1
                                ? FontWeight.w600
                                : FontWeight.w400,
                            color: section == tab.$1
                                ? context.paints.ink
                                : context.paints.muted,
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
            ],
          ),
          const SizedBox(height: 30),
          switch (section) {
            'playback' => _playback(context),
            'controls' => _controls(context),
            'library' => _library(context),
            'updates' => _updates(context),
            _ => _appearance(context),
          },
          const SizedBox(height: 24),
          Row(
            children: [
              Text(
                'SoundFlow ${core.update['current'] ?? '1.1.2'}',
                style: TextStyle(fontSize: 12, color: context.paints.muted),
              ),
              const Spacer(),
              if (core.update['status'] == 'ready')
                ActionButton(
                  core.update['installed'] == true
                      ? 'Перезапустити для оновлення'.tr(context)
                      : 'Встановити оновлення'.tr(context),
                  compact: true,
                  flat: true,
                  onTap: () => core.action('update-apply'),
                ),
              ActionButton(
                'Про програму'.tr(context),
                compact: true,
                flat: true,
                onTap: () => showAboutDialog(
                  context: context,
                  applicationName: 'SoundFlow',
                  applicationVersion:
                      core.update['current']?.toString() ?? '1.1.2',
                  children: [
                    Text(
                      'Музика, медіатека й мініплеєр для Windows.'.tr(context),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ],
      ),
    ),
  );

  Widget _appearance(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      _heading(context, 'Оформлення'.tr(context)),
      _group(context, [
        _choiceRow(
          context,
          'Мова'.tr(context),
          'Мова інтерфейсу'.tr(context),
          DropdownButton<String>(
            value: core.language,
            items: const [
              DropdownMenuItem(value: 'uk', child: Text('Українська')),
              DropdownMenuItem(value: 'en', child: Text('English')),
            ],
            onChanged: (value) {
              if (value != null) core.setting('language', value);
            },
          ),
        ),
        _choiceRow(
          context,
          'Тема'.tr(context),
          'Колір інтерфейсу'.tr(context),
          _themeChoices(context),
        ),
        _choiceRow(
          context,
          'Іконка'.tr(context),
          'На робочому столі та панелі задач'.tr(context),
          _iconChoices(context),
        ),
      ]),
      const SizedBox(height: 24),
      _heading(context, 'Анімації'.tr(context)),
      _group(context, [
        _toggle(
          context,
          'Менше руху'.tr(context),
          'Спокійні переходи без переміщення елементів.'.tr(context),
          'reduced_motion',
          false,
        ),
      ]),
    ],
  );

  Widget _updates(BuildContext context) {
    final update = core.update;
    final status = update['status']?.toString() ?? 'idle';
    final downloading = status == 'downloading';
    final busy = [
      'checking',
      'downloading',
      'verifying',
      'installing',
      'restarting',
    ].contains(status);
    final total = (update['total'] as num?)?.toDouble() ?? 0;
    final received = (update['received'] as num?)?.toDouble() ?? 0;
    final description = status == 'idle'
        ? 'Перевірити, чи є нова версія SoundFlow.'.tr(context)
        : (update['message']?.toString() ?? '').tr(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _heading(context, 'Оновлення SoundFlow'.tr(context)),
        _group(context, [
          _toggle(
            context,
            'Автоматичні оновлення'.tr(context),
            update['installed'] == true
                ? 'Перевіряти нові версії після запуску та кожні 12 годин.'.tr(
                    context,
                  )
                : 'Автооновлення доступні в готовій версії. Ця збірка призначена для розробки.'
                      .tr(context),
            'auto_update_enabled',
            true,
          ),
          _toggle(
            context,
            'Завантажувати оновлення автоматично'.tr(context),
            'Завантаження у фоні, встановлення під час наступного запуску. Музика не перериватиметься. Вимкни, щоб оновлювати вручну.'
                .tr(context),
            'auto_update_install',
            true,
          ),
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 18),
            child: Row(
              children: [
                Expanded(
                  child: _label(
                    context,
                    update['version'] == null
                        ? 'Версія {version}'
                              .tr(context)
                              .replaceAll(
                                '{version}',
                                '${update['current'] ?? '1.1.1'}',
                              )
                        : 'Доступна версія {version}'
                              .tr(context)
                              .replaceAll('{version}', '${update['version']}'),
                    description,
                  ),
                ),
                const SizedBox(width: 16),
                if (downloading ||
                    status == 'checking' ||
                    status == 'verifying')
                  ActionButton(
                    'Скасувати'.tr(context),
                    compact: true,
                    flat: true,
                    onTap: () => core.action('update-cancel'),
                  )
                else if (!busy)
                  ActionButton(
                    switch (status) {
                      'available' =>
                        update['installed'] == true
                            ? 'Оновити'.tr(context)
                            : 'Завантажити'.tr(context),
                      'ready' =>
                        update['installed'] == true
                            ? 'Оновити'.tr(context)
                            : 'Встановити'.tr(context),
                      _ => 'Перевірити'.tr(context),
                    },
                    compact: true,
                    onTap: () => core.action(switch (status) {
                      'available' =>
                        update['installed'] == true
                            ? 'update-install'
                            : 'update-download',
                      'ready' =>
                        update['installed'] == true
                            ? 'update-install'
                            : 'update-apply',
                      _ => 'update-check',
                    }),
                  )
                else
                  const SizedBox(
                    width: 18,
                    height: 18,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
              ],
            ),
          ),
          if (downloading)
            Padding(
              padding: const EdgeInsets.only(bottom: 20),
              child: LinearProgressIndicator(
                value: total > 0 ? (received / total).clamp(0, 1) : null,
                minHeight: 3,
                borderRadius: BorderRadius.circular(4),
                color: context.paints.accent,
                backgroundColor: context.paints.ink.withValues(alpha: .08),
              ),
            ),
        ]),
      ],
    );
  }

  Widget _playback(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      _heading(context, 'Відтворення'.tr(context)),
      _group(context, [
        _toggle(
          context,
          'Продовжити з того ж місця'.tr(context),
          'Пам’ятати трек під час перемикання профілів. Новий запуск завжди починається з порожнього плеєра.'
              .tr(context),
          'restore_playback_enabled',
          true,
        ),
        _toggle(
          context,
          'Нормалізація гучності'.tr(context),
          'Рівніша гучність між треками.'.tr(context),
          'volume_norm_enabled',
          true,
        ),
        _toggle(
          context,
          'Автовідтворення'.tr(context),
          'Схожі треки після завершення черги.'.tr(context),
          'infinite_autoplay',
          true,
        ),
        _toggle(
          context,
          'Статус у Discord'.tr(context),
          'Показувати музику, яку ти слухаєш.'.tr(context),
          'discord_rpc_enabled',
          true,
        ),
      ]),
    ],
  );

  Widget _controls(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      _heading(context, 'Поведінка'.tr(context)),
      _group(context, [
        _toggle(
          context,
          'Медіаклавіші та Island'.tr(context),
          'Керування музикою з інших програм.'.tr(context),
          'hotkeys_enabled',
          true,
        ),
        _toggle(
          context,
          'Згортати у трей'.tr(context),
          'Продовжувати слухати після закриття вікна.'.tr(context),
          'tray_enabled',
          true,
        ),
      ]),
      const SizedBox(height: 24),
      _heading(context, 'Клавіатурні скорочення'.tr(context)),
      _group(context, [
        for (final shortcut in [
          ('Пошук'.tr(context), 'Ctrl  K'),
          ('Відтворення / пауза'.tr(context), 'Space'),
          ('Dynamic Island', 'Ctrl  Alt  M'),
          ('Island із фокусом'.tr(context), 'Ctrl  Alt  Shift  M'),
        ])
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 16),
            child: Row(
              children: [
                Expanded(
                  child: Text(
                    shortcut.$1,
                    style: const TextStyle(fontSize: 13),
                  ),
                ),
                Text(
                  shortcut.$2,
                  style: TextStyle(fontSize: 12, color: context.paints.muted),
                ),
              ],
            ),
          ),
      ]),
    ],
  );

  Widget _library(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      _heading(context, 'Медіатека'.tr(context)),
      _group(context, [
        _actionRow(
          context,
          'Імпорт із сервісів'.tr(context),
          'Підключені музичні профілі та плейлісти.'.tr(context),
          'Підключення'.tr(context),
          () => core.nav('connections'),
        ),
        _actionRow(
          context,
          'Імпорт медіатеки'.tr(context),
          'Відновити медіатеку з файлу JSON.'.tr(context),
          'Імпортувати'.tr(context),
          () => core.call('import_library_file', [true]),
        ),
        _actionRow(
          context,
          'Експорт у файл'.tr(context),
          'Зберегти медіатеку окремо — наприклад, на іншому диску.'.tr(context),
          'Експортувати'.tr(context),
          () => core.call('export_library_file'),
        ),
      ]),
      const SizedBox(height: 24),
      _heading(context, 'Резервні копії'.tr(context)),
      _group(context, [
        _actionRow(
          context,
          'Зберегти бібліотеку зараз'.tr(context),
          'Автоматичні копії створюються перед змінами, не частіше разу на годину. Зберігаються 10 автоматичних і 10 ручних копій на цьому комп’ютері.'
              .tr(context),
          'Створити копію'.tr(context),
          () => core.call('create_library_backup'),
        ),
        _actionRow(
          context,
          'Збережені копії'.tr(context),
          'Відновлюються плейлісти, улюблене й історія. Профіль та налаштування залишаються поточними.'
              .tr(context),
          'Оновити список'.tr(context),
          () => core.call('list_library_backups'),
        ),
        for (final backup in objects(
          object(core.state['library_backups'])['backups'],
        ))
          _actionRow(
            context,
            _backupTitle(backup),
            '{playlists} плейлістів · {liked} улюблених · {history} в історії'
                .tr(context)
                .replaceAll('{playlists}', '${backup['playlist_count']}')
                .replaceAll('{liked}', '${backup['liked_count']}')
                .replaceAll('{history}', '${backup['history_count']}'),
            'Відновити'.tr(context),
            () => _restoreBackup(backup),
          ),
        if (core.state['library_backups'] != null &&
            objects(object(core.state['library_backups'])['backups']).isEmpty)
          Padding(
            padding: EdgeInsets.symmetric(vertical: 18),
            child: Text(
              'Копій ще немає. Створи першу кнопкою вище.'.tr(context),
            ),
          ),
      ]),
    ],
  );

  String _backupTitle(Json backup) {
    final date = DateTime.fromMillisecondsSinceEpoch(
      (((backup['created_at'] as num?)?.toDouble() ?? 0) * 1000).round(),
    ).toLocal();
    String two(int value) => value.toString().padLeft(2, '0');
    final kind = switch (backup['kind']) {
      'automatic' => 'Автоматична'.tr(context),
      'before_restore' => 'Перед відновленням'.tr(context),
      _ => 'Ручна'.tr(context),
    };
    return '$kind · ${two(date.day)}.${two(date.month)}.${date.year} ${two(date.hour)}:${two(date.minute)}';
  }

  Future<void> _restoreBackup(Json backup) async {
    final accepted = await showAnicryDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: Text('Відновити бібліотеку?'.tr(context)),
        content: Text(
          '{backup}\n\nПлейлісти, улюблене та історію буде замінено. Перед цим SoundFlow збереже поточну бібліотеку в окрему копію.'
              .tr(context)
              .replaceAll('{backup}', _backupTitle(backup)),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(dialogContext, false),
            child: Text('Скасувати'.tr(context)),
          ),
          TextButton(
            onPressed: () => Navigator.pop(dialogContext, true),
            child: Text('Відновити'.tr(context)),
          ),
        ],
      ),
    );
    if (accepted == true && mounted) {
      core.call('restore_library_backup', [backup['id']]);
    }
  }

  Widget _heading(BuildContext context, String title) => Padding(
    padding: const EdgeInsets.only(left: 2, bottom: 12),
    child: Text(
      title,
      style: TextStyle(
        fontFamily: SoundFlowType.heading,
        fontSize: 13,
        fontWeight: FontWeight.w600,
        color: context.paints.muted,
      ),
    ),
  );

  Widget _group(BuildContext context, List<Widget> children) => Container(
    width: double.infinity,
    padding: const EdgeInsets.symmetric(horizontal: 20),
    decoration: BoxDecoration(
      color: context.paints.surface.withValues(alpha: .65),
      borderRadius: BorderRadius.circular(16),
    ),
    child: Column(
      children: [
        for (var i = 0; i < children.length; i++) ...[
          if (i > 0)
            Divider(
              height: 1,
              color: context.paints.ink.withValues(alpha: .06),
            ),
          children[i],
        ],
      ],
    ),
  );

  Widget _label(BuildContext context, String title, String description) =>
      Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(title, style: const TextStyle(fontSize: 14)),
          const SizedBox(height: 5),
          Text(
            description,
            style: TextStyle(
              fontSize: 12,
              height: 1.4,
              color: context.paints.muted,
            ),
          ),
        ],
      );

  Widget _choiceRow(
    BuildContext context,
    String title,
    String description,
    Widget choices,
  ) => Padding(
    padding: const EdgeInsets.symmetric(vertical: 20),
    child: LayoutBuilder(
      builder: (context, box) => box.maxWidth < 720
          ? Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                _label(context, title, description),
                const SizedBox(height: 18),
                choices,
              ],
            )
          : Row(
              children: [
                Expanded(child: _label(context, title, description)),
                const SizedBox(width: 24),
                choices,
              ],
            ),
    ),
  );

  Widget _themeChoices(BuildContext context) => Wrap(
    spacing: 12,
    runSpacing: 12,
    children: [
      for (final scheme in ['dark', 'light', 'rose', 'violet', 'ocean'])
        _theme(context, scheme),
    ],
  );

  Widget _theme(BuildContext context, String scheme) {
    final labels = {
      'dark': 'Темна'.tr(context),
      'light': 'Світла'.tr(context),
      'rose': 'Рожева'.tr(context),
      'violet': 'Фіолетова'.tr(context),
      'ocean': 'Океан'.tr(context),
    };
    final p = Paints.forScheme(scheme), selected = core.scheme == scheme;
    return Semantics(
      button: true,
      selected: selected,
      label: '{theme} тема'.tr(context).replaceAll('{theme}', labels[scheme]!),
      child: AnicryTap(
        radius: 9,
        onTap: () => core.setting('theme', scheme),
        child: SizedBox(
          width: 66,
          child: Column(
            children: [
              Container(
                height: 42,
                padding: const EdgeInsets.all(7),
                decoration: BoxDecoration(
                  color: p.canvas,
                  borderRadius: BorderRadius.circular(9),
                  border: Border.all(
                    color: selected
                        ? context.paints.ink.withValues(alpha: .8)
                        : context.paints.ink.withValues(alpha: .14),
                    width: selected ? 1.5 : 1,
                  ),
                ),
                child: Row(
                  children: [
                    Container(
                      width: 10,
                      decoration: BoxDecoration(
                        color: p.surface,
                        borderRadius: BorderRadius.circular(3),
                      ),
                    ),
                    const SizedBox(width: 5),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          Container(
                            width: 19,
                            height: 3,
                            color: p.ink.withValues(alpha: .7),
                          ),
                          const SizedBox(height: 5),
                          Container(
                            height: 8,
                            decoration: BoxDecoration(
                              color: p.surface,
                              borderRadius: BorderRadius.circular(2),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 8),
              Text(
                labels[scheme]!,
                maxLines: 1,
                style: TextStyle(
                  fontSize: 11,
                  color: selected ? context.paints.ink : context.paints.muted,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _iconChoices(BuildContext context) => Wrap(
    spacing: 16,
    runSpacing: 12,
    children: [
      for (final option in [
        ('default', 'Класика'.tr(context)),
        ('rose', 'Рожева'.tr(context)),
        ('violet', 'Фіолетова'.tr(context)),
        ('ocean', 'Океан'.tr(context)),
      ])
        Semantics(
          button: true,
          selected: (core.settings['desktop_icon'] ?? 'default') == option.$1,
          label: 'Іконка: {icon}'.tr(context).replaceAll('{icon}', option.$2),
          child: AnicryTap(
            radius: 10,
            onTap: () => core.call('set_desktop_icon', [option.$1]),
            child: SizedBox(
              width: 66,
              child: Column(
                children: [
                  Container(
                    width: 44,
                    height: 44,
                    padding: const EdgeInsets.all(5),
                    decoration: BoxDecoration(
                      borderRadius: BorderRadius.circular(11),
                      border: Border.all(
                        color:
                            (core.settings['desktop_icon'] ?? 'default') ==
                                option.$1
                            ? context.paints.ink.withValues(alpha: .7)
                            : Colors.transparent,
                      ),
                    ),
                    child: Image.asset('assets/app-icons/${option.$1}.png'),
                  ),
                  const SizedBox(height: 7),
                  Text(
                    option.$2,
                    style: TextStyle(fontSize: 11, color: context.paints.muted),
                  ),
                ],
              ),
            ),
          ),
        ),
    ],
  );

  Widget _toggle(
    BuildContext context,
    String title,
    String description,
    String key,
    bool fallback,
  ) => Padding(
    padding: const EdgeInsets.symmetric(vertical: 16),
    child: Row(
      children: [
        Expanded(child: _label(context, title, description)),
        const SizedBox(width: 24),
        Semantics(
          label: title,
          child: Switch.adaptive(
            value: (core.settings[key] as bool?) ?? fallback,
            onChanged: (v) => core.setting(key, v),
            activeTrackColor: context.paints.accent,
            activeThumbColor: context.paints.canvas,
          ),
        ),
      ],
    ),
  );

  Widget _actionRow(
    BuildContext context,
    String title,
    String description,
    String action,
    VoidCallback onTap,
  ) => Padding(
    padding: const EdgeInsets.symmetric(vertical: 18),
    child: Row(
      children: [
        Expanded(child: _label(context, title, description)),
        const SizedBox(width: 16),
        ActionButton(action, compact: true, onTap: onTap),
      ],
    ),
  );
}
