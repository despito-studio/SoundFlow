# Windows build

The current frontend is Flutter 3.47.7 / Dart 3.13.5. The Rust audio core builds as soundflow_lib.dll, without a separate Rust UI executable.

The checkout-local build uses:

- Flutter at output/toolchain/flutter.
- Rust MSVC 1.92 or newer.
- Microsoft C++ toolset and Windows SDK prepared by setup-msvc.ps1 and setup-windows-sdk.ps1.
- CMake and Ninja under output/toolchain/python-build.

Run scripts/build-flutter-windows.ps1. The script builds the Rust library, resolves Flutter packages, generates Flutter Windows configuration, then builds the standard Windows runner using Ninja and the local Microsoft compiler. The Flutter tool does not discover this portable toolset as a registered Visual Studio installation; its generated configuration is consumed by the native CMake build.

The output is output/flutter-preview/SoundFlow.exe with its required DLLs and data directories. Keep the whole folder together. -SkipCore reuses the current Rust DLL while iterating on the frontend.

Use `-BundlePath output/flutter-preview-hover` to build into another directory inside the checkout when an existing preview is running.

The application has a native Windows caption and resizable main window. Initial bounds fit the monitor work area. Rust handles the single-instance mutex, tray, media keys and Ctrl+Alt+M shortcuts. The Windows runner owns the Island window and forwards its commands to the main Flutter engine.

User data remains in %APPDATA%\app.soundflow.desktop. --data-dir selects an isolated profile; --resources selects a bundle root; --offline prevents cloud work. --export-design <png> exports one frame directly from the Flutter render tree for design review. It does not inject desktop input.

The Dock, settings and search have Flutter widget tests in flutter_app/test/redesign_test.dart, including responsive layout, gestures, queue occurrences and search interactions. Run `flutter test` from flutter_app using the configured SDK. Add `--dart-define=CAPTURE_UI=true` to export fixture-based Flutter frames for visual inspection under ignored output/flutter-design.

Flutter package notices are generated into data/flutter_assets/NOTICES.Z and accessible from About. Existing icon and media-runtime notices stay under resources/licenses. The Flutter app and installer embed Onest Regular and SemiBold under the SIL Open Font License; the installer's private DirectWrite collection does not install fonts into Windows. The pinned source, derived faces and provenance are in resources/fonts, with the license in resources/licenses/ONEST-OFL.txt. scripts/prepare-onest.py regenerates the static faces using fonttools 4.60.1.

## Installer and updates

`scripts/package-windows.ps1` builds the native installer and a portable ZIP. The terms
website is optional for now: with an empty `termsUrl` in `resources/installer/config.json`,
the download button starts installation directly, without a consent sheet or an acceptance
record. To add the website later, set that value or pass `-TermsUrl` with the real HTTPS page.
Configured terms open in the browser. `-RequireTermsUrl` is an explicit opt-in build check,
not the default. The installer does not bundle or invent legal text.

The installer UI has ready, progress and completion states. Its icons are exported from
the app's pinned AnimateIcons catalog through the same Flutter painter by
`flutter_app/tool/export_installer_icons_test.dart`; no system glyphs or hand-drawn icon
substitutes are used. The native C++ frontend uses the static C++ runtime, as does the
Flutter Windows runner, so a separate VC runtime installation is unnecessary.

The installer draws the complete window, including button visuals, into one physical-pixel
Direct2D surface. DirectWrite renders Onest directly at the monitor's scale. Native buttons
retain keyboard and accessibility behavior while sharing that surface, so parent repaints
cannot erase their fills or icons. A per-pixel alpha contour provides smooth rounded corners.
`flutter_app/lib/installer_design.dart` defines the shared surfaces and action groups.
The existing Flutter exporter executes `SoundFlowCorners` from `concentric.dart`, including
Flutter's radius normalization, and emits `installer_design.h` beside the icon assets.
The native renderer consumes that generated geometry and shared theme roles; there is no
second corner algorithm. The download and folder controls share the bottom action group,
and the larger cancel button sits inside the compact surface's trailing action group.
The same exporter captures vector strokes directly from the AnimateIcons painter after
its transforms and animated path trimming. `installer_vectors.h` contains those paths,
catalog motion frames and Anicry's easing/durations. Direct2D renders the strokes at the
actual monitor resolution, including the completion check; glyphs are not bitmap-scaled.
The surface, retained logo, title and action share Anicry's 360ms continuity curve.
Both renderers interpolate the geometry exported from InstallerDesign; they do not swap
or scale whole pages. The native HWND/backbuffer viewport stays fixed while the alpha
contour changes, and each animation tick presents one complete frame. Compact status
changes resolve only their labels, without hiding the logo/title or restarting the morph.
Progress eases between reports, and
completion/hover motion plays from the library. Windows reduced-motion settings use final
static geometry. The completion reveal finishes before the application takes focus.

Release versioning starts at **1.0.0**. Keep Cargo.toml and
flutter_app/pubspec.yaml in agreement. Never replace a published stable version.

Public GitHub Release assets are exactly:

- `SoundFlow-<version>-Setup.exe`: the native installer UI, embedding a private
  Velopack Setup. It works offline; silent backend installation is followed by
  our animated completion state and a single application launch.
- `SoundFlow-<version>-windows-x64.zip`: Velopack's managed portable layout.
  Extract the entire archive and use the root SoundFlow.exe launcher.

`dist/release/<version>` contains those two files. The former Download.exe,
Package.exe and update.json are not part of this release pipeline. NSIS is no
longer the installation/update backend. The generated private Velopack backend
stays under ignored output/velopack; it is embedded, never a third public asset.

The GitHub Pages channel is:
`https://despito-studio.github.io/SoundFlow/updates/win-x64/`.
It contains `releases.win.json` and generated full/delta `.nupkg` packages only.
Neither the public installer nor portable ZIP participates in applying updates.
Velopack compares semantic versions, verifies packages, prepares deltas with full
fallback, waits for a graceful application exit, applies and restarts silently.
Our cancellable HTTPS UpdateSource validates product/filenames/SHA-256/size and
uses bounded idle reads. Cancellation remains set through a delta fallback and
is checked again before handoff. Cached packages are verified against the feed
and their internal version even when the SDK would reuse a downloaded file.
Delta reconstruction creates a new ZIP, so its outer bytes/size can differ from
the published Full. After SDK verification, an atomic receipt binds its actual
local SHA-256/size to the unchanged remote target identity. Restore/apply verify
that receipt. Delta bases require either a prior verified receipt or a matching
canonical Full hash from the feed; otherwise the client requests a Full update.

The Windows runner calls sf_velopack_startup before COM/Flutter/profile locks.
Startup auto-apply is disabled: downloading/installing requires the user's Update
action. Stable installed and managed portable builds check once at startup and
every 12 hours while enabled. Development flat bundles do not auto-update.
The existing Onest/Anicry startup card owns progress, cancellation and completion.
Its library completion animation finishes before Flutter closes engine/audio.
User library/settings remain under `%APPDATA%\app.soundflow.desktop`.

Initial installation runs the framework against the final chosen directory so
its registry and shortcuts point at the correct root launcher. A prior owned
folder is retained as a sibling backup. Failure restores folder, uninstall values
and exact SoundFlow shortcut files. Cancellation is disabled before the backend
starts modifying installed files; the backend is never force-killed by a timer.
Per-run frontend logs are under `%LOCALAPPDATA%\SoundFlowInstaller\logs`.

Build-only tooling is installed locally by scripts/setup-velopack.ps1: pinned
.NET SDK 8.0.426 (Microsoft SHA-512) and vpk 1.2.161/net8.0. Users do not need .NET.
The Rust crate is pinned to the same Velopack version.

Automation: `.github/workflows/windows-release.yml` runs on stable version tags.
It tests/builds, restores the deployed Pages feed/packages, generates a new delta,
retains three full versions and their available deltas, deploys the service feed
atomically, then creates a Release with the two public downloads. Configure Pages
with source **GitHub Actions** before the first publication. The version tag must
match the application version. A plain source push does not publish binaries.
Workflow dispatch without Publish is a build-only verification. Publishing from
dispatch requires selecting the matching tag. If Pages succeeded but Release
creation failed, rerun the failed job; do not rebuild/overwrite the same version.
The Pages artifact currently owns the update host; a future website deployment
must include dist/pages/updates alongside its own content, not erase this path.
Local packaging never uploads or deploys anything.

For an already tested local build, `publish-prebuilt.yml` publishes the exact
verified artifacts without rebuilding. Prepare a private draft with the installer,
portable ZIP and `SoundFlow-<version>-update-channel.tar` containing `dist/pages`.
Set the draft's target commit to the exact source revision. Dispatch from that
revision with the channel archive SHA-256 and, when replacing a pre-product tag,
its expected old object SHA. The workflow validates the channel, deploys Pages,
checks its public HTTPS package, sets the tag, removes the private transfer
archive and publishes exactly two downloads. Tag changes use GITHUB_TOKEN so
they do not trigger a second build. Subsequent stable versions normally use the
tag-based release workflow. Published stable versions must remain immutable.

Local commands:

```powershell
./scripts/build-flutter-windows.ps1
./scripts/package-windows.ps1
# Later releases: restore packages for delta generation and bounded retention.
./scripts/package-windows.ps1 -PreviousFeed 'https://despito-studio.github.io/SoundFlow/updates/win-x64/'
# Service packages can also be built without either user download.
./scripts/package-windows.ps1 -SkipInstaller -SkipZip
./scripts/test-velopack.ps1
```

`test-velopack.ps1` packs a separate inert portable probe, checks 1.0.0 -> 1.0.1,
removes every installer/portable and the target Full file from its update source,
and verifies an actual delta, persisted cache receipt, silent apply/restart plus
an unchanged isolated data fixture. Full fallback cannot hide a broken delta in
this check. Everything stays under
ignored output/update-integration; it never opens the real application profile.
`cargo test --release update::tests` covers numeric version selection through the
actual SDK, downloads without Setup, integrity rejection, corrupt cache repair,
bounded transfers, cancellation and refusal to apply after cancellation.

`Setup.exe --self-test` checks private directory swaps, rollback and embedded
payload integrity without installing. `--verify-ui` checks actual rendering,
button/icon pixels at 100–200% DPI, shutdown acknowledgment and legacy waiting.
`--verify-motion <absolute-output-directory>` exercises queued UI commands,
real presentation/timers, interruptions and delayed frames, asserting stable
window bounds and continuous shared geometry. It stays offscreen and never
installs. Combine with `--dpi-scale` or `--reduced-motion` for those paths.
`--preview ready|terms|progress|waiting|complete|error --dpi-scale <factor>
--motion-ms <elapsed> --capture <absolute.png>` renders a noninteractive frame.
Flutter widget tests in test/update_prompt_test.dart cover startup deduplication,
one-click update, cancellation/retry, continuous shared elements and completion.
