# Native architecture

The shipped Windows application has one Rust process and, only when online music is requested, one bundled JavaScript media worker. Slint renders native controls and exposes an AccessKit accessibility tree. Rodio/Symphonia decodes and plays audio in Rust. There is no DOM, frontend server, Python process or WebView runtime.

| Module | Responsibility |
| --- | --- |
| `application` | Typed UI bindings, queues, asynchronous commands and artwork |
| `desktop` | Application commands, native dialogs and provider boundary |
| `core`, `library` | Validated local data, account isolation and approved music folders |
| `listening` | SQLite sessions and imported listening events |
| `playback` | Audio actor, decoding, seeking, volume and audible time |
| `cloud` | HTTPS accounts, credential vault, entitlement and synchronization |
| `platform` | Windows identity, instance activation and native lifecycle |
| `media` | Bounded JSON-lines protocol and child Job object |
| `verification` | Explicit verification with isolated data and native snapshots |

The UI thread performs no extraction or blocking network calls. Audio loading uses generation counters so a stale load cannot replace a newer selection. Listening sessions retain the account they started under; pauses, muted output, buffering and seeks do not count as listening. Clearing history installs an account barrier against late audio reports.

The media worker uses `youtubei.js` and a narrow SoundCloud public API adapter. YouTube transfers use validated byte ranges and expected-size checks. Playback files and download jobs remain separate. Runtime source/build tools are separate from generated bundles and caches.

Cloud accounts and the $1 Plus payment service remain a separate deployment stage. No local preference or imported entitlement field can grant paid access. macOS, Android and iOS require their own platform adapters and validation before being offered as supported builds.
