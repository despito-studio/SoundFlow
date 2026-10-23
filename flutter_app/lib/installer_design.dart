import 'package:flutter/painting.dart';

import 'concentric.dart';
import 'design.dart';

/// A single scene: shared elements retain their identity while the card morphs.
/// The Windows renderer consumes these same endpoints and Anicry's timing.
class InstallerLayout {
  const InstallerLayout({
    required this.size,
    required this.radius,
    required this.logo,
    required this.title,
    required this.detail,
    required this.action,
    required this.titleSize,
    required this.actionRadius,
    this.metadata = 0,
    this.folder = 0,
    this.ring = 0,
    this.check = 0,
    this.actionOpacity = 1,
    this.outline = 0,
  });

  final Size size;
  final Rect logo, title, detail, action;
  final double radius, titleSize, actionRadius;
  final double metadata, folder, ring, check, actionOpacity, outline;

  static InstallerLayout lerp(
    InstallerLayout a,
    InstallerLayout b,
    double t, {
    double? entering,
    double? leaving,
    double? secondaryEntering,
    double? secondaryLeaving,
  }) {
    double mix(double a, double b) => a + (b - a) * t;
    double opacity(double a, double b) =>
        a + (b - a) * (b > a ? entering ?? t : leaving ?? t);
    double secondary(double a, double b) =>
        a +
        (b - a) *
            (b > a
                ? secondaryEntering ?? entering ?? t
                : secondaryLeaving ?? leaving ?? t);
    return InstallerLayout(
      size: Size.lerp(a.size, b.size, t)!,
      radius: mix(a.radius, b.radius),
      logo: Rect.lerp(a.logo, b.logo, t)!,
      title: Rect.lerp(a.title, b.title, t)!,
      detail: Rect.lerp(a.detail, b.detail, t)!,
      action: Rect.lerp(a.action, b.action, t)!,
      titleSize: mix(a.titleSize, b.titleSize),
      actionRadius: mix(a.actionRadius, b.actionRadius),
      metadata: secondary(a.metadata, b.metadata),
      folder: secondary(a.folder, b.folder),
      ring: opacity(a.ring, b.ring),
      check: opacity(a.check, b.check),
      actionOpacity: opacity(a.actionOpacity, b.actionOpacity),
      outline: mix(a.outline, b.outline),
    );
  }
}

/// Reference geometry adapted through the same corner rules as the player.
/// Exported for the native bootstrapper; it does not maintain a second rule.
abstract final class InstallerDesign {
  static const scale = 1.8;
  static const ready = Size(230, 186);
  static const compact = Size(282, 78);
  static const readyRadius = 43.0;
  static const compactRadius = 39.0;
  static const actions = Rect.fromLTWH(15, 143, 200, 22);
  static const download = Rect.fromLTWH(15, 143, 172, 22);
  static const folder = Rect.fromLTWH(193, 143, 22, 22);
  static const cancelGroup = Rect.fromLTWH(199, 17, 62, 44);
  static const cancel = Rect.fromLTWH(199, 26, 62, 26);
  static const focusGap = 1.5;
  static const compactControl = SoundFlowCorners.compactControl;
  static const control = SoundFlowCorners.control;

  // The reference's black surface and blue action are installer roles.
  static const background = Color(0xff131315);
  static const blue = Color(0xff0a84ff);
  static final ink = Paints.night.ink;
  static final muted = Paints.night.muted;

  static BorderRadius nested(BorderRadius outer, Rect parent, Rect child) =>
      _normalized(
        SoundFlowCorners.inset(
          outer,
          EdgeInsets.fromLTRB(
            child.left - parent.left,
            child.top - parent.top,
            parent.right - child.right,
            parent.bottom - child.bottom,
          ),
        ),
        child.size,
      );

  static BorderRadius _normalized(BorderRadius corners, Size size) {
    final shape = corners.toRRect(Offset.zero & size).scaleRadii();
    return BorderRadius.only(
      topLeft: shape.tlRadius,
      topRight: shape.trRadius,
      bottomLeft: shape.blRadius,
      bottomRight: shape.brRadius,
    );
  }

  static final actionCorners = nested(
    BorderRadius.circular(readyRadius),
    Offset.zero & ready,
    actions,
  );
  static final downloadCorners = nested(actionCorners, actions, download);
  static final folderCorners = nested(actionCorners, actions, folder);
  static final cancelCorners = nested(
    nested(
      BorderRadius.circular(compactRadius),
      Offset.zero & compact,
      cancelGroup,
    ),
    cancelGroup,
    cancel,
  );

  static final readyLayout = InstallerLayout(
    size: ready,
    radius: readyRadius,
    logo: const Rect.fromLTWH(24, 24, 52, 52),
    title: const Rect.fromLTWH(93, 26, 117, 20),
    detail: const Rect.fromLTWH(93, 55, 117, 25),
    action: download,
    titleSize: 12.5,
    actionRadius: downloadCorners.topLeft.x,
    metadata: 1,
    folder: 1,
  );
  static final updateLayout = InstallerLayout(
    size: ready,
    radius: readyRadius,
    logo: readyLayout.logo,
    title: readyLayout.title,
    detail: readyLayout.detail,
    action: actions,
    titleSize: readyLayout.titleSize,
    actionRadius: actionCorners.topLeft.x,
    metadata: 1,
  );
  static final progressLayout = InstallerLayout(
    size: compact,
    radius: compactRadius,
    logo: const Rect.fromLTWH(28, 26, 26, 26),
    title: const Rect.fromLTWH(81, 19, 110, 19),
    detail: const Rect.fromLTWH(81, 36, 110, 29),
    action: cancel,
    titleSize: 12.2,
    actionRadius: cancelCorners.topLeft.x,
    ring: 1,
    outline: 1,
  );
  static final commitLayout = InstallerLayout(
    size: compact,
    radius: compactRadius,
    logo: progressLayout.logo,
    title: progressLayout.title,
    detail: progressLayout.detail,
    action: cancel,
    titleSize: progressLayout.titleSize,
    actionRadius: progressLayout.actionRadius,
    ring: 1,
    outline: 1,
    actionOpacity: 0,
  );
  static final completeLayout = InstallerLayout(
    size: compact,
    radius: compactRadius,
    logo: const Rect.fromLTWH(16, 13, 52, 52),
    title: progressLayout.title,
    detail: progressLayout.detail,
    action: cancel,
    titleSize: progressLayout.titleSize,
    actionRadius: progressLayout.actionRadius,
    check: 1,
    outline: 1,
    actionOpacity: 0,
  );
  static const messageLayout = InstallerLayout(
    size: Size(230, 190),
    radius: readyRadius,
    logo: Rect.fromLTWH(16, 25, 30, 30),
    title: Rect.fromLTWH(57, 29, 157, 22),
    detail: Rect.fromLTWH(16, 67, 198, 78),
    action: Rect.fromLTWH(16, 156, 198, 24),
    titleSize: 10.5,
    actionRadius: control,
  );

  // A stable viewport prevents native HWND/backbuffer resize races mid-frame.
  static const viewport = Size(282, 190);

  static BorderRadius focus(BorderRadius corners, Size size) => _normalized(
    SoundFlowCorners.inset(corners, const EdgeInsets.all(-focusGap)),
    Size(size.width + focusGap * 2, size.height + focusGap * 2),
  );
}
