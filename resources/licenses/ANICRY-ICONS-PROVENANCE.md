# SoundFlow Flutter icon and motion provenance

The current Flutter interface imports the local `animate_icons` package. It adapts
the complete official `@animateicons/vanilla` 0.1.0 catalog, including all 1,170
Lucide and Hugeicons entries, their SVG geometry, normal states and motion targets.
The package was downloaded directly from npm and its pinned SHA-512 integrity was
verified. `resources/icons/flutter-sources.json` records the archive and catalog
hash. `scripts/update-animateicons.py` reproduces this dependency without Anicry.

The native Flutter painter caches parsed paths, repaints individual icons from an
Animation, and handles transforms, opacity, strokes, dashed strokes and path-length
reveals. Indefinite repeats run once; CSS 3D perspective is flattened to 2D.
The React/JavaScript runtime is not embedded in the Windows UI.

Anicry remains the source of SoundFlow surface-motion tokens: 120ms press,
220ms reveal, 280ms dialog, 360ms layout and easeOut (.16, 1, .3, 1).
Upstream source: Anicry, `src/features/anicry/motion.ts`.
The SoundFlow adaptation lives in `flutter_app/lib/anicry_motion.dart`.
Controls provide accessible action names, focus, press and hover triggers.
Reduced-motion preferences disable icon travel and surface transitions.

The original AnimateIcons MIT license and Lucide ISC, Feather MIT and free
Hugeicons MIT notices are shipped as ANIMATEICONS-LICENSE.txt and
ANIMATEICONS-THIRD-PARTY-NOTICES.txt. Unused icon subsets and former toolkit
notices are not included in the current source distribution.
