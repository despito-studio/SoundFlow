import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:soundflow/account_login.dart';
import 'package:soundflow/core.dart';
import 'package:soundflow/design.dart';

class LoginCore extends Core {
  LoginCore(): super([]);
  final commands = <Json>[];
  @override
  void command(Json value) { commands.add(value); }
}
void main() {
  testWidgets('server signup keeps confirmation pending instead of claiming login', (tester) async {
    final core = LoginCore();
    await tester.pumpWidget(MaterialApp(
      locale: const Locale('uk'),
      supportedLocales: const [Locale('uk'), Locale('en')],
      localizationsDelegates: GlobalMaterialLocalizations.delegates,
theme: soundflowTheme(true, true), home: AccountLogin(core, server: true)));
    await tester.tap(find.text('Створити новий акаунт'));
    await tester.pump();
    final fields = find.byType(TextFormField);
    await tester.enterText(fields.at(0), 'Listener');
    await tester.enterText(fields.at(1), 'listener@example.com');
    await tester.enterText(fields.at(2), 'test-password');
    await tester.enterText(fields.at(3), 'test-password');
    await tester.tap(find.widgetWithText(FilledButton, 'Створити акаунт'));
    await tester.pump();
    expect(core.commands.single['method'], 'supabase_sign_up');
    core.state = {'last_method':'supabase_sign_up', 'last_result': {'authenticated':false,'confirmation_required':true}};
    core.notifyListeners();
    await tester.pump();
    expect(find.textContaining('Перевір пошту'), findsOneWidget);
    expect(find.byType(AccountLogin), findsOneWidget);
    expect(find.byType(LinearProgressIndicator), findsNothing);
    await tester.pumpWidget(const SizedBox.shrink());
    core.dispose();
  });
  testWidgets('registration validates confirmation and prevents duplicate submit', (tester) async {
    final core = LoginCore();
    await tester.pumpWidget(MaterialApp(
      locale: const Locale('uk'),
      supportedLocales: const [Locale('uk'), Locale('en')],
      localizationsDelegates: GlobalMaterialLocalizations.delegates,
theme: soundflowTheme(true, true), home: AccountLogin(core)));
    await tester.tap(find.text('Створити новий акаунт'));
    await tester.pump();
    final fields = find.byType(TextFormField);
    await tester.enterText(fields.at(0), 'Слухач');
    await tester.enterText(fields.at(1), 'listener@example.com');
    await tester.enterText(fields.at(2), 'long-password');
    await tester.enterText(fields.at(3), 'wrong');
    await tester.tap(find.widgetWithText(FilledButton, 'Створити акаунт'));
    await tester.pump();
    expect(core.commands, isEmpty);
    expect(find.text('Паролі не збігаються'), findsOneWidget);
    await tester.enterText(fields.at(3), 'long-password');
    await tester.tap(find.widgetWithText(FilledButton, 'Створити акаунт'));
    await tester.pump();
    expect(core.commands.single['method'], 'sign_up');
    await tester.tap(find.widgetWithText(FilledButton, 'Створити акаунт'));
    expect(core.commands, hasLength(1));
    core.state = {'last_method':'sign_up', 'last_result': {'ok':false,'error':'Ця адреса вже зареєстрована'}};
    core.notifyListeners();
    await tester.pump();
    expect(find.text('Ця адреса вже зареєстрована'), findsOneWidget);
    expect(find.byType(LinearProgressIndicator), findsNothing);
    await tester.pumpWidget(const SizedBox.shrink());
    core.dispose();
  });
}
