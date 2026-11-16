# Reel

A tiny game clipper for Windows. It sits in your tray, notices when you're playing a game, and keeps the last few seconds in memory so you can press **F9** and save that play you just made.

No account, no launcher, no overlay, no feed. Just clips on your disk.

## Is this for you?

Reel is for you if:

- you want **instant replay clips** of your games,
- you don't want a **heavyweight app** like Medal.tv running all day just to do that,
- you're done with **arbitrary limits**: no watermark, no forced downscaling, no upload quota, no "premium" tier for quality. You choose the resolution, frame rate, codec and file size, and you get exactly that.

What it is **not**: a streaming tool, a video editor, or a social network. If you want those, OBS or Medal are better fits.

## How it works

- **It does nothing until you play.** Reel checks which window is in front about once a second. When a game is in front it starts buffering. When the game closes it stops. On the desktop, in your browser or in Discord, nothing is captured at all.
- **Clips stay in memory until you save.** The last N seconds live in RAM, already encoded. Nothing touches your disk until you press F9. Then the clip is written to `Videos\Reel\<Game>\`.
- **Your GPU does the work.** Capture, scaling and encoding run on your graphics card's video encoder, so the CPU barely notices. Measured on a Radeon RX 9070 XT: about 0.3% of one CPU core while capturing Valorant at 60 fps.
- **Game audio only.** Clips contain the game's sound, including in-game voice chat, but not Spotify, Discord calls or notification sounds.
- **Small.** The installer is under 1 MB. The tray app idles at around 200 KB of RAM. The recorder only exists while a game is running.

### Game detection

A window counts as a game if either:

1. Windows already knows it's a game: the same list Xbox Game Bar uses (Win+G), or
2. it's running fullscreen from a game library folder (Steam, Epic, Riot, GOG, Xbox, EA, Ubisoft, Battlestate).

So browsers and video players never trigger it, even in fullscreen.

### Anti-cheat

Reel never touches the game process. Video comes from Windows' own screen capture API (the same one OBS's Window Capture and Discord screen share use), and audio comes from the Windows audio engine. There's nothing injected and no driver. It's used with Valorant (Vanguard) without problems, but no tool can promise every anti-cheat will always be happy.

## Install

1. Download `reel-x.y.z-x64.msi` from [Releases](https://github.com/sandersonstabo/reel/releases/latest).
2. Open it. No admin rights needed: it installs just for you, into `%LOCALAPPDATA%\Programs\Reel`.
3. Windows SmartScreen may warn about an unrecognized app the first time, because the installer isn't code-signed yet. Click **More info → Run anyway**.

Reel starts right away and lives in your tray. To update, install a newer MSI over it. To remove it, use Settings → Apps.

There's also a portable `.exe` on each release if you don't want an installer.

## Use

| Key | What it does |
| --- | --- |
| **F9** | Save the last 30 seconds (or whatever you set) |
| **F8** | Turn clipping on or off |

Everything else is in the tray menu (right-click the dot):

| Setting | Options |
| --- | --- |
| Clip length | 15 s, 30 s, 1 min, 2 min |
| Size target | None, 10, 20, 30, 50, 100 MB |
| Resolution | Native, 2160p, 1440p, 1080p, 720p |
| Frame rate | 30, 60 fps |
| Codec | H.264, H.265 (HEVC), AV1 |
| Clip key | F6, F7, F9, F10, F11 |
| Start with Windows | on or off |

**Size target** is for sharing. Pick 10 MB and every clip fits under Discord's free upload limit: Reel lowers the bitrate, and the resolution if it has to, to make that happen. The menu shows what you'll get, e.g. "10 MB (720p60)". With **None** you get full quality at your native resolution.

**Codec:** H.264 plays everywhere. H.265 and AV1 give smaller files at the same quality but don't play in every app or browser. Codecs your GPU can't encode are greyed out.

The tray dot shows the state: a grey ring means off, a grey dot means waiting for a game, a red dot means buffering.

## Requirements

- Windows 11, or Windows 10 version 2004 or newer
- A GPU with a hardware video encoder: most AMD, NVIDIA and Intel GPUs from the last several years
- RAM for the buffer: about 100 MB for 30 s at 1440p60, less with a size target

So far it has been tested on Windows 11 with an AMD Radeon RX 9070 XT. Reports from NVIDIA and Intel users are very welcome.

## Privacy

Reel has no account, no telemetry and no network access. Clips, settings (`%APPDATA%\Reel\settings.ini`) and a small log (`%LOCALAPPDATA%\Reel\reel.log`) stay on your PC.

## Build from source

Requires Rust (stable) and the MSVC build tools on Windows.

```
cargo test --release
cargo build --release
```

The exe ends up in `target\release\reel.exe`. The installer is built by CI with [WiX](https://wixtoolset.org/) from `installer\reel.wxs`. Pushing a `v*` tag that matches the version in `Cargo.toml` publishes a release.

## License

[BSD 3-Clause](LICENSE)
