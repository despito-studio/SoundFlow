# AnimateIcons for SoundFlow Flutter

This local Flutter package adapts the **complete official AnimateIcons catalog**
from `@animateicons/vanilla` 0.1.0: 1,170 Lucide and Hugeicons entries, original
SVG geometry, normal states, and motion targets. It does not copy Anicry's subset.
Source: https://github.com/Avijit07x/animateicons and https://animateicons.in.

```dart
await AnimateIcons.load();
AnimateIcon('lucide/heart', size: 24, color: Colors.white);
AnimateIcon('huge/headphones', animation: controller);
```

`AnimateIcons.names` exposes the entire catalog. Unprefixed names select Lucide.
The full catalog asset remains bundled. Loading indexes encoded JSON entries
without decoding their nested graphs, and bypasses the asset string cache.
Entries are decoded on access; only icons used by the renderer retain parsed
nodes and paths. `AnimateIcons.catalog` remains a map with the original keys and
values, but each lookup returns a fresh decoded value: nested mutations of a
returned value do not change the catalog. Assign a complete entry to replace it.
Replacing the catalog or calling `load()` clears the renderer's parsed cache.
This reduces retained object graphs; process memory savings depend on the engine
and the number of icons used and must be measured in the application.
`AnimateIcons.duration(name)` supplies the finite animation duration. With no
external animation, hover starts one sequence. Applications can drive an
external controller for focus and press. System reduced motion displays the
resting icon without travel. Path parsing is cached, and animation repaints the
icon directly on Canvas rather than rebuilding or parsing SVG every frame.

The painter handles shape paths, nested transforms, opacity, strokes, dashed
strokes, path-length reveals and clipping. Indefinite upstream repeats run one
cycle; finite repeats are limited to three cycles. CSS 3D perspective is flattened
to the 2D canvas. Anicry supplies SoundFlow's surface-motion timing separately.

Run `scripts/update-animateicons.py` from the repository to reproduce the catalog
from the pinned official npm archive. The script checks its SHA-512 integrity.
Version, source URL and catalog hash are recorded in
`resources/icons/flutter-sources.json`. MIT and original third-party notices are
retained in this package and the Windows application's resources/licenses folder.
