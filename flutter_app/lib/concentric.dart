import 'dart:math' as math;

import 'package:flutter/painting.dart';

/// Flutter adaptation of tailwindcss-concentric's edge-distance rule.
abstract final class SoundFlowCorners {
  static const panel = 24.0;
  static const card = 24.0;
  static const cardInset = 12.0;
  static const control = 12.0;
  static const compactControl = 10.0;

  static double inner(
    double outer,
    double gap, {
    double border = 0,
    double floor = 0,
  }) => math.max(floor, outer - gap - border);

  static BorderRadius inset(
    BorderRadius outer,
    EdgeInsets padding, {
    EdgeInsets border = EdgeInsets.zero,
    EdgeInsets wrapper = EdgeInsets.zero,
    EdgeInsets margin = EdgeInsets.zero,
    bool elliptical = false,
    double floor = 0,
  }) {
    final gap = padding + border + wrapper + margin;
    Radius corner(Radius radius, double x, double y) => Radius.elliptical(
      inner(radius.x, elliptical ? x : math.min(x, y), floor: floor),
      inner(radius.y, elliptical ? y : math.min(x, y), floor: floor),
    );
    return BorderRadius.only(
      topLeft: corner(outer.topLeft, gap.left, gap.top),
      topRight: corner(outer.topRight, gap.right, gap.top),
      bottomLeft: corner(outer.bottomLeft, gap.left, gap.bottom),
      bottomRight: corner(outer.bottomRight, gap.right, gap.bottom),
    );
  }
}
