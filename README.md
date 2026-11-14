# android-client

Fancy Mumble's native client (`fancy-gpui`) as an Android app: a `cdylib`
whose `android_main` starts gpui_mobile's Android platform and runs the
client's own view on it, exactly as `main` does on a desktop
(`fancy_gpui::Launch`). The APK is `../android-host` built with
`-PnativeLibraryName=fancy_mumble_android -PapplicationId=com.fancymumble.gpui`.

## What it depends on, and what changes when the port lands

`fancy-gpui` is a binary crate in the project. A library entry point needs
two changes to it, made here in a copy (`../client-android-trial`, the port
`client-2-edit` at f71ddc0 plus the two patches) and kept as patches for
the port:

1. `F:\fancy-migrate\notes\android\client-lib-split.patch`: `src/main.rs`
   becomes `src/lib.rs` with a `pub struct Launch` (`begin`,
   `with_data_dir`, `assets`, `run`); the new `src/main.rs` is twelve
   lines; `paths::choose` takes the directory the platform gives
   (Android's files directory); `Cargo.toml` gets `[lib] name = "fancy_gpui"`.
   The window keeps no 720 by 480 least size on Android.
2. `F:\fancy-migrate\notes\android\screenshare-gate.patch`: `fancy-screenshare`
   and `gpui_platform` move under `[target.'cfg(not(target_os =
   "android"))'.dependencies]`; on Android `src/share/real.rs` is replaced by
   `src/share/real_absent.rs` (shares are heard of and announced, strokes
   travel; starting or watching one says it is not available yet), and
   `src/share/replay.rs`'s two encoder paths (demo measuring only) do
   nothing.

When the port carries both: point `fancy-gpui` in `Cargo.toml` at the
port (`../client-2-edit/src/ui/gpui`, later the project's
`vendor/client/src/ui/gpui`), copy its `Cargo.lock`, delete
`../client-android-trial`. Nothing else here changes.

A third change is the engine's (`crates/mumble-tauri`, shared with the web
client): `F:\fancy-migrate\notes\android\engine-log-try-init.patch`. Its
`logging::init` installed the tracing subscriber with `.init()`, which
panics when a `log` logger is set already, as gpui_mobile sets one for
logcat; with `.try_init()` the subscriber is installed and only the `log`
bridge is left out.

The APK needs the NDK's `libc++_shared.so` beside the library (the engine's
C++: oboe); `<scratchpad>\agent-android-backend\apk-client.ps1` packages
both.

## Checked

On the emulator: the client starts with the real engine on gpui_mobile and
shows its connect screen (`F:\fancy-migrate\notes\android\cl1-client-start.png`,
log `cl1-client-log.txt`), still in the desktop layout; 72.4 MB PSS
(`mem-client-start.txt`) against the web client's 330 to 440 MB.

`cargo check --target x86_64-linux-android` of the client library (with
the engine, `mumble-tauri` without its web layer) and of this package:
clean apart from dead-code warnings for what Android leaves unused (the
share peers' signalling, the player's page). The engine's Opus build needs
`CMAKE_POLICY_VERSION_MINIMUM=3.5` with this machine's CMake 4.

## Owed before it is the phone client

The phone layout (`F:\fancy-migrate\notes\PHONE-LAYOUT.md`), and in the
client: the Android services (`gpui_mobile::android::services`) wired where
the desktop has tray, notifications, the player and file saving; the
engine's audio on Android (oboe) needs `RECORD_AUDIO` asked for first
(`services::permissions::MICROPHONE`) and the connection service running
while connected.
