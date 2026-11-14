# android-hello

A small GPUI program for trying gpui-next's Android platform
(`../gpui-next/crates/gpui_mobile`) on a device or the emulator, laid out
as a messaging app is:

- a header with colour emoji, a counter button and a Panel button;
- a row of buttons for the Android services the client needs (`src/services_demo.rs`):
  microphone and notification permissions, the connection service, a
  message notification, a link, a file pick, a web view over the surface;
- a hundred rows to scroll and fling; a tap clicks a row, a long press opens
  its menu (and copies "Row n" to the clipboard);
- a composer at the bottom (`src/input.rs`, Zed's input example grown
  multi-line): suggestions, autocorrect, a Send key or line breaks, caret
  placement by tap, paste on long press;
- the window's insets applied as padding (status bar, navigation bar,
  keyboard), back closing the menu, then the panel, then leaving the app,
  and the counter and draft kept across process death.

Everything is logged under the logcat tag `fancy-hello`.

## Build and run

Rules of this machine: build output on F:, six jobs, no debug info.

```powershell
. <scratchpad>\agent-gpui-android\android-env.ps1    # NDK, CMake, Java, F: target dir
cargo build --target x86_64-linux-android            # the emulator's ABI
# then the host: ../android-host (copies the .so into jniLibs, Gradle offline)
```

`<scratchpad>\agent-android-backend\apk.ps1` does both and installs on
`emulator-5554`; `emu.ps1 start|stop` runs the `fancy-gpui-next` emulator
headless without audio; `ui.ps1` drives it (taps, swipes, text, screenshots
to `F:\fancy-migrate\notes\android\`, logcat).
