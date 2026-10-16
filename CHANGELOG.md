# Changelog

## 3.3.0 — 2026-10-16

SoundFlow 3.3.0 brings full bilingual localization, subscription management, cross-platform stabilization, and release readiness for both Linux and Windows.

- **Bilingual Interface (Ukrainian 🇺🇦 & English 🇺🇸)**:
  - Instant runtime language toggle between Ukrainian and English in Settings.
  - Fully localized views: Home, Search, Library, Playlists, Downloads, Flow, Stats, Settings, Connected Services, and all Modal dialogs.
  - Localized player bar, notifications, and accessibility labels without requiring an application restart.
- **SoundFlow Plus Subscription Management**:
  - Added full cancellation and opt-out support for active subscriptions across both Linux and Windows.
  - Direct cancellation button in the Stats banner, Subscription Plans modal, and Settings account view with confirmation protection.
- **Cross-Platform Bug Fixes & Stability**:
  - **Discord Rich Presence on Linux**: Implemented Unix domain socket IPC stream (`$XDG_RUNTIME_DIR/discord-ipc-*` / `/tmp/discord-ipc-*`) alongside Windows Named Pipes.
  - **Account Store Resilience**: Fixed unwrap panic on initial config load in `AccountsStore::open`.
  - **Mutex Concurrency Protection**: Guarded all UI thread and worker mutex locks against poisoned mutex crashes.
  - **Media Worker Safety**: Replaced unsafe process unwrap with graceful error handling in media provider lifecycle.
- **Automated Validation**:
  - 24 Rust unit tests passed with 100% success rate.
  - Clean Linux packages produced (`.tar.gz` and `.run` installer).

## 3.2.0 — 2026-10-10

SoundFlow moves to a native Rust/Slint application. The Python,
Tauri and web UI implementations are removed from the active source tree.

- Compact light and dark themes, floating navigation and player, adaptive
  layouts, keyboard controls and accessibility.
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

Validation: 23 Rust tests and 6 media-worker tests passed.
Earlier Python releases remain available in the GitHub release history.
