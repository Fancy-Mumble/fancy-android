# android-client

Fancy Mumble's native client (`fancy-gpui`) as an Android app: a `cdylib`
whose `android_main` starts gpui_mobile's Android platform and runs the
client's own view on it, exactly as `main` does on a desktop
(`fancy_gpui::Launch`). The APK is `../android-host` built with
`-PnativeLibraryName=fancy_mumble_android -PapplicationId=com.fancymumble.gpui`.

## What it depends on

Both from their repositories' `develop` branches, the commits in
`Cargo.lock`:

- `fancy-gpui` from [Fancy-Mumble/FancyMumble](https://github.com/Fancy-Mumble/FancyMumble)
  (`src/ui/gpui`), the library half of the desktop program. On Android it
  builds without screen sharing's capture, encoder and peers
  (`src/share/real_absent.rs` stands in: shares are heard of and announced,
  strokes travel, starting or watching one says it is not available yet),
  and the window keeps no least size. The engine (`src/ui/tauri`) installs
  its tracing subscriber with `try_init`, as gpui_mobile sets a `log`
  logger for logcat first.
- `gpui` and `gpui_mobile` from [Fancy-Mumble/gpui](https://github.com/Fancy-Mumble/gpui).

To build against local checkouts, override them in `.cargo/config.toml`:

```toml
[patch."https://github.com/Fancy-Mumble/FancyMumble"]
fancy-gpui = { path = "<FancyMumble>/src/ui/gpui" }

[patch."https://github.com/Fancy-Mumble/gpui"]
gpui = { path = "<gpui>/crates/gpui" }
gpui_mobile = { path = "<gpui>/crates/gpui_mobile" }
```

The APK needs the NDK's `libc++_shared.so` beside the library (the engine's
C++: oboe). The engine's Opus build needs `CMAKE_POLICY_VERSION_MINIMUM=3.5`
with CMake 4.

## Checked

- On the emulator: the client starts with the real engine on gpui_mobile and
  shows its connect screen, still in the desktop layout; 72.4 MB PSS against
  the web client's 330 to 440 MB.
- On a Pixel 7 (arm64, release): it starts and renders on the Mali-G710
  through Vulkan (wgpu chose it over GL).
- `cargo check --target aarch64-linux-android` of this package: clean.

## Owed before it is the phone client

The phone layout, and in the client: the Android services
(`gpui_mobile::android::services`) wired where the desktop has tray,
notifications, the player and file saving; the engine's audio on Android
(oboe) needs `RECORD_AUDIO` asked for first
(`services::permissions::MICROPHONE`) and the connection service running
while connected. On the Pixel 7 the title bar still sits under the status
bar.
