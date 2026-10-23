import 'dart:math' as math;
import 'dart:async';

import 'package:animate_icons/animate_icons.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'anicry_motion.dart';
import 'core.dart';
import 'installer_design.dart';

/// Keeps the startup offer outside the workspace's navigation/shortcut scope.
class UpdatePromptHost extends StatefulWidget {
  const UpdatePromptHost({super.key, required this.core, required this.child});
  final Core core;
  final Widget child;
  @override
  State<UpdatePromptHost> createState() => _UpdatePromptHostState();
}

class _UpdatePromptHostState extends State<UpdatePromptHost> {
  final offered = <String>{};
  bool showing = false;
  Timer? retry;
  @override
  void initState() {
    super.initState();
    widget.core.addListener(consider);
    consider();
  }

  void consider() {
    final update = widget.core.update;
    final version = update['version']?.toString();
    if (!mounted ||
        showing ||
        update['installed'] != true ||
        widget.core.arguments.contains('--offline') ||
        widget.core.settings['auto_update_enabled'] == false ||
        !['available', 'ready'].contains(update['status']) ||
        version == null ||
        !offered.add(version)) {
      return;
    }
    showing = true;
    widget.core.updatePromptOpen = true;
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mounted) return;
      if (ModalRoute.of(context)?.isCurrent == false) {
        offered.remove(version);
        widget.core.updatePromptOpen = false;
        retry = Timer(const Duration(milliseconds: 300), () {
          showing = false;
          if (mounted) consider();
        });
        return;
      }
      await showAnicryDialog<void>(
        context: context,
        barrierDismissible: false,
        builder: (_) => UpdatePrompt(widget.core),
      );
      showing = false;
      if (mounted) widget.core.closeUpdatePrompt();
    });
  }

  @override
  void dispose() {
    widget.core.removeListener(consider);
    retry?.cancel();
    widget.core.updatePromptOpen = false;
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => widget.child;
}

class UpdatePrompt extends StatefulWidget {
  const UpdatePrompt(this.core, {super.key});
  final Core core;
  @override
  State<UpdatePrompt> createState() => _UpdatePromptState();
}

class _UpdatePromptState extends State<UpdatePrompt> {
  bool requested = false, cancelling = false, closing = false;
  final updateFocus = FocusNode(), cancelFocus = FocusNode();
  int submittedSequence = -1;
  late final String version = widget.core.update['version']?.toString() ?? '';
  late final int total = (widget.core.update['total'] as num?)?.toInt() ?? 0;
  String get status => widget.core.update['status']?.toString() ?? '';
  bool get locked => ['installing', 'restarting'].contains(status);
  @override
  void initState() {
    super.initState();
    widget.core.addListener(changed);
  }

  void changed() {
    if (!mounted) return;
    if (cancelling && status == 'cancelled' && !closing) {
      closing = true;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) Navigator.of(context).pop();
      });
    }
    if (status == 'error' &&
        (!requested ||
            (widget.core.update['sequence'] as num?)?.toInt() !=
                submittedSequence)) {
      requested = false;
      cancelling = false;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) updateFocus.requestFocus();
      });
    }
    setState(() {});
  }

  void install() {
    if (requested) return;
    setState(() {
      requested = true;
      cancelling = false;
      submittedSequence =
          (widget.core.update['sequence'] as num?)?.toInt() ?? 0;
    });
    widget.core.action('update-install');
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && !locked) cancelFocus.requestFocus();
    });
  }

  @override
  void dispose() {
    widget.core.removeListener(changed);
    updateFocus.dispose();
    cancelFocus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final reduced = MediaQuery.disableAnimationsOf(context);
    final compact =
        requested ||
        [
          'downloading',
          'verifying',
          'installing',
          'restarting',
        ].contains(status);
    final viewport = MediaQuery.sizeOf(context);
    final textScale = MediaQuery.textScalerOf(context).scale(14) / 14;
    final scale = math.min(
      InstallerDesign.scale * textScale,
      math.min((viewport.width - 32) / 282, (viewport.height - 32) / 190),
    );
    final received = (widget.core.update['received'] as num?)?.toDouble() ?? 0;
    final bytes =
        (widget.core.update['total'] as num?)?.toDouble() ?? total.toDouble();
    final message = status == 'downloading' && bytes > 0
        ? 'Завантаження · ${(received / bytes * 100).clamp(0, 100).round()}%'
        : status == 'available' || status == 'ready'
        ? 'Готуємо оновлення…'
        : widget.core.update['message']?.toString() ?? 'Оновлення…';
    final failed = status == 'error';
    Widget label(
      String text,
      double size, {
      Color? color,
      bool bold = false,
      bool center = false,
    }) => Text(
      text,
      textScaler: TextScaler.noScaling,
      textAlign: center ? TextAlign.center : TextAlign.start,
      style: TextStyle(
        fontFamily: 'Onest',
        fontSize: size * scale,
        fontWeight: bold ? FontWeight.w600 : FontWeight.w400,
        color: color ?? InstallerDesign.ink,
      ),
    );
    Widget at(Rect rect, Widget child) => Positioned(
      left: rect.left * scale,
      top: rect.top * scale,
      width: rect.width * scale,
      height: rect.height * scale,
      child: child,
    );
    Widget logo(double size) => ClipRRect(
      borderRadius: BorderRadius.circular(size * .22),
      child: Image.asset(
        'assets/app-icons/default.png',
        width: size,
        height: size,
        filterQuality: FilterQuality.high,
      ),
    );
    Widget changingLabel(
      String value,
      double size, {
      bool center = false,
      String? identity,
    }) => AnimatedSwitcher(
      duration: reduced
          ? Duration.zero
          : Anicry.contentExit + Anicry.contentOpacity,
      reverseDuration: reduced ? Duration.zero : Anicry.contentExit,
      switchInCurve: Interval(
        Anicry.contentExit.inMilliseconds /
            (Anicry.contentExit + Anicry.contentOpacity).inMilliseconds,
        1,
        curve: Anicry.easeOut,
      ),
      switchOutCurve: Curves.linear,
      layoutBuilder: (current, previous) => Stack(
        alignment: center ? Alignment.center : Alignment.topLeft,
        children: [...previous, ?current],
      ),
      transitionBuilder: (child, animation) =>
          FadeTransition(opacity: animation, child: child),
      child: KeyedSubtree(
        key: ValueKey(identity ?? value),
        child: label(
          value,
          size,
          center: center,
          color: center ? InstallerDesign.ink : InstallerDesign.muted,
        ),
      ),
    );
    final target = status == 'restarting'
        ? InstallerDesign.completeLayout
        : compact
        ? locked
              ? InstallerDesign.commitLayout
              : InstallerDesign.progressLayout
        : InstallerDesign.updateLayout;
    final actionText = compact
        ? cancelling
              ? 'Скасовуємо…'
              : 'Скасувати'
        : failed
        ? 'Спробувати ще раз'
        : 'Оновити';
    void cancel() {
      setState(() => cancelling = true);
      widget.core.action('update-cancel');
    }

    final metadata = Stack(
      children: [
        if (!failed) ...[
          at(
            const Rect.fromLTWH(15, 99, 58, 17),
            Center(child: label(version, 7.6, center: true)),
          ),
          at(
            const Rect.fromLTWH(15, 118, 58, 13),
            Center(
              child: label(
                'Версія',
                7.2,
                color: InstallerDesign.muted,
                center: true,
              ),
            ),
          ),
          for (final x in [76.0, 154.0])
            at(
              Rect.fromLTWH(x, 105, .8, 16),
              ColoredBox(color: InstallerDesign.muted.withValues(alpha: .2)),
            ),
          at(
            const Rect.fromLTWH(110, 99, 14, 14),
            AnimateIcon(
              'refresh-cw',
              size: 14 * scale,
              color: InstallerDesign.muted,
            ),
          ),
          at(
            const Rect.fromLTWH(80, 118, 70, 13),
            Center(
              child: label(
                'Автооновлення',
                7.2,
                color: InstallerDesign.muted,
                center: true,
              ),
            ),
          ),
          at(
            const Rect.fromLTWH(181, 99, 14, 14),
            AnimateIcon(
              'download',
              size: 14 * scale,
              color: InstallerDesign.muted,
            ),
          ),
          at(
            const Rect.fromLTWH(162, 118, 52, 13),
            Center(
              child: label(
                total > 0 ? '${(total / 1048576).toStringAsFixed(1)} МБ' : '—',
                7.2,
                color: InstallerDesign.muted,
                center: true,
              ),
            ),
          ),
        ] else
          at(
            const Rect.fromLTWH(16, 89, 198, 43),
            label(
              widget.core.update['message']?.toString() ??
                  'Не вдалося оновити. Спробуй ще раз.',
              8,
              color: InstallerDesign.muted,
            ),
          ),
      ],
    );
    return Material(
      type: MaterialType.transparency,
      child: CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.escape): () {
            if (!compact) Navigator.of(context).pop();
          },
        },
        child: PopScope(
          canPop: !compact,
          child: Center(
            child: TweenAnimationBuilder<InstallerLayout>(
              tween: _LayoutTween(end: target),
              duration: reduced ? Duration.zero : Anicry.layout,
              curve: Curves.linear,
              builder: (context, frame, _) => ClipRRect(
                borderRadius: BorderRadius.circular(frame.radius * scale),
                child: SizedBox(
                  key: const Key('update-prompt-card'),
                  width: frame.size.width * scale,
                  height: frame.size.height * scale,
                  child: ColoredBox(
                    color: InstallerDesign.background,
                    child: Stack(
                      clipBehavior: Clip.hardEdge,
                      children: [
                        Positioned.fill(
                          child: IgnorePointer(
                            child: ExcludeSemantics(
                              excluding: compact,
                              child: Opacity(
                                key: const Key('update-metadata'),
                                opacity: frame.metadata,
                                child: Transform.translate(
                                  offset: Offset(
                                    0,
                                    -6 * (1 - frame.metadata) * scale,
                                  ),
                                  child: OverflowBox(
                                    alignment: Alignment.topLeft,
                                    minWidth: 0,
                                    minHeight: 0,
                                    maxWidth:
                                        InstallerDesign.ready.width * scale,
                                    maxHeight:
                                        InstallerDesign.ready.height * scale,
                                    child: SizedBox(
                                      width:
                                          InstallerDesign.ready.width * scale,
                                      height:
                                          InstallerDesign.ready.height * scale,
                                      child: metadata,
                                    ),
                                  ),
                                ),
                              ),
                            ),
                          ),
                        ),
                        at(
                          Rect.fromCenter(
                            center: frame.logo.center,
                            width: 48,
                            height: 48,
                          ),
                          IgnorePointer(
                            child: Opacity(
                              opacity: frame.ring,
                              child: TickerMode(
                                enabled: !reduced && compact,
                                child: TweenAnimationBuilder<double>(
                                  tween: Tween(
                                    end: bytes > 0 ? received / bytes : 0,
                                  ),
                                  duration: reduced
                                      ? Duration.zero
                                      : Anicry.press,
                                  curve: Anicry.easeOut,
                                  builder: (_, value, _) =>
                                      CircularProgressIndicator(
                                        value:
                                            status == 'downloading' && bytes > 0
                                            ? value.clamp(0, 1)
                                            : null,
                                        strokeWidth: 1.8 * scale,
                                        color: InstallerDesign.blue,
                                        backgroundColor: const Color(
                                          0xff303036,
                                        ),
                                      ),
                                ),
                              ),
                            ),
                          ),
                        ),
                        at(
                          frame.logo,
                          KeyedSubtree(
                            key: const Key('update-shared-logo'),
                            child: logo(frame.logo.width * scale),
                          ),
                        ),
                        at(
                          frame.title,
                          KeyedSubtree(
                            key: const Key('update-shared-title'),
                            child: label(
                              'SoundFlow',
                              frame.titleSize,
                              bold: true,
                            ),
                          ),
                        ),
                        at(
                          const Rect.fromLTWH(93, 43, 117, 12),
                          Opacity(
                            opacity:
                                frame.metadata *
                                frame.metadata *
                                frame.metadata,
                            child: label(
                              'Нова версія',
                              7.8,
                              color: const Color(0xff9c95d8),
                            ),
                          ),
                        ),
                        at(
                          frame.detail,
                          changingLabel(
                            compact ? message : 'Музичний плеєр\nдля Windows',
                            8.6,
                            identity: compact
                                ? 'detail:$status'
                                : 'description',
                          ),
                        ),
                        at(
                          frame.action,
                          ExcludeFocus(
                            excluding: locked,
                            child: IgnorePointer(
                              ignoring: locked,
                              child: Opacity(
                                opacity: frame.actionOpacity,
                                child: AnicryTap(
                                  autofocus: !compact,
                                  focusNode: compact
                                      ? cancelFocus
                                      : updateFocus,
                                  onTap: locked || cancelling
                                      ? null
                                      : compact
                                      ? cancel
                                      : install,
                                  radius: frame.actionRadius * scale,
                                  focusColor: InstallerDesign.ink,
                                  hoverColor: InstallerDesign.ink,
                                  child: Container(
                                    key: const Key('update-shared-action'),
                                    alignment: Alignment.center,
                                    decoration: BoxDecoration(
                                      color: Color.lerp(
                                        InstallerDesign.blue,
                                        InstallerDesign.background,
                                        frame.outline,
                                      ),
                                      borderRadius: BorderRadius.circular(
                                        frame.actionRadius * scale,
                                      ),
                                      border: Border.all(
                                        color: InstallerDesign.muted.withValues(
                                          alpha: frame.outline,
                                        ),
                                        width: .65 * scale,
                                      ),
                                    ),
                                    child: Padding(
                                      padding: EdgeInsets.symmetric(
                                        horizontal: 6 * scale,
                                      ),
                                      child: FittedBox(
                                        fit: BoxFit.scaleDown,
                                        child: changingLabel(
                                          actionText,
                                          9.2,
                                          center: true,
                                        ),
                                      ),
                                    ),
                                  ),
                                ),
                              ),
                            ),
                          ),
                        ),
                        if (status == 'restarting')
                          at(
                            Rect.fromCenter(
                              center: frame.action.center,
                              width: 16,
                              height: 16,
                            ),
                            IgnorePointer(
                              child: Opacity(
                                opacity: frame.check,
                                child: TweenAnimationBuilder<double>(
                                  tween: Tween(begin: 0, end: 1),
                                  duration: reduced
                                      ? Duration.zero
                                      : AnimateIcons.duration('circle-check'),
                                  builder: (_, value, _) => AnimateIcon(
                                    'circle-check',
                                    key: const Key('update-complete-check'),
                                    size: 16 * scale,
                                    color: InstallerDesign.ink,
                                    animation: AlwaysStoppedAnimation(value),
                                  ),
                                ),
                              ),
                            ),
                          ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _LayoutTween extends Tween<InstallerLayout> {
  _LayoutTween({super.end});
  @override
  InstallerLayout lerp(double t) {
    final elapsed = t * Anicry.layout.inMilliseconds;
    return InstallerLayout.lerp(
      begin!,
      end!,
      Anicry.easeInOut.transform(t),
      entering: Anicry.easeOut.transform(
        ((elapsed - Anicry.contentDelay.inMilliseconds) /
                Anicry.contentOpacity.inMilliseconds)
            .clamp(0, 1),
      ),
      leaving: Anicry.easeOut.transform(
        (elapsed / Anicry.contentExit.inMilliseconds).clamp(0, 1),
      ),
      secondaryEntering: Anicry.easeOut.transform(
        ((elapsed - Anicry.reveal.inMilliseconds) /
                Anicry.contentOpacity.inMilliseconds)
            .clamp(0, 1),
      ),
      secondaryLeaving:
          1 -
          math
              .pow(
                1 -
                    Anicry.easeOut.transform(
                      (elapsed / Anicry.contentOpacity.inMilliseconds).clamp(
                        0,
                        1,
                      ),
                    ),
                3,
              )
              .toDouble(),
    );
  }
}
