# tunerift

[English](README.en.md) | [简体中文](README.md)

<p align="center">
  <img src="docs/assets/tunerift-banner.png" alt="Tunerift — A free, open-source terminal radio. Tune into the world." width="100%">
</p>
<p align="center"><sub>Promotional concept artwork. Interface details may differ from the actual application.</sub></p>

**A terminal radio. Tune into the world.**

A cross-platform terminal radio built with Rust. Supports mouse controls, multiple station sources, proxies, session persistence, and a one-key minimal mode. Free and open source, with no account, API key, Python, or external player required.

This is an early development release. Stations are provided by third parties; availability, regional restrictions, and terms of use depend on each provider.

## Download

Download the archive for your operating system and CPU architecture from [GitHub Releases](https://github.com/Nyakooo/tunerift/releases/latest): `.zip` for Windows, `.tar.gz` for Linux and macOS. Extract and run. Packages include documentation, the license, and release notes. Each release provides a `SHA256SUMS` file for verifying downloads.

### One-command install / update

Linux / macOS:

```sh
curl -fsSL https://raw.githubusercontent.com/Nyakooo/tunerift/master/install.sh | sh
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/Nyakooo/tunerift/master/install.ps1 | iex
```

The installer detects your operating system and x86_64 / ARM64 architecture, downloads the latest stable GitHub Release, and verifies its SHA256 checksum before installation. Run the same command again to update. No Rust, Python, or administrator privileges are required. A published Release containing your platform's package must already exist; the installer never downloads intermediate Actions artifacts or compiles locally.

Default locations: `~/.local/bin/tunerift` on Linux / macOS, and `%LOCALAPPDATA%\Tunerift\bin\tunerift.exe` on Windows. The installer configures the user PATH. If the Unix installation directory was not already on PATH, open a new terminal before running `tunerift`, or use the full path printed by the installer. Your configuration and favorites are preserved.

Linux / macOS requires `curl`, `tar`, and either `sha256sum` or `shasum`. Linux still needs the system ALSA runtime library.

To select a version or installation directory (add custom directories to PATH yourself):

```sh
curl -fsSL https://raw.githubusercontent.com/Nyakooo/tunerift/master/install.sh | TUNERIFT_VERSION=v0.1.0 TUNERIFT_INSTALL_DIR="$HOME/.local/bin" sh
```

On Windows, set `$env:TUNERIFT_VERSION` and `$env:TUNERIFT_INSTALL_DIR` before running the installer. Set `TUNERIFT_NO_MODIFY_PATH=1` to disable PATH configuration changes.

## Usage

Run `tunerift` (`tunerift.exe` on Windows) for your platform. The application creates `config.toml` on first launch. It starts in minimal mode; switch to a full layout at any time without interrupting playback.

```sh
./tunerift
./tunerift --config-path
./tunerift --portable
./tunerift --config /path/to/config.toml
./tunerift --check
```

`--check` checks the configuration and audio device. `--portable` stores the configuration beside the executable.

Desktop targets: Windows, Linux, and macOS on x86_64 and ARM64. Local runtime verification has been limited to Linux x86_64; other targets are built and tested natively by CI and still need runtime verification on actual machines. Phones, Termux, Raspberry Pi-specific support, and ARMv7 are outside the current scope.

## Controls

| Action | Mouse | Keyboard |
| --- | --- | --- |
| Select a station | Click | ↑ / ↓ or j / k |
| Play a station | Double-click or click Play | Enter |
| Play / Stop | Click playback control | Space |
| Search | Click Search | / |
| Settings | Click Settings | p or comma |
| Minimal mode / Restore full layout | Click Minimal / Full | m |
| Switch station source | Source list in full layouts; source settings in minimal mode | s |
| Switch stream route | Click Route | r |
| Adjust volume | Click or drag the volume bar | + / - |
| Add / Remove favorite | Click Favorite; removal requires confirmation | f |
| Delete a favorite or custom entry | Click Delete and confirm | d / Delete |
| Add a station | Settings → Custom stations → Add | n |
| Force an online directory refresh | Click Refresh | u |
| Shortcut help | Click Help | ? / F1 |
| Quit | — | q / Ctrl+C |

Use a terminal with UTF-8, true color, and mouse support. Holding Shift usually bypasses mouse capture for text selection, depending on your terminal. Small windows temporarily use a simplified layout without changing your saved mode.

## Layouts and themes

Five full layouts share the same playback engine: Airwave, Night Receiver, Operations, Tracker, and btop. Settings lets you select the layout and a Mint, Amber, Violet, Ocean, or Mono theme independently.

A compact stereo level meter is used by default. Enable the live FFT spectrum in Settings to visualize decoded audio. Playback pauses and buffering do not produce fake waveforms.

## Station sources and stream routes

Built-in sources include [Radio Browser](https://www.radio-browser.info/) and [SomaFM](https://somafm.com/listen/), alongside favorites, custom stations, and M3U/PLS subscriptions. SomaFM streams are intended for personal listening; follow its [usage guidance](https://somafm.com/live/directstreamlinks.html).

Add an online M3U/PLS URL or a full local file path in Settings. Subscriptions and custom stations can be edited and deleted there. An HLS `.m3u8` URL is an individual live stream: use **Add station**, rather than importing it as a station directory. You can also add a backup stream URL when creating a station.

Directory sources and stream routes are separate choices. Radio Browser mirrors belong to a single directory source. Switching sources does not stop the current station, and connection failures do not automatically switch sources. Press Enter to retry, `r` to change route, or `s` to change source.

## Proxy

Open Settings → Proxy and enter a complete URL. Leave it empty for a direct connection. Examples:

```text
http://127.0.0.1:7890
socks5h://127.0.0.1:1080
http://username:password@127.0.0.1:7890
```

Search, live streams, playlists, redirects, and HLS segments all use this setting. Direct mode does not inherit system proxy environment variables. `socks5h` resolves hostnames through the proxy. Changing the proxy reconnects the current stream.

Proxy settings and the input field display the full URL, including the username and password. Credentials are stored unencrypted in the local configuration; files created on Unix use permissions `0600`. Do not commit personal configurations to Git.

## Session and configuration

Volume, layout, theme, minimal mode, station source, favorites, subscriptions, proxy, and the last station and route are saved automatically. By default, launch restores the station and settings without starting playback. Toggle **Startup** directly in the main Settings menu to enable playback on the next launch. The legacy `autoplay` configuration field is ignored; the new option defaults to off.

Run `tunerift --config-path` to see the actual configuration location:

- Windows: `%APPDATA%\tunerift\config\config.toml`
- Linux: `$XDG_CONFIG_HOME/tunerift/config.toml`, usually `~/.config/tunerift/config.toml`
- macOS: `~/Library/Application Support/tunerift/config.toml`

An invalid configuration produces an error and leaves the original file intact. You can edit subscriptions, favorites, and stream URLs in TOML; avoid editing the file while the application is running.

## Directory cache

Online station directories are cached for one hour. Search results are cached separately by source and query for ten minutes. Cached results appear first on launch, source changes, and repeated searches. Expired entries refresh in the background; failed refreshes retain the cached list. The header shows the data source and update time.

Press `u` or click Refresh to force an online update regardless of cache age. Favorites, custom stations, and local subscriptions are read directly and bypass the disk directory cache. Live audio is not cached to disk; playback still requests the live stream and its playlists.

The cache uses the system cache directory, with a separate namespace per configuration, limited to 10 MB and 128 query entries. Older entries are removed when those limits are exceeded. Settings → Clear directory and search cache requires confirmation. Clearing it preserves favorites, configuration, and current playback; pending old requests cannot write the cleared cache back.

```sh
./tunerift --cache-path
```

## Interface language

English is the default. English and Simplified Chinese are supported. Open Settings (`p`), click Language, or press `l` while in Settings to switch immediately. Your choice is saved for the next launch. Existing configurations without a language setting also default to English. Station names, subscription names, and search input remain unchanged.

You can also set the language in the configuration file:

```toml
language = "english" # or "chinese"
```

## Menu shortcuts

- Settings: `↑↓` to select, `Enter` to change, `1…9` / `0` for direct access, and `l` to switch language. Startup playback toggles directly in this menu.
- Source and custom station managers: `↑↓` to select, `a` to add, `e` to edit, and `d` to delete. `Enter` switches the selected source or edits the selected custom station.
- Forms: `Tab` / `Shift+Tab` to change fields, `Ctrl+u` to clear the current field, `Enter` to apply, and `Esc` to return.
- Delete confirmation: `y` to confirm, `n` / `Esc` to cancel. Cancel is selected by default.
- `Esc` or Back returns to the parent menu and preserves its selection. Returning from `F1` help also preserves form input.

## Playback support

Audio decoding and system audio output are built in; Tunerift does not invoke mpv or FFmpeg. Supported decoder formats include MP3, AAC-LC, FLAC, Vorbis, and WAV, along with common unencrypted HLS streams using MPEG audio/AAC in MPEG-TS or direct audio segments.

Encrypted HLS, byte-range segments, some fragmented MP4 streams, and HE-AAC/SBR require further compatibility work or verification. Switch routes when a stream is unsupported. Linux uses system audio facilities through ALSA, often with PipeWire or PulseAudio providing the default output. The executable is not fully static and depends on system libraries.

## Build from source

Install stable Rust. On Linux, install the audio build dependencies as well:

```sh
# Debian / Ubuntu
sudo apt-get install libasound2-dev pkg-config
cargo build --release --locked
cargo test --locked
cargo clippy --all-targets -- -D warnings
```

Executables are written to `target/release/`. End users do not need the Rust toolchain. CI builds a separate executable for each of the six desktop targets; a single binary does not run across operating systems.

## License

[MIT](LICENSE). Radio programs and third-party data remain the property of their respective rights holders.

## Automated builds and releases

[GitHub Actions](https://github.com/Nyakooo/tunerift/actions) handles builds, checks, tests, and packaging. Local builds are not required for releases.

- Branch pushes and pull requests run formatting checks, Clippy, and tests, then build natively for Windows, Linux, and macOS on x86_64 and ARM64. Six packages are uploaded as Actions artifacts.
- To release a version, update the project version in `Cargo.toml` and `Cargo.lock`, update `RELEASE_NOTES.md`, merge into `master`, and push a matching `v*` tag, such as `v0.1.1`.
- Tags trigger all six platform builds. Once every build succeeds, Actions creates a GitHub Release and uploads the `.zip` / `.tar.gz` archives and `SHA256SUMS`. The tag must match the Cargo version. Tags such as `v0.2.0-beta.1` are marked as prereleases automatically.
- Rerunning the workflow for an existing tag updates its assets instead of creating a duplicate release. You can also manually run the workflow against an existing tag from Actions.

Python is used only for packaging in Actions. It is not distributed with the application, and users do not need Python to run Tunerift.
