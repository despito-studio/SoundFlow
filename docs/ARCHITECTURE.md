# Flutter / Rust architecture

The Windows executable hosts Flutter and loads soundflow_lib.dll through dart:ffi. Rust retains NativeSession, audio decoding, the library, account isolation, SQLite listening history, native dialogs and the provider worker. Flutter owns visual composition and route history.

The C ABI accepts bounded UTF-8 JSON requests, catches Rust panics before they reach Dart, and provides explicit ownership through sf_free. sf_open creates the shared session, sf_poll returns state and shell events, sf_command dispatches actions, and sf_close shuts down audio and owned workers.

The primary Flutter engine owns the Rust session. The optional Island engine receives playback state through the Windows MethodChannel and sends commands back to the primary engine. It does not create a second audio player or library. A native alpha surface supplies its smooth outer contour. Desktop, foreground and minimize events control DWM cloaking of both surfaces. Quick mode does not activate the window; keyboard mode permits focus.

Flutter renders icons from the full licensed AnimateIcons catalog. Artwork is decoded through Flutter image streams. Floating playback controls use matte surfaces. The header fades linearly from opaque #181818 to transparent. Main content has additional scroll space while playback controls are present.

Provider extraction and network work remain outside the UI thread. NativeSession preserves generation checks for stale playback requests, account boundaries, duplicate queue occurrences and bounded asynchronous command queues. The optional bundled Node worker is contained by the existing Windows Job object.

The previous UI sources are archived locally under output/previous-slint-source-20261021 and are not compiled or linked. Cargo produces a library; the user-facing executable is built from flutter_app/windows.
