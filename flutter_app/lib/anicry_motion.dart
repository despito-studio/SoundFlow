import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:animate_icons/animate_icons.dart';

abstract final class Anicry {
  static const easeOut = Cubic(.16, 1, .3, 1);
  static const easeInOut = Cubic(.22, .82, .24, 1);
  static const press = Duration(milliseconds: 120);
  static const popover = Duration(milliseconds: 160);
  static const reveal = Duration(milliseconds: 220);
  static const layout = Duration(milliseconds: 360);
  static const dialog = Duration(milliseconds: 280);
  // Anicry motion.ts: continuity uses layout/easeInOut; only new labels reveal.
  static const contentDelay = Duration(milliseconds: 50);
  static const contentOpacity = Duration(milliseconds: 80);
  static const contentExit = Duration(milliseconds: 160);
  static Future<void> load() => AnimateIcons.load();
}

Future<T?> showAnicryDialog<T>({
  required BuildContext context,
  required WidgetBuilder builder,
  bool barrierDismissible = true,
}) => showGeneralDialog<T>(
  context: context,
  barrierDismissible: barrierDismissible,
  barrierLabel: 'Закрити',
  barrierColor: Colors.black.withValues(alpha: .56),
  transitionDuration: MediaQuery.disableAnimationsOf(context)
      ? Duration.zero
      : Anicry.dialog,
  pageBuilder: (context, animation, secondary) =>
      SafeArea(child: Builder(builder: builder)),
  transitionBuilder: (context, animation, secondary, child) {
    final curve = CurvedAnimation(
      parent: animation,
      curve: Anicry.easeOut,
      reverseCurve: Anicry.easeInOut,
    );
    return FadeTransition(
      opacity: curve,
      child: ScaleTransition(
        scale: Tween<double>(begin: .96, end: 1).animate(curve),
        child: child,
      ),
    );
  },
);

class IconMotion extends InheritedNotifier<Animation<double>> {
  const IconMotion({super.key, required super.notifier, required super.child});
  static Animation<double>? of(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<IconMotion>()?.notifier;
}

class AnicryTap extends StatefulWidget {
  const AnicryTap({
    super.key,
    required this.child,
    required this.onTap,
    this.icon,
    this.circle = false,
    this.radius = 10,
    this.hoverColor,
    this.focusColor,
    this.autofocus = false,
    this.focusNode,
  });
  final Widget child;
  final VoidCallback? onTap;
  final String? icon;
  final bool circle;
  final double radius;
  final Color? hoverColor, focusColor;
  final bool autofocus;
  final FocusNode? focusNode;
  @override
  State<AnicryTap> createState() => _AnicryTapState();
}

class _AnicryTapState extends State<AnicryTap>
    with SingleTickerProviderStateMixin {
  late final controller = AnimationController(
    vsync: this,
    value: 1,
    duration: const Duration(milliseconds: 800),
  );
  bool pressed = false, hovered = false, focused = false;
  void animate() {
    if (widget.onTap == null || MediaQuery.disableAnimationsOf(context)) return;
    controller.duration = widget.icon == null
        ? const Duration(milliseconds: 300)
        : AnimateIcons.duration(widget.icon!);
    controller.forward(from: 0);
  }

  @override
  void dispose() {
    controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => IconMotion(
    notifier: controller,
    child: AnimatedScale(
      scale: pressed && !MediaQuery.disableAnimationsOf(context) ? .97 : 1,
      duration: MediaQuery.disableAnimationsOf(context)
          ? Duration.zero
          : Anicry.press,
      curve: Anicry.easeOut,
      child: FocusableActionDetector(
        autofocus: widget.autofocus,
        focusNode: widget.focusNode,
        enabled: widget.onTap != null,
        mouseCursor: widget.onTap == null
            ? SystemMouseCursors.basic
            : SystemMouseCursors.click,
        shortcuts: const {
          SingleActivator(LogicalKeyboardKey.enter): ActivateIntent(),
          SingleActivator(LogicalKeyboardKey.space): ActivateIntent(),
        },
        actions: {
          ActivateIntent: CallbackAction<ActivateIntent>(
            onInvoke: (_) {
              animate();
              widget.onTap?.call();
              return null;
            },
          ),
        },
        onShowHoverHighlight: (value) {
          setState(() => hovered = value);
          if (value) {
            animate();
          } else if (!MediaQuery.disableAnimationsOf(context)) {
            controller.animateTo(
              1,
              duration: Anicry.press,
              curve: Anicry.easeOut,
            );
          }
        },
        onShowFocusHighlight: (value) => setState(() => focused = value),
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: widget.onTap == null
              ? null
              : () {
                  animate();
                  widget.onTap!();
                },
          onTapDown: widget.onTap == null
              ? null
              : (_) => setState(() => pressed = true),
          onTapUp: widget.onTap == null
              ? null
              : (_) => setState(() => pressed = false),
          onTapCancel: () => setState(() => pressed = false),
          child: AnimatedContainer(
            duration: MediaQuery.disableAnimationsOf(context)
                ? Duration.zero
                : Anicry.press,
            curve: Anicry.easeOut,
            decoration: BoxDecoration(
              shape: widget.circle ? BoxShape.circle : BoxShape.rectangle,
              borderRadius: widget.circle
                  ? null
                  : BorderRadius.circular(widget.radius),
              color: hovered || pressed
                  ? (widget.hoverColor ??
                            Theme.of(context).colorScheme.onSurface)
                        .withValues(alpha: pressed ? .10 : .05)
                  : Colors.transparent,
            ),
            foregroundDecoration: BoxDecoration(
              shape: widget.circle ? BoxShape.circle : BoxShape.rectangle,
              borderRadius: widget.circle
                  ? null
                  : BorderRadius.circular(widget.radius),
              border: Border.all(
                color: focused
                    ? (widget.focusColor ??
                          Theme.of(context).colorScheme.primary)
                    : Colors.transparent,
              ),
            ),
            child: widget.child,
          ),
        ),
      ),
    ),
  );
}

class AnicryGlyph extends StatelessWidget {
  const AnicryGlyph(
    this.name, {
    super.key,
    required this.size,
    required this.color,
    this.filled = false,
  });
  final String name;
  final double size;
  final Color color;
  final bool filled;
  @override
  Widget build(BuildContext context) => AnimateIcon(
    name == 'more' ? 'ellipsis' : name,
    size: size,
    color: color,
    filled: filled,
    animation: IconMotion.of(context),
  );
}
