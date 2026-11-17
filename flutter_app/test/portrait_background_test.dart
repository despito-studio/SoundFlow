import 'dart:io';
import 'dart:ui' as ui;
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:soundflow/design.dart';
import 'package:soundflow/portrait_background.dart';

void main() {
  setUpAll(() async {
    final loader = FontLoader('Onest')
      ..addFont(rootBundle.load('assets/fonts/Onest-Regular.ttf'))
      ..addFont(rootBundle.load('assets/fonts/Onest-SemiBold.ttf'));
    await loader.load();
  });
  testWidgets('startup artwork fits the application viewport', (tester) async {
    tester.view.physicalSize = const Size(1200, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final capture = GlobalKey();
    await tester.pumpWidget(RepaintBoundary(
      key: capture,
      child: MaterialApp(
      locale: const Locale('uk'),
      supportedLocales: const [Locale('uk'), Locale('en')],
      localizationsDelegates: GlobalMaterialLocalizations.delegates,
debugShowCheckedModeBanner: false, theme: soundflowTheme(true, true),
        home: const PortraitStartup(error: ''),
      ),
    ));
    await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 400)));
    await tester.pump(const Duration(milliseconds: 100));
    expect(find.byType(Image), findsOneWidget);
    expect(tester.takeException(), isNull);
    if (const bool.fromEnvironment('CAPTURE_UI')) {
      await tester.runAsync(() async {
      final boundary = capture.currentContext!.findRenderObject()! as RenderRepaintBoundary;
      final image = await boundary.toImage(pixelRatio: 1);
      final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
      final file = File('../output/portrait-startup-render.png');
      await file.writeAsBytes(bytes!.buffer.asUint8List());
      image.dispose();
      });
    }
    await tester.pumpWidget(const SizedBox.shrink());
  });
  testWidgets('portrait is decorative and does not block controls', (tester) async {
    var taps = 0;
    await tester.pumpWidget(MaterialApp(
      locale: const Locale('uk'),
      supportedLocales: const [Locale('uk'), Locale('en')],
      localizationsDelegates: GlobalMaterialLocalizations.delegates,

      theme: soundflowTheme(true, true),
      home: Scaffold(body: Stack(children: [
        Positioned.fill(child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: () => taps++,
          child: const SizedBox.expand(),
        )),
        const Positioned.fill(child: PortraitBackground()),
      ])),
    ));
    await tester.tapAt(const Offset(700, 300));
    expect(taps, 1);
    expect(find.byType(ExcludeSemantics), findsWidgets);
    expect(tester.takeException(), isNull);
  });
  testWidgets('startup errors replace loading indicator', (tester) async {
    await tester.pumpWidget(MaterialApp(
      locale: const Locale('uk'),
      supportedLocales: const [Locale('uk'), Locale('en')],
      localizationsDelegates: GlobalMaterialLocalizations.delegates,

      theme: soundflowTheme(false, true),
      home: const PortraitStartup(error: 'Не вдалося відкрити медіатеку'),
    ));
    expect(find.text('Не вдалося відкрити медіатеку'), findsOneWidget);
    expect(find.byType(LinearProgressIndicator), findsNothing);
    expect(tester.takeException(), isNull);
  });
}
