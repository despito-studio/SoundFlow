import 'dart:convert';
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:path_parsing/path_parsing.dart';

abstract final class AnimateIcons {
  static Map<String, dynamic> catalog = {};
  static final _icons = <String, _Node>{};
  static Future<void> load() async {
    catalog = Map<String, dynamic>.from(
      jsonDecode(
        await rootBundle.loadString(
          'packages/animate_icons/assets/catalog.json',
        ),
      ) as Map,
    );
  }

  static Iterable<String> get names => catalog.keys;
  static String resolve(String name) =>
      name.contains('/') ? name : 'lucide/$name';
  static _Node _icon(String name) => _icons.putIfAbsent(
    resolve(name),
    () => _Node(
      Map<String, dynamic>.from(
        catalog[resolve(name)] ?? catalog['lucide/info'] ?? {'tag': 'g'},
      ),
    ),
  );
  static Duration duration(String name) =>
      Duration(milliseconds: (_icon(name).duration * 1000).ceil());

  /// Build-time native export uses this same painter, timing and path trimming.
  static List<AnimateVectorStroke> vectorFrame(String name, double progress) {
    final strokes = <AnimateVectorStroke>[];
    final recorder = ui.PictureRecorder();
    _IconPainter(
      _icon(name),
      AlwaysStoppedAnimation(progress),
      Colors.white,
      false,
      vector: strokes,
    ).paint(Canvas(recorder), const Size(24, 24));
    recorder.endRecording().dispose();
    return strokes;
  }
}

class AnimateVectorStroke {
  const AnimateVectorStroke(
    this.points,
    this.width,
    this.opacity,
    this.closed,
    this.filled,
    this.cap,
    this.join,
  );
  final List<Offset> points;
  final double width, opacity;
  final bool closed, filled;
  final StrokeCap cap;
  final StrokeJoin join;
}

/// Accepts `heart`, `lucide/heart` or `huge/heart`, including upstream motion.
class AnimateIcon extends StatefulWidget {
  const AnimateIcon(
    this.name, {
    super.key,
    this.size = 24,
    this.color = Colors.white,
    this.filled = false,
    this.animation,
  });
  final String name;
  final double size;
  final Color color;
  final bool filled;
  final Animation<double>? animation;
  @override
  State<AnimateIcon> createState() => _AnimateIconState();
}

class _AnimateIconState extends State<AnimateIcon>
    with SingleTickerProviderStateMixin {
  late final controller = AnimationController(vsync: this, value: 1);
  @override
  void dispose() {
    controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final reduced = MediaQuery.disableAnimationsOf(context);
    final icon = AnimateIcons._icon(widget.name);
    final animation = reduced
        ? const AlwaysStoppedAnimation<double>(1)
        : widget.animation ?? controller;
    final picture = RepaintBoundary(
      child: SizedBox(
        width: widget.size,
        height: widget.size,
        child: CustomPaint(
          painter: _IconPainter(icon, animation, widget.color, widget.filled),
        ),
      ),
    );
    if (widget.animation != null || reduced) return picture;
    return MouseRegion(
      onEnter: (_) {
        controller.duration = AnimateIcons.duration(widget.name);
        controller.forward(from: 0);
      },
      onExit: (_) =>
          controller.animateTo(1, duration: const Duration(milliseconds: 120)),
      child: picture,
    );
  }
}

Map<String, dynamic> _map(dynamic value) =>
    value is Map ? Map<String, dynamic>.from(value) : {};
double _number(dynamic value, [double fallback = 0]) => value is num
    ? value.toDouble()
    : double.tryParse(value?.toString().replaceAll('px', '') ?? '') ?? fallback;

class _Node {
  _Node(Map<String, dynamic> data)
    : tag = data['tag'] ?? 'g',
      attrs = _map(data['attrs']),
      normal = _map(data['normal']),
      animate = _map(data['animate']),
      style = _map(data['style']),
      children = (data['children'] as List? ?? [])
          .map((v) => _Node(_map(v)))
          .toList();
  final String tag;
  final Map<String, dynamic> attrs, normal, animate, style;
  final List<_Node> children;
  Path? _path;
  List<ui.PathMetric>? _metrics;
  Map<String, dynamic> timing(String property) {
    final transition = _map(animate['transition']);
    return {...transition, ..._map(transition[property])};
  }

  double get duration {
    double total = .3;
    for (final key in animate.keys.where((k) => k != 'transition')) {
      final t = timing(key);
      final cycles = (_number(t['repeat']).clamp(0, 2) + 1);
      total = math.max(
        total,
        _number(t['delay']) + _number(t['duration'], .3) * cycles,
      );
    }
    for (final child in children) {
      total = math.max(total, child.duration);
    }
    return total;
  }

  dynamic value(String key, double elapsed, dynamic fallback) {
    final rest = normal[key] ?? style[key] ?? fallback;
    final target = animate[key];
    if (target == null) return rest is List ? rest.last : rest;
    final transition = timing(key);
    final length = math.max(.001, _number(transition['duration'], .3));
    final delay = _number(transition['delay']);
    // A delayed keyframe sequence holds its first keyframe. Holding the settled
    // normal state here made entry animations flash visible before disappearing.
    if (elapsed < delay) {
      return target is List && target.isNotEmpty
          ? target.first
          : rest is List
          ? rest.last
          : rest;
    }
    final cycles = _number(transition['repeat']).clamp(0, 2).toInt() + 1;
    final raw = ((elapsed - delay) / length).clamp(0.0, cycles.toDouble());
    final cycle = math.min(raw.floor(), cycles - 1);
    var t = raw >= cycles ? 1.0 : raw - cycle;
    if (transition['repeatType'] == 'reverse' && cycle.isOdd) t = 1 - t;
    final values = target is List ? target : [rest ?? target, target];
    if (values.length < 2) return values.first;
    final stops =
        transition['times'] is List &&
            (transition['times'] as List).length == values.length
        ? (transition['times'] as List).map((v) => _number(v)).toList()
        : List.generate(values.length, (i) => i / (values.length - 1));
    for (var i = 1; i < values.length; i++) {
      if (t <= stops[i]) {
        final span = stops[i] - stops[i - 1];
        var f = span <= 0 ? 1.0 : ((t - stops[i - 1]) / span).clamp(0.0, 1.0);
        final ease = transition['ease'];
        final curve = ease is List && ease.length == 4
            ? Cubic(
                _number(ease[0]),
                _number(ease[1]),
                _number(ease[2]),
                _number(ease[3]),
              )
            : switch (ease) {
                'linear' => Curves.linear,
                'easeOut' => Curves.easeOut,
                'easeIn' => Curves.easeIn,
                _ => Curves.easeInOut,
              };
        f = curve.transform(f);
        final a = values[i - 1], b = values[i];
        if (a is num && b is num) return a + (b - a) * f;
        return f < .5 ? a : b;
      }
    }
    return values.last;
  }

  Path get path => _path ??= _makePath();
  List<ui.PathMetric> get metrics =>
      _metrics ??= path.computeMetrics().toList();
  Path _makePath() {
    final p = Path();
    double n(String key, [double fallback = 0]) =>
        _number(attrs[key], fallback);
    switch (tag) {
      case 'path':
        writeSvgPathDataToPath(attrs['d']?.toString(), _Path(p));
      case 'circle':
        p.addOval(
          Rect.fromCircle(center: Offset(n('cx'), n('cy')), radius: n('r')),
        );
      case 'ellipse':
        p.addOval(
          Rect.fromCenter(
            center: Offset(n('cx'), n('cy')),
            width: n('rx') * 2,
            height: n('ry') * 2,
          ),
        );
      case 'rect':
        p.addRRect(
          RRect.fromRectAndCorners(
            Rect.fromLTWH(n('x'), n('y'), n('width'), n('height')),
            topLeft: Radius.elliptical(n('rx', n('ry')), n('ry', n('rx'))),
            topRight: Radius.elliptical(n('rx', n('ry')), n('ry', n('rx'))),
            bottomLeft: Radius.elliptical(n('rx', n('ry')), n('ry', n('rx'))),
            bottomRight: Radius.elliptical(n('rx', n('ry')), n('ry', n('rx'))),
          ),
        );
      case 'line':
        p.moveTo(n('x1'), n('y1'));
        p.lineTo(n('x2'), n('y2'));
      case 'polygon':
      case 'polyline':
        final points = RegExp(r'-?\d*\.?\d+(?:e[-+]?\d+)?')
            .allMatches(attrs['points']?.toString() ?? '')
            .map((m) => double.parse(m[0]!))
            .toList();
        for (var i = 0; i + 1 < points.length; i += 2) {
          if (i == 0) {
            p.moveTo(points[i], points[i + 1]);
          } else {
            p.lineTo(points[i], points[i + 1]);
          }
        }
        if (tag == 'polygon') p.close();
    }
    if (attrs['fill-rule'] == 'evenodd') p.fillType = PathFillType.evenOdd;
    return p;
  }
}

class _Path extends PathProxy {
  _Path(this.path);
  final Path path;
  @override
  void moveTo(double x, double y) => path.moveTo(x, y);
  @override
  void lineTo(double x, double y) => path.lineTo(x, y);
  @override
  void cubicTo(
    double x1,
    double y1,
    double x2,
    double y2,
    double x3,
    double y3,
  ) => path.cubicTo(x1, y1, x2, y2, x3, y3);
  @override
  void close() => path.close();
}

class _IconPainter extends CustomPainter {
  _IconPainter(
    this.icon,
    this.animation,
    this.color,
    this.filled, {
    this.vector,
  }) : super(repaint: animation);
  final _Node icon;
  final Animation<double> animation;
  final Color color;
  final bool filled;
  final List<AnimateVectorStroke>? vector;
  final clips = <String, Path>{};
  @override
  void paint(Canvas canvas, Size size) {
    canvas.save();
    final viewBox = (icon.attrs['viewBox']?.toString() ?? '0 0 24 24')
        .split(RegExp(r'[ ,]+'))
        .map(double.parse)
        .toList();
    canvas.scale(size.width / viewBox[2], size.height / viewBox[3]);
    canvas.translate(-viewBox[0], -viewBox[1]);
    void collect(_Node n) {
      if (n.tag == 'clipPath') {
        final path = Path();
        for (final child in n.children) {
          path.addPath(child.path, Offset.zero);
        }
        clips[n.attrs['id']?.toString() ?? ''] = path;
      }
      for (final child in n.children) {
        collect(child);
      }
    }

    collect(icon);
    _draw(canvas, icon, {}, 1, animation.value * icon.duration);
    canvas.restore();
  }

  void _draw(
    Canvas canvas,
    _Node node,
    Map<String, dynamic> inherited,
    double opacity,
    double elapsed,
  ) {
    if (node.tag == 'defs' || node.tag == 'clipPath') return;
    final attrs = {...inherited, ...node.attrs};
    double v(String key, double fallback) =>
        _number(node.value(key, elapsed, fallback), fallback);
    opacity *= v('opacity', _number(node.attrs['opacity'], 1));
    if (opacity <= 0) return;
    canvas.save();
    final originX = node.value('originX', elapsed, '12px'),
        originY = node.value('originY', elapsed, '12px');
    double origin(dynamic val) => val.toString().endsWith('%')
        ? _number(val.toString().replaceAll('%', '')) * .24
        : _number(val, 12);
    final ox = origin(originX), oy = origin(originY);
    final matrix = Matrix4.identity()
      ..translateByDouble(v('x', 0) + ox, v('y', 0) + oy, 0, 1)
      ..rotateZ(v('rotate', 0) * math.pi / 180)
      ..rotateX(v('rotateX', 0) * math.pi / 180)
      ..scaleByDouble(
        v('scale', 1) * v('scaleX', 1),
        v('scale', 1) * v('scaleY', 1),
        1,
        1,
      );
    matrix.setEntry(
      0,
      1,
      matrix.entry(0, 1) + math.tan(v('skewX', 0) * math.pi / 180),
    );
    matrix.setEntry(
      1,
      0,
      matrix.entry(1, 0) + math.tan(v('skewY', 0) * math.pi / 180),
    );
    matrix.translateByDouble(-ox, -oy, 0, 1);
    canvas.transform(matrix.storage);
    final transform = attrs['transform']?.toString() ?? '';
    for (final m in RegExp(
      r'(translate|rotate|scale|matrix)\(([^)]+)\)',
    ).allMatches(transform)) {
      final n = m[2]!.split(RegExp(r'[ ,]+')).map(double.parse).toList();
      switch (m[1]) {
        case 'translate':
          canvas.translate(n[0], n.length > 1 ? n[1] : 0);
        case 'scale':
          canvas.scale(n[0], n.length > 1 ? n[1] : n[0]);
        case 'rotate':
          if (n.length == 3) canvas.translate(n[1], n[2]);
          canvas.rotate(n[0] * math.pi / 180);
          if (n.length == 3) canvas.translate(-n[1], -n[2]);
        case 'matrix':
          if (n.length == 6) {
            final t = Matrix4.identity()
              ..setEntry(0, 0, n[0])
              ..setEntry(1, 0, n[1])
              ..setEntry(0, 1, n[2])
              ..setEntry(1, 1, n[3])
              ..setEntry(0, 3, n[4])
              ..setEntry(1, 3, n[5]);
            canvas.transform(t.storage);
          }
      }
    }
    final clipId = RegExp(r'url\(#([^)]*)\)')
        .firstMatch(node.attrs['clip-path']?.toString() ?? '')?[1];
    if (clips[clipId] != null) canvas.clipPath(clips[clipId]!);
    void draw(Path path, Paint paint) {
      canvas.drawPath(path, paint);
      if (vector == null) return;
      if (clips[clipId] != null) {
        throw UnsupportedError(
          'Native vector export requires an unclipped icon.',
        );
      }
      final matrix = canvas.getTransform();
      final sx = math.sqrt(matrix[0] * matrix[0] + matrix[1] * matrix[1]);
      final sy = math.sqrt(matrix[4] * matrix[4] + matrix[5] * matrix[5]);
      if ((sx - sy).abs() > .0001 ||
          (matrix[0] * matrix[4] + matrix[1] * matrix[5]).abs() > .0001) {
        throw UnsupportedError(
          'Native vector export requires uniform stroke transforms.',
        );
      }
      for (final metric in path.transform(matrix).computeMetrics()) {
        final count = math.max(1, (metric.length / .10).ceil());
        final points = <Offset>[];
        for (var i = 0; i <= count; i++) {
          final tangent = metric.getTangentForOffset(metric.length * i / count);
          if (tangent != null) points.add(tangent.position);
        }
        if (points.length > 1) {
          vector!.add(
            AnimateVectorStroke(
              points,
              paint.strokeWidth * sx,
              paint.color.a,
              metric.isClosed,
              paint.style == PaintingStyle.fill,
              paint.strokeCap,
              paint.strokeJoin,
            ),
          );
        }
      }
    }

    if (node.tag != 'g' && node.tag != 'svg') {
      final paint = Paint()
        ..isAntiAlias = true
        ..color = color.withValues(alpha: color.a * opacity.clamp(0, 1));
      if (filled || (attrs['fill'] != null && attrs['fill'] != 'none')) {
        draw(node.path, paint);
      }
      if (attrs['stroke'] != null && attrs['stroke'] != 'none') {
        paint
          ..style = PaintingStyle.stroke
          ..strokeWidth = v('strokeWidth', _number(attrs['stroke-width'], 2))
          ..strokeCap = switch (attrs['stroke-linecap']) {
            'butt' => StrokeCap.butt,
            'square' => StrokeCap.square,
            _ => StrokeCap.round,
          }
          ..strokeJoin = switch (attrs['stroke-linejoin']) {
            'bevel' => StrokeJoin.bevel,
            'miter' => StrokeJoin.miter,
            _ => StrokeJoin.round,
          };
        final length = v('pathLength', 1).clamp(0.0, 1.0),
            offset = v('pathOffset', 0);
        final dash = node.value(
          'strokeDasharray',
          elapsed,
          attrs['stroke-dasharray'],
        );
        if (length < 1 || offset != 0 || dash != null) {
          for (final metric in node.metrics) {
            if (length < 1 || offset != 0) {
              draw(
                metric.extractPath(
                  metric.length * offset,
                  metric.length * (offset + length).clamp(0, 1),
                ),
                paint,
              );
            } else {
              final pattern = RegExp(r'\d*\.?\d+')
                  .allMatches(dash.toString())
                  .map((m) => double.parse(m[0]!))
                  .where((n) => n > 0)
                  .toList();
              if (pattern.isEmpty) {
                draw(node.path, paint);
                break;
              }
              if (pattern.length.isOdd) pattern.addAll(List.of(pattern));
              final scale = attrs['pathLength'] == null
                  ? 1.0
                  : metric.length / _number(attrs['pathLength'], 1);
              final sum = pattern.fold<double>(0, (a, b) => a + b) * scale;
              var start =
                  -v('strokeDashoffset', _number(attrs['stroke-dashoffset'])) *
                      scale %
                      sum -
                  sum;
              var i = 0;
              while (start < metric.length) {
                final end = start + pattern[i % pattern.length] * scale;
                if (i.isEven && end > 0) {
                  draw(
                    metric.extractPath(
                      math.max(0, start),
                      math.min(metric.length, end),
                    ),
                    paint,
                  );
                }
                start = end;
                i++;
              }
            }
          }
        } else {
          draw(node.path, paint);
        }
      }
    }
    final inheritable = Map<String, dynamic>.from(attrs)
      ..remove('transform')
      ..remove('clip-path')
      ..remove('id')
      ..remove('opacity');
    for (final child in node.children) {
      _draw(canvas, child, inheritable, opacity, elapsed);
    }
    canvas.restore();
  }

  @override
  bool shouldRepaint(covariant _IconPainter old) =>
      old.icon != icon ||
      old.color != color ||
      old.animation != animation ||
      old.filled != filled;
}
