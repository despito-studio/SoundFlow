# Flutter / Rust architecture

The Windows executable hosts Flutter and loads soundflow_lib.dll through dart:ffi. Rust retains NativeSession, audio decoding, the library, account isolation, SQLite listening history, native dialogs and the provider worker. Flutter owns visual composition and route history.

The C ABI accepts bounded UTF-8 JSON requests, catches Rust panics before they reach Dart, and provides explicit ownership through sf_free. sf_open creates the shared session, sf_poll returns state and shell events, sf_command dispatches actions, and sf_close shuts down audio and owned workers.

The primary Flutter engine owns the Rust session. The optional Island engine receives playback state through the Windows MethodChannel and sends commands back to the primary engine. It does not create a second audio player or library. A native alpha surface supplies its smooth outer contour. Desktop, foreground and minimize events control DWM cloaking of both surfaces. Quick mode does not activate the window; keyboard mode permits focus.

Flutter renders icons from the full licensed AnimateIcons catalog. Artwork is decoded through Flutter image streams. The floating player uses a clipped glass surface and expands through a vertical gesture into track details and an occurrence-aware queue. Search loading and errors are independent of other background jobs. The header fades linearly from opaque #181818 to transparent. Main content has additional scroll space while playback controls are present.

Provider extraction and network work remain outside the UI thread. NativeSession preserves generation checks for stale playback requests, account boundaries, duplicate queue occurrences and bounded asynchronous command queues. The optional bundled Node worker is contained by the existing Windows Job object.

Cargo produces a library; the user-facing executable is built from flutter_app/windows.

`NativeSession` owns an independent `update::Updater`. Its worker serializes release checks,
verified/cancellable downloads, cache restoration and installer handoff; its JSON state is
included in `sf_poll`. Flutter only schedules checks and presents progress/actions. The Windows
startup offer uses one `update-install` action; the updater keeps a single busy guard through
download, verification and handoff, and checks cancellation before framework ownership. The
shared installer-style card uses Onest, InstallerDesign and Anicry; startup checks do not
download before approval. The Windows
runner accepts an exact-process graceful-shutdown message so the installer never needs to kill
an audio process. The separate static-runtime native installer shows consent when a terms
website is configured (otherwise skipping that step), embeds a silent Velopack backend
and launches the application. Velopack 1.2.161 owns the managed installed/portable layout,
full/delta updates, apply and restart; GitHub Pages hosts its service feed/packages.
Releases contain only our Setup and managed portable ZIP. The early native entry point
runs Velopack startup before Flutter and profile locking. The same Flutter
AnimateIcons painter exports its transformed/trimmed vector strokes and catalog motion at
build time. Native Direct2D uses those vectors, with shared Anicry transitions and reduced motion.
