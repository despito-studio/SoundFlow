import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:soundflow/design.dart';
import 'package:soundflow/settings_page.dart';
import 'redesign_test.dart' show PreviewCore;

void main() {
  testWidgets('backup list loads and restore requires confirmation', (tester) async {
    final core = PreviewCore();
    core.state['library_backups'] = {
      'backups': [
        {'id': 'saved.json', 'created_at': 1794744000, 'kind': 'manual',
         'playlist_count': 3, 'liked_count': 12, 'history_count': 20},
      ],
    };
    await tester.pumpWidget(MaterialApp(
      locale: const Locale('uk'),
      supportedLocales: const [Locale('uk'), Locale('en')],
      localizationsDelegates: GlobalMaterialLocalizations.delegates,

      theme: soundflowTheme(true, true),
      home: Scaffold(body: SingleChildScrollView(child: SettingsPage(core))),
    ));
    await tester.tap(find.text('Медіатека'));
    await tester.pumpAndSettle();
    expect(core.commands.last['method'], 'list_library_backups');
    await tester.ensureVisible(find.text('Створити копію'));
    await tester.tap(find.text('Створити копію'));
    expect(core.commands.last['method'], 'create_library_backup');
    await tester.ensureVisible(find.text('Відновити'));
    await tester.tap(find.text('Відновити'));
    await tester.pumpAndSettle();
    expect(find.text('Відновити бібліотеку?'), findsOneWidget);
    expect(core.commands.where((c) => c['method'] == 'restore_library_backup'), isEmpty);
    await tester.tap(find.text('Скасувати'));
    await tester.pumpAndSettle();
    expect(core.commands.where((c) => c['method'] == 'restore_library_backup'), isEmpty);
    await tester.tap(find.text('Відновити'));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(TextButton, 'Відновити'));
    await tester.pumpAndSettle();
    expect(core.commands.last['method'], 'restore_library_backup');
    expect(core.commands.last['args'], ['saved.json']);
    await tester.pumpWidget(const SizedBox.shrink());
    core.dispose();
  });
}
