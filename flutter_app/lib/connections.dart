import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'anicry_motion.dart';
import 'core.dart';
import 'design.dart';

class ConnectionsPopup extends StatefulWidget {
  const ConnectionsPopup(this.core, {super.key});
  final Core core;
  @override
  State<ConnectionsPopup> createState() => _ConnectionsPopupState();
}

class _ConnectionsPopupState extends State<ConnectionsPopup> {
  final fields = <String, TextEditingController>{};
  final edited = <String>{};
  final errors = <String, String>{};
  String source = 'soundcloud';
  bool importing = false;
  DateTime? started;
  Core get c => widget.core;
  @override
  void initState() {
    super.initState();
    for (final name in ['soundcloud', 'youtube', 'spotify']) {
      fields[name] = TextEditingController();
    }
    c.addListener(sync);
    sync();
  }

  void sync() {
    final saved = object(c.profile['connected_accounts']);
    for (final entry in fields.entries) {
      if (!edited.contains(entry.key)) {
        final value = saved[entry.key];
        final text = value is String
            ? value
            : object(value)['identifier']?.toString() ?? '';
        if (entry.value.text != text) entry.value.text = text;
      }
    }
    if (importing &&
        c.pending == 0 &&
        started != null &&
        DateTime.now().difference(started!).inMilliseconds > 500) {
      setState(() => importing = false);
    }
  }

  @override
  void dispose() {
    c.removeListener(sync);
    for (final field in fields.values) {
      field.dispose();
    }
    super.dispose();
  }

  bool valid() {
    final uri = Uri.tryParse(fields[source]!.text.trim());
    final hosts = {
      'soundcloud': [
        'soundcloud.com',
        'www.soundcloud.com',
        'm.soundcloud.com',
      ],
      'youtube': [
        'youtube.com',
        'www.youtube.com',
        'music.youtube.com',
        'youtu.be',
      ],
      'spotify': ['open.spotify.com', 'www.open.spotify.com'],
    };
    final ok = uri?.scheme == 'https' && hosts[source]!.contains(uri?.host);
    setState(() {
      if (ok) {
        errors.remove(source);
      } else {
        errors[source] =
            'Вкажи посилання ${sourceName({'source': source})} через https://';
      }
    });
    return ok;
  }

  Future<void> paste() async {
    final data = await Clipboard.getData(Clipboard.kTextPlain);
    if (mounted && data?.text != null) {
      edited.add(source);
      fields[source]!.text = data!.text!.trim();
    }
  }

  void save({bool import = false}) {
    if (!valid()) return;
    c.call('set_connected_account', [source, fields[source]!.text.trim()]);
    if (import) {
      setState(() {
        importing = true;
        started = DateTime.now();
      });
      c.call('import_external_profile', [source, fields[source]!.text.trim()]);
      Timer(const Duration(milliseconds: 600), () {
        if (mounted && c.pending == 0) setState(() => importing = false);
      });
    }
  }

  @override
  Widget build(BuildContext context) => Dialog(
    child: ConstrainedBox(
      constraints: BoxConstraints(
        maxWidth: 560,
        maxHeight: MediaQuery.sizeOf(context).height - 64,
      ),
      child: ListenableBuilder(
        listenable: c,
        builder: (context, _) {
          final p = context.paints, cloud = object(c.state['cloud']);
          final saved = object(c.profile['connected_accounts']);
          final linked = object(object(c.profileAssets['connections'])[source]);
          final error = c.error.isNotEmpty
              ? c.error
              : c.state['message_error'] == true
              ? c.state['message']?.toString() ?? ''
              : '';
          return SingleChildScrollView(
            padding: const EdgeInsets.all(24),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    const Expanded(
                      child: Text(
                        'Підключення',
                        style: TextStyle(
                          fontSize: 24,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ),
                    Control(
                      'x',
                      'Закрити',
                      onTap: () => Navigator.pop(context),
                    ),
                  ],
                ),
                const SizedBox(height: 20),
                Row(
                  children: [
                    const Glyph('user-round', size: 24),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            c.profile['name']?.toString() ?? 'SoundFlow',
                            style: const TextStyle(fontWeight: FontWeight.w600),
                          ),
                          const SizedBox(height: 4),
                          Text(
                            cloud['signed_in'] == true
                                ? 'Акаунт SoundFlow підключено'
                                : 'Синхронізація медіатеки',
                            style: TextStyle(fontSize: 12, color: p.muted),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(width: 12),
                    ActionButton(
                      cloud['signed_in'] == true
                          ? 'Вийти'
                          : 'Увійти через Google',
                      flat: true,
                      onTap: () => c.call(
                        cloud['signed_in'] == true ? 'sign_out' : 'start_oauth',
                        cloud['signed_in'] == true ? [] : ['google'],
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 24),
                Divider(color: p.ink.withValues(alpha: .08), height: 1),
                const SizedBox(height: 20),
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: [
                    for (final name in fields.keys)
                      Material(
                        color: name == source ? p.hover : Colors.transparent,
                        borderRadius: BorderRadius.circular(9),
                        child: AnicryTap(
                          radius: 9,
                          onTap: importing
                              ? null
                              : () => setState(() => source = name),
                          child: Padding(
                            padding: const EdgeInsets.symmetric(
                              horizontal: 14,
                              vertical: 10,
                            ),
                            child: Text(
                              sourceName({'source': name}),
                              style: TextStyle(
                                fontSize: 14,
                                color: name == source ? p.ink : p.muted,
                                fontWeight: name == source
                                    ? FontWeight.w600
                                    : FontWeight.w400,
                              ),
                            ),
                          ),
                        ),
                      ),
                  ],
                ),
                const SizedBox(height: 20),
                Row(
                  children: [
                    if (linked['avatar']?.toString().isNotEmpty == true) ...[
                      Artwork(
                        linked['avatar'].toString(),
                        size: 30,
                        radius: 15,
                      ),
                      const SizedBox(width: 10),
                    ],
                    Expanded(
                      child: Text(
                        linked['name']?.toString() ?? 'Профіль або плейліст',
                        style: TextStyle(fontSize: 13, color: p.muted),
                      ),
                    ),
                    if (saved[source] != null) ...[
                      Glyph('check', size: 15, color: p.accent),
                      const SizedBox(width: 6),
                      Text(
                        'Посилання збережено',
                        style: TextStyle(fontSize: 12, color: p.muted),
                      ),
                    ],
                  ],
                ),
                const SizedBox(height: 10),
                TextField(
                  key: ValueKey(source),
                  controller: fields[source],
                  enabled: !importing,
                  onChanged: (_) => edited.add(source),
                  decoration: InputDecoration(
                    fillColor: p.canvas,
                    hintText: 'https://…',
                    errorText: errors[source],
                    suffixIcon: Padding(
                      padding: const EdgeInsets.all(4),
                      child: Control(
                        'copy',
                        'Вставити посилання',
                        size: 32,
                        onTap: importing ? null : paste,
                      ),
                    ),
                  ),
                ),
                const SizedBox(height: 20),
                Row(
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: [
                    ActionButton(
                      'Зберегти',
                      flat: true,
                      icon: 'check',
                      onTap: importing ? null : save,
                    ),
                    const SizedBox(width: 10),
                    ActionButton(
                      importing ? 'Імпортуємо…' : 'Імпортувати плейлісти',
                      primary: true,
                      icon: 'arrow-right',
                      onTap: importing ? null : () => save(import: true),
                    ),
                  ],
                ),
                if (importing) ...[
                  const SizedBox(height: 16),
                  const LinearProgressIndicator(minHeight: 2),
                ],
                if (error.isNotEmpty) ...[
                  const SizedBox(height: 16),
                  Text(
                    error,
                    style: const TextStyle(
                      fontSize: 13,
                      height: 1.4,
                      color: Color(0xffedb0a7),
                    ),
                  ),
                ],
                const SizedBox(height: 20),
                Text(
                  'Імпортовані плейлісти з’являться в медіатеці.',
                  style: TextStyle(fontSize: 12, height: 1.4, color: p.muted),
                ),
              ],
            ),
          );
        },
      ),
    ),
  );
}
