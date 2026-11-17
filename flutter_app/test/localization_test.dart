import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:soundflow/core.dart';
import 'package:soundflow/design.dart';
import 'package:soundflow/l10n.dart';
import 'package:soundflow/main.dart';
import 'package:soundflow/settings_page.dart';

class SettingsCore extends Core {
  SettingsCore() : super([]);
  final List<Json> patches = [];

  @override
  void command(Json value) {
    patches.add(value);
    if (value['type'] == 'settings') {
      settings = {...settings, ...object(value['patch'])};
      notifyListeners();
    }
  }
}

Widget localized(String language, WidgetBuilder builder) => MaterialApp(
  theme: soundflowTheme(true, true),
  locale: Locale(language),
  supportedLocales: const [Locale('uk'), Locale('en')],
  localizationsDelegates: GlobalMaterialLocalizations.delegates,
  home: Builder(builder: builder),
);

void main() {
  testWidgets('translation follows context locale and preserves unknown text', (
    tester,
  ) async {
    for (final language in ['uk', 'en', 'uk']) {
      await tester.pumpWidget(
        localized(
          language,
          (context) => Column(
            children: [
              Text('Налаштування'.tr(context)),
              Text('Runtime title that has no translation'.tr(context)),
              Text(
                'Версія {version}'.tr(context).replaceAll('{version}', '2.3'),
              ),
            ],
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(
        find.text(language == 'en' ? 'Settings' : 'Налаштування'),
        findsOneWidget,
      );
      expect(
        find.text('Runtime title that has no translation'),
        findsOneWidget,
      );
      expect(
        find.text(language == 'en' ? 'Version 2.3' : 'Версія 2.3'),
        findsOneWidget,
      );
    }
  });

  testWidgets('helpers translate only fallback metadata and count labels', (
    tester,
  ) async {
    await tester.pumpWidget(
      localized(
        'en',
        (context) => Column(
          children: [
            Text(trackTitle({'title': 'Налаштування'}, context)),
            Text(trackArtist({'artist': 'Пошук'}, context)),
            Text(trackTitle({}, context)),
            Text(trackArtist({}, context)),
            Text(sourceName({'source': 'local'}, context)),
            Text(trackCount(1, context)),
            Text(trackCount(22, context)),
          ],
        ),
      ),
    );
    expect(find.text('Налаштування'), findsOneWidget);
    expect(find.text('Пошук'), findsOneWidget);
    expect(find.text('Untitled'), findsOneWidget);
    expect(find.text('Unknown artist'), findsOneWidget);
    expect(find.text('Local'), findsOneWidget);
    expect(find.text('1 track'), findsOneWidget);
    expect(find.text('22 tracks'), findsOneWidget);
    expect(trackCount(22), '22 треки');
    expect(trackCount(11), '11 треків');
  });

  testWidgets('settings dropdown saves language through settings command', (
    tester,
  ) async {
    final core = SettingsCore();
    await tester.pumpWidget(
      ListenableBuilder(
        listenable: core,
        builder: (context, _) => localized(
          core.language,
          (_) =>
              Scaffold(body: SingleChildScrollView(child: SettingsPage(core))),
        ),
      ),
    );
    await tester.tap(find.byType(DropdownButton<String>));
    await tester.pumpAndSettle();
    await tester.tap(find.text('English').last);
    await tester.pumpAndSettle();
    expect(core.language, 'en');
    expect(
      core.patches.single,
      equals({
        'type': 'settings',
        'patch': {'language': 'en'},
      }),
    );
    expect(find.text('Interface language'), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
    core.dispose();
  });

  testWidgets('main app uses persisted language with safe default', (
    tester,
  ) async {
    final core = SettingsCore();
    expect(core.language, 'uk');
    core.settings = {'language': 'en'};
    await tester.pumpWidget(SoundFlow(core));
    final app = tester.widget<MaterialApp>(find.byType(MaterialApp));
    expect(app.locale, const Locale('en'));
    expect(
      app.supportedLocales,
      containsAll([const Locale('uk'), const Locale('en')]),
    );
    await tester.pumpWidget(const SizedBox());
    core.dispose();
  });
}
