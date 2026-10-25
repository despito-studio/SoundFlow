import 'dart:async';

import 'package:flutter/material.dart';

import 'core.dart';

class MediaSlider extends StatefulWidget {
  const MediaSlider({
    super.key,
    required this.value,
    this.max = 1,
    required this.onChanged,
    this.continuous = false,
    this.semanticFormatterCallback,
    this.onPreview,
    this.focusNode,
  });
  final double value, max;
  final ValueChanged<double>? onChanged;
  final bool continuous;
  final ValueChanged<double>? onPreview;
  final FocusNode? focusNode;
  final SemanticFormatterCallback? semanticFormatterCallback;
  @override
  State<MediaSlider> createState() => _MediaSliderState();
}

class _MediaSliderState extends State<MediaSlider> {
  double? local;
  bool dragging = false;
  Timer? settle;
  @override
  void didUpdateWidget(covariant MediaSlider old) {
    super.didUpdateWidget(old);
    if (!dragging &&
        local != null &&
        (widget.value - local!).abs() < (widget.max > 1 ? 1.5 : .0005)) {
      local = null;
      settle?.cancel();
    }
  }

  @override
  void dispose() {
    settle?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => TweenAnimationBuilder<double>(
    tween: Tween<double>(end: (local ?? widget.value).clamp(0, widget.max)),
    duration: Duration(
      milliseconds: local != null || MediaQuery.disableAnimationsOf(context)
          ? 0
          : 250,
    ),
    curve: Curves.linear,
    builder: (context, value, _) => Slider(
      focusNode: widget.focusNode,
      value: value.clamp(0, widget.max),
      max: widget.max,
      semanticFormatterCallback: widget.semanticFormatterCallback,
      onChangeStart: widget.onChanged == null
          ? null
          : (v) {
              settle?.cancel();
              dragging = true;
              setState(() => local = v);
              widget.onPreview?.call(v);
            },
      onChanged: widget.onChanged == null
          ? null
          : (v) {
              setState(() => local = v);
              widget.onPreview?.call(v);
              if (widget.continuous) widget.onChanged!(v);
            },
      onChangeEnd: widget.onChanged == null
          ? null
          : (v) {
              dragging = false;
              widget.onChanged!(v);
              settle = Timer(const Duration(milliseconds: 400), () {
                if (mounted) setState(() => local = null);
              });
            },
    ),
  );
}

class PlaybackTimeline extends StatefulWidget {
  const PlaybackTimeline({
    super.key,
    required this.position,
    required this.duration,
    required this.onSeek,
    this.inline = false,
    this.fontSize = 11,
  });
  final double position, duration, fontSize;
  final ValueChanged<double>? onSeek;
  final bool inline;
  @override
  State<PlaybackTimeline> createState() => _PlaybackTimelineState();
}

class _PlaybackTimelineState extends State<PlaybackTimeline> {
  double? preview;
  bool dragging = false;
  Timer? settle;
  @override
  void didUpdateWidget(covariant PlaybackTimeline old) {
    super.didUpdateWidget(old);
    if (old.duration != widget.duration ||
        (!dragging &&
            preview != null &&
            (preview! - widget.position).abs() < 1.5)) {
      preview = null;
      settle?.cancel();
    }
  }

  @override
  void dispose() {
    settle?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final max = widget.duration > 0 ? widget.duration : 1.0;
    final position = (preview ?? widget.position).clamp(0.0, max);
    final style = TextStyle(
      fontSize: widget.fontSize,
      color: Theme.of(context).textTheme.bodySmall?.color,
    );
    final start = Text(
      clock(position),
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
      style: style,
    );
    final end = Text(
      clock(widget.duration),
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
      style: style,
      textAlign: TextAlign.right,
    );
    final slider = SizedBox(
      height: widget.inline ? 18 : 24,
      child: MediaSlider(
        value: position,
        max: max,
        semanticFormatterCallback: (v) =>
            '${clock(v)} із ${clock(widget.duration)}',
        onPreview: widget.onSeek == null
            ? null
            : (v) {
                settle?.cancel();
                dragging = true;
                setState(() => preview = v);
              },
        onChanged: widget.onSeek == null
            ? null
            : (v) {
                dragging = false;
                widget.onSeek!(v);
                settle = Timer(const Duration(milliseconds: 1200), () {
                  if (mounted) setState(() => preview = null);
                });
              },
      ),
    );
    return widget.inline
        ? Row(
            children: [
              SizedBox(width: 34, child: start),
              Expanded(child: slider),
              SizedBox(width: 34, child: end),
            ],
          )
        : Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              slider,
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [start, end],
              ),
            ],
          );
  }
}
