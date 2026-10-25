import 'package:flutter/material.dart';

import 'anicry_motion.dart';
import 'core.dart';
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
                ('appearance', 'Вигляд'),
                ('playback', 'Відтворення'),
                ('controls', 'Керування'),
                ('library', 'Медіатека'),
                ('updates', 'Оновлення'),
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
                      onTap: () => setState(() => section = tab.$1),
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
                'SoundFlow ${core.update['current'] ?? '1.1.0'}',
                style: TextStyle(fontSize: 12, color: context.paints.muted),
              ),
              const Spacer(),
              if (core.update['status'] == 'ready')
                ActionButton(
                  core.update['installed'] == true
                      ? 'Перезапустити для оновлення'
                      : 'Встановити оновлення',
                  compact: true,
                  flat: true,
                  onTap: () => core.action('update-apply'),
                ),
              ActionButton(
                'Про програму',
                compact: true,
                flat: true,
                onTap: () => showAboutDialog(
                  context: context,
                  applicationName: 'SoundFlow',
                  applicationVersion:
                      core.update['current']?.toString() ?? '1.1.0',
                  children: [
                    const Text('Музика, медіатека й мініплеєр для Windows.'),
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
      _heading(context, 'Оформлення'),
      _group(context, [
        _choiceRow(context, 'Тема', 'Колір інтерфейсу', _themeChoices(context)),
        _choiceRow(
          context,
          'Іконка',
          'На робочому столі та панелі задач',
          _iconChoices(context),
        ),
      ]),
      const SizedBox(height: 24),
      _heading(context, 'Анімації'),
      _group(context, [
        _toggle(
          context,
          'Менше руху',
          'Спокійні переходи без переміщення елементів.',
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
        ? 'Перевірити, чи є нова версія SoundFlow.'
        : update['message']?.toString() ?? '';
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _heading(context, 'Оновлення SoundFlow'),
        _group(context, [
          _toggle(
            context,
            'Автоматичні оновлення',
            update['installed'] == true
                ? 'Показувати нову версію після запуску. «Оновити» завантажить і встановить її.'
                : 'Автооновлення доступні в готовій версії. Ця збірка призначена для розробки.',
            'auto_update_enabled',
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
                        ? 'Версія ${update['current'] ?? '1.1.0'}'
                        : 'Доступна версія ${update['version']}',
                    description,
                  ),
                ),
                const SizedBox(width: 16),
                if (downloading ||
                    status == 'checking' ||
                    status == 'verifying')
                  ActionButton(
                    'Скасувати',
                    compact: true,
                    flat: true,
                    onTap: () => core.action('update-cancel'),
                  )
                else if (!busy)
                  ActionButton(
                    switch (status) {
                      'available' =>
                        update['installed'] == true ? 'Оновити' : 'Завантажити',
                      'ready' =>
                        update['installed'] == true ? 'Оновити' : 'Встановити',
                      _ => 'Перевірити',
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
      _heading(context, 'Відтворення'),
      _group(context, [
        _toggle(
          context,
          'Нормалізація гучності',
          'Рівніша гучність між треками.',
          'volume_norm_enabled',
          true,
        ),
        _toggle(
          context,
          'Автовідтворення',
          'Схожі треки після завершення черги.',
          'infinite_autoplay',
          true,
        ),
        _toggle(
          context,
          'Статус у Discord',
          'Показувати музику, яку ти слухаєш.',
          'discord_rpc_enabled',
          true,
        ),
      ]),
    ],
  );

  Widget _controls(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      _heading(context, 'Поведінка'),
      _group(context, [
        _toggle(
          context,
          'Медіаклавіші та Island',
          'Керування музикою з інших програм.',
          'hotkeys_enabled',
          true,
        ),
        _toggle(
          context,
          'Згортати у трей',
          'Продовжувати слухати після закриття вікна.',
          'tray_enabled',
          true,
        ),
      ]),
      const SizedBox(height: 24),
      _heading(context, 'Клавіатурні скорочення'),
      _group(context, [
        for (final shortcut in [
          ('Пошук', 'Ctrl  K'),
          ('Відтворення / пауза', 'Space'),
          ('Dynamic Island', 'Ctrl  Alt  M'),
          ('Island із фокусом', 'Ctrl  Alt  Shift  M'),
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
      _heading(context, 'Медіатека'),
      _group(context, [
        _actionRow(
          context,
          'Імпорт із сервісів',
          'Підключені музичні профілі та плейлісти.',
          'Підключення',
          () => core.nav('connections'),
        ),
        _actionRow(
          context,
          'Імпорт медіатеки',
          'Відновити медіатеку з файлу JSON.',
          'Імпортувати',
          () => core.call('import_library_file', [true]),
        ),
        _actionRow(
          context,
          'Резервна копія',
          'Зберегти плейлісти, улюблене та налаштування.',
          'Експортувати',
          () => core.call('export_library_file'),
        ),
      ]),
    ],
  );

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
    const labels = {
      'dark': 'Темна',
      'light': 'Світла',
      'rose': 'Рожева',
      'violet': 'Фіолетова',
      'ocean': 'Океан',
    };
    final p = Paints.forScheme(scheme), selected = core.scheme == scheme;
    return Semantics(
      button: true,
      selected: selected,
      label: '${labels[scheme]} тема',
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
        ('default', 'Класика'),
        ('rose', 'Рожева'),
        ('violet', 'Фіолетова'),
        ('ocean', 'Океан'),
      ])
        Semantics(
          button: true,
          selected: (core.settings['desktop_icon'] ?? 'default') == option.$1,
          label: 'Іконка: ${option.$2}',
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
