import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:soundflow/design.dart';

void main() {
  testWidgets('small covers decode at display size with HiDPI support', (tester) async {
    await tester.pumpWidget(MaterialApp(
      home: MediaQuery(
        data: const MediaQueryData(devicePixelRatio: 2),
        child: const Center(child: Artwork('missing-cover.jpg', size: 48)),
      ),
    ));
    final image = tester.widget<Image>(find.byType(Image));
    expect((image.image as ResizeImage).width, 128);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('responsive covers use their available width', (tester) async {
    await tester.pumpWidget(MaterialApp(
      home: MediaQuery(
        data: const MediaQueryData(devicePixelRatio: 1),
        child: const Center(child: SizedBox(
          width: 240, height: 240, child: Artwork('missing-cover.jpg'),
        )),
      ),
    ));
    final image = tester.widget<Image>(find.byType(Image));
    expect((image.image as ResizeImage).width, 256);
    await tester.pumpWidget(const SizedBox.shrink());
  });
}
