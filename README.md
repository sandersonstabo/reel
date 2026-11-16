# Reel: a lightweight game clipper for Windows

**Reel is a free, open-source game clipping app for Windows.** It runs quietly in your tray, starts buffering when you play a game, and saves the last moments of gameplay as a clip when you press a key. It's a lightweight alternative to Medal.tv, NVIDIA ShadowPlay and Xbox Game Bar for anyone who just wants instant replay clips.

[Download the latest release](https://github.com/sandersonstabo/reel/releases/latest)

## Who it's for

Reel is for you if:

- you want **instant replay clips** of your games, saved with one key,
- you don't want a **resource-heavy app** running all day just to do that,
- you're tired of **arbitrary limits** like watermarks, forced downscaling, upload quotas or paid tiers for quality.

With Reel you choose the quality, and your clips stay on your PC.

## Features

- **Lightweight.** A small installer, near-zero memory when idle, and encoding on your graphics card, so it stays out of your game's way.
- **Records only when you play.** Reel stays idle until it detects a game, and stops when you close it.
- **Nothing hits your disk until you clip.** Recent gameplay stays in memory. Press the clip key and the moment is saved to your Videos folder.
- **Your quality, your rules.** Native resolution and frame rate by default, with modern codecs and an optional file-size target for easy sharing.
- **No watermark, no account, no limits.** No sign-up, no feed and no upload quotas.
- **Private by design.** Fully local, no telemetry and no internet connection required.
- **Configured from the tray.** No windows or launchers. Right-click the tray icon and everything is there.
- **Open source.** BSD 3-Clause licensed, free forever.

## Install

1. Download the `.msi` installer from [Releases](https://github.com/sandersonstabo/reel/releases/latest).
2. Open it. No admin rights are needed.
3. If Windows SmartScreen warns about an unrecognized app, choose **More info → Run anyway**. The installer isn't code-signed yet.

Reel starts right away and lives in your system tray. Updates install over the old version. Uninstall it from Settings → Apps. A portable `.exe` is also available on each release.

## Use

- Play a game. Reel starts buffering automatically.
- Press **F9** to save a clip.
- Press **F8** to turn clipping on or off.
- Right-click the tray icon for settings: clip length, quality, file-size target, codec and hotkey.

## FAQ

### What is Reel?
Reel is a free, open-source instant replay and game clipping tool for Windows. It keeps recent gameplay in memory and saves it as an MP4 clip when you press a hotkey.

### Is Reel a good Medal.tv alternative?
Yes, if what you want from Medal is clipping. Reel focuses only on capturing clips locally, with low resource usage, no account and no watermark. It has no social feed or cloud sharing.

### Does Reel record all the time?
No. Reel only buffers while a game is running, and it only writes a file when you save a clip.

### Does Reel slow down my games?
Reel is designed to have minimal impact. Capture and encoding use your graphics card's built-in video encoder rather than your CPU, and the app does nothing while you're not gaming.

### Does Reel add a watermark or limit clip quality?
No. Clips are saved at the quality you choose, up to your native resolution, with no watermark.

### Can I make clips small enough for Discord?
Yes. Set a file-size target in the tray menu and Reel keeps every clip under it.

### Is Reel safe to use with anti-cheat games like Valorant?
Reel uses Windows' own capture features and doesn't modify or inject into games. It has been used with Valorant without problems, but no third-party tool can guarantee compatibility with every anti-cheat system.

### Is Reel free?
Yes. Reel is free and open source under the BSD 3-Clause license.

### Which Windows versions are supported?
Windows 11, and Windows 10 version 2004 or newer, with a graphics card that has a hardware video encoder (most AMD, NVIDIA and Intel GPUs from recent years).

## Build from source

Requires Rust (stable) and the MSVC build tools on Windows.

```
cargo test --release
cargo build --release
```

CI builds the installer and publishes releases when a version tag is pushed.

## Contributing

Bug reports, hardware compatibility reports (especially NVIDIA and Intel) and pull requests are welcome in [Issues](https://github.com/sandersonstabo/reel/issues).

## License

[BSD 3-Clause](LICENSE)
