<p align="center">
  <img src="resources/icon.png" width="80" height="80" alt="SoundFlow">
</p>

<h1 align="center">SoundFlow</h1>

<p align="center">A music player for Windows by Despito Studio.</p>

<p align="center">
  <a href="https://github.com/despito-studio/SoundFlow/releases">Download</a> ·
  <a href="CHANGELOG.md">What's new</a> ·
  <a href="SUPPORT.md">Support</a>
</p>

SoundFlow brings music search, your library, and playback controls together in one calm interface. Find new tracks, build playlists, and keep the music close while you work.

## Features

- **Music search and discovery.** Search across YouTube and SoundCloud, support for Spotify track links, curated picks and artist pages.
- **Your own library.** Playlists, favorite tracks, and listening history.
- **Floating Dock.** A transparent panel with playback controls. Drag it up to open the current track and queue; drag it down or press Escape to collapse it.
- **Dynamic Island.** Compact controls on your desktop and quick access on top of other apps.
- **Personalization.** Themes, icon variants, audio settings, and reduced animations.
- **In-app updates.** New-version notifications and one-click installation with automatic restart.

## Download

Official builds are available in [GitHub Releases](https://github.com/despito-studio/SoundFlow/releases). Platform — **Windows x64**.

| File | How to use |
| --- | --- |
| `SoundFlow-<version>-Setup.exe` | Run the installer and follow the prompts. SoundFlow opens automatically after installation. |
| `SoundFlow-<version>-windows-x64.zip` | Extract the archive fully and run `SoundFlow.exe` from the root folder. |

Both the installed and portable versions support in-app updates. Current version — **1.1.1**.

In versions 1.0.0 and 1.1.0, the update check could hang. To move from them, run the current installer once, or download and extract the new portable ZIP. Your library and settings are preserved.

## Quick controls

| Shortcut | Action |
| --- | --- |
| `Ctrl+K` | Open search |
| `Ctrl+Alt+M` or `Ctrl+Shift+Space` | Show the compact Island over the current app |
| `Ctrl+Alt+Shift+M` | Open the Island with keyboard control |
| `Escape` | Collapse the expanded player or Island |

## Feedback

For bugs and suggestions, use [Issues](https://github.com/despito-studio/SoundFlow/issues). Before reaching out, check the [support rules](SUPPORT.md). Report vulnerabilities according to the [security policy](SECURITY.md).

The project's spaces follow the [code of conduct](CODE_OF_CONDUCT.md).

## For developers

The interface is built with Flutter; the audio core and library are in Rust. Build and publishing instructions are in the [Windows build](docs/WINDOWS.md), the component overview in the [architecture](docs/ARCHITECTURE.md), and the interface principles in the [design system](docs/design/SOUNDFLOW-REDESIGN.md).

Cloud features and SoundFlow Plus require a separately configured service; building the client yourself does not grant paid access.

## License

The source code is distributed under the [Apache License 2.0](LICENSE).

Third-party libraries, icons, and fonts have their own licenses and attributions in [resources/licenses](resources/licenses). Music artwork rights are described in [ARTWORK.md](resources/ARTWORK.md).
