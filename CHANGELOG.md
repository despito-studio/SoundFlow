# Changelog

## 3.2.0 — 2026-10-10

SoundFlow for Windows moves to a native Rust/Slint application. The Python,
Tauri and web UI implementations are removed from the active source tree.

- Compact light and dark themes, floating navigation and player, adaptive
  layouts, keyboard controls and Windows UI Automation accessibility.
- Native audio playback, local library, favorites, playlists and history.
- YouTube and SoundCloud search, playback and downloads through one bundled
  media worker. Spotify metadata uses an explicitly selected counterpart.
- Flow previews with skip, like and playlist actions. Preview placement uses
  the track midpoint when highlight metadata is unavailable.
- Listening time stored in SQLite; statistics require a server-verified Plus
  entitlement. Account, sync and payment servers are not deployed by this release.
- SoundFlow icons and product identity in the app, media worker, tray,
  installer and shortcuts. Single-instance activation and child-process cleanup.
- Per-user Windows installer and portable ZIP, including third-party notices.
  Installation preserves existing user libraries.

Validation: 23 Rust tests and 6 media-worker tests passed. Windows checks covered
native startup, accessibility, several window sizes, icons, instance activation,
graceful and forced worker cleanup, complete AAC/MP3 decoding, and installation.

Earlier Python releases remain available in the GitHub release history.
