import 'l10n.dart';

import 'package:flutter/material.dart';

import 'core.dart';
import 'design.dart';

class AccountLogin extends StatefulWidget {
  const AccountLogin(this.core, {super.key, this.server = false});
  final Core core;
  final bool server;
  @override
  State<AccountLogin> createState() => _AccountLoginState();
}

class _AccountLoginState extends State<AccountLogin> {
  final form = GlobalKey<FormState>();
  final name = TextEditingController();
  final email = TextEditingController();
  final password = TextEditingController();
  final confirmation = TextEditingController();
  bool register = false, busy = false;
  String error = '';
  String? pendingMethod;
  Object? previousResult;
  @override
  void initState() {
    super.initState();
    widget.core.addListener(changed);
  }

  void changed() {
    if (!mounted ||
        !busy ||
        widget.core.state['last_method'] != pendingMethod ||
        identical(previousResult, widget.core.state['last_result'])) {
      return;
    }
    final result = object(widget.core.state['last_result']);
    if (result['ok'] == false || result['error'] != null) {
      setState(() {
        busy = false;
        error = result['error']?.toString() ?? 'Не вдалося увійти'.tr(context);
      });
    } else if (result['confirmation_required'] == true) {
      setState(() {
        busy = false;
        register = false;
        error = 'Перевір пошту й підтвердь адресу за посиланням у листі. Потім увійди.'
            .tr(context);
      });
    } else {
      busy = false;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) Navigator.of(context).pop();
      });
    }
  }

  void submit() {
    if (busy || !form.currentState!.validate()) return;
    setState(() {
      busy = true;
      error = '';
    });
    pendingMethod = widget.server
        ? (register ? 'supabase_sign_up' : 'supabase_sign_in')
        : (register ? 'sign_up' : 'sign_in');
    previousResult = widget.core.state['last_result'];
    widget.core.call(pendingMethod!, [
      {
        if (register) 'name': name.text.trim(),
        'email': email.text.trim(),
        'password': password.text,
      },
    ]);
  }

  @override
  void dispose() {
    widget.core.removeListener(changed);
    for (final c in [name, email, password, confirmation]) {
      c.dispose();
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
    title: Text(
      register
          ? 'Створити акаунт SoundFlow'.tr(context)
          : 'Вхід у SoundFlow'.tr(context),
    ),
    content: SizedBox(
      width: 420,
      child: SingleChildScrollView(
        child: Form(
          key: form,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                widget.server
                    ? 'Серверний акаунт SoundFlow. Після реєстрації підтвердь пошту. Адміністративна роль перевіряється на сервері.'
                          .tr(context)
                    : 'Профіль на цьому комп’ютері. Бібліотека зберігається локально; синхронізація між пристроями потребує підключеного сервера.'
                          .tr(context),
                style: TextStyle(fontSize: 12, color: context.paints.muted),
              ),
              const SizedBox(height: 20),
              if (register) ...[
                TextFormField(
                  controller: name,
                  enabled: !busy,
                  decoration: InputDecoration(labelText: 'Ім’я'.tr(context)),
                  validator: (v) => (v ?? '').trim().isEmpty
                      ? 'Вкажи ім’я'.tr(context)
                      : null,
                ),
                const SizedBox(height: 12),
              ],
              TextFormField(
                controller: email,
                enabled: !busy,
                keyboardType: TextInputType.emailAddress,
                decoration: InputDecoration(
                  labelText: 'Електронна пошта'.tr(context),
                ),
                validator: (v) =>
                    RegExp(r'^[^\s@]+@[^\s@]+\.[^\s@]+$')
                        .hasMatch((v ?? '').trim())
                    ? null
                    : 'Вкажи коректну адресу'.tr(context),
              ),
              const SizedBox(height: 12),
              TextFormField(
                controller: password,
                enabled: !busy,
                obscureText: true,
                enableSuggestions: false,
                autocorrect: false,
                decoration: InputDecoration(labelText: 'Пароль'.tr(context)),
                onFieldSubmitted: (_) => submit(),
                validator: (v) => (v ?? '').length < 8
                    ? 'Щонайменше 8 символів'.tr(context)
                    : null,
              ),
              if (register) ...[
                const SizedBox(height: 12),
                TextFormField(
                  controller: confirmation,
                  enabled: !busy,
                  obscureText: true,
                  enableSuggestions: false,
                  autocorrect: false,
                  decoration: InputDecoration(
                    labelText: 'Повтори пароль'.tr(context),
                  ),
                  onFieldSubmitted: (_) => submit(),
                  validator: (v) => v == password.text
                      ? null
                      : 'Паролі не збігаються'.tr(context),
                ),
              ],
              if (error.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.only(top: 14),
                  child: Text(
                    error.tr(context),
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.error,
                    ),
                  ),
                ),
              if (busy)
                const Padding(
                  padding: EdgeInsets.only(top: 16),
                  child: LinearProgressIndicator(),
                ),
              TextButton(
                onPressed: busy
                    ? null
                    : () => setState(() {
                        register = !register;
                        error = '';
                        confirmation.clear();
                      }),
                child: Text(
                  register
                      ? 'Уже є акаунт? Увійти'.tr(context)
                      : 'Створити новий акаунт'.tr(context),
                ),
              ),
            ],
          ),
        ),
      ),
    ),
    actions: [
      TextButton(
        onPressed: busy ? null : () => Navigator.pop(context),
        child: Text('Закрити'.tr(context)),
      ),
      FilledButton(
        onPressed: busy ? null : submit,
        child: Text(
          register ? 'Створити акаунт'.tr(context) : 'Увійти'.tr(context),
        ),
      ),
    ],
  );
}
