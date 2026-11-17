import 'l10n.dart';

import 'package:flutter/material.dart';

import 'design.dart';

/// Decorative transparent artwork never participates in input or accessibility.
class PortraitBackground extends StatelessWidget {
  const PortraitBackground({super.key, this.loading = false});
  final bool loading;

  @override
  Widget build(BuildContext context) => IgnorePointer(
    child: ExcludeSemantics(
      child: LayoutBuilder(
        builder: (context, box) {
          final width = (box.maxWidth * .46).clamp(180.0, 560.0);
          return Align(
            alignment: Alignment.centerRight,
            child: Opacity(
              opacity: loading ? .65 : .09,
              child: Image.asset(
                'assets/backgrounds/portrait.png',
                width: width,
                height: box.maxHeight,
                fit: BoxFit.contain,
                alignment: Alignment.bottomRight,
                color: context.paints.ink,
                colorBlendMode: BlendMode.srcIn,
                cacheWidth: 900,
                errorBuilder: (_, _, _) => const SizedBox.shrink(),
              ),
            ),
          );
        },
      ),
    ),
  );
}

class PortraitStartup extends StatelessWidget {
  const PortraitStartup({super.key, required this.error});
  final String error;

  @override
  Widget build(BuildContext context) => Scaffold(
    body: Stack(
      children: [
        const Positioned.fill(child: PortraitBackground(loading: true)),
        Center(
          child: Container(
            constraints: const BoxConstraints(maxWidth: 420),
            margin: const EdgeInsets.all(24),
            padding: const EdgeInsets.all(28),
            decoration: BoxDecoration(
              color: context.paints.canvas.withValues(alpha: .94),
              borderRadius: BorderRadius.circular(20),
            ),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Text(
                  'SoundFlow',
                  style: TextStyle(fontSize: 32, fontWeight: FontWeight.w600),
                ),
                const SizedBox(height: 20),
                Text(
                  error.isEmpty
                      ? 'Відкриваємо твою медіатеку…'.tr(context)
                      : error.tr(context),
                ),
                if (error.isEmpty) ...[
                  const SizedBox(height: 24),
                  const LinearProgressIndicator(minHeight: 2),
                ],
              ],
            ),
          ),
        ),
      ],
    ),
  );
}
