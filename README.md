# android-hello

A small GPUI program for trying the Android platform of
[Fancy-Mumble/gpui](https://github.com/Fancy-Mumble/gpui) (`crates/gpui_mobile`,
taken from its `develop` branch; the commit is in `Cargo.lock`) on a device or
the emulator, laid out as a messaging app is:

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

With the NDK's clang set for the Android target (`CC_<target>`,
`CARGO_TARGET_<TARGET>_LINKER`):

```powershell
cargo build --target x86_64-linux-android            # the emulator's ABI
cargo build --target aarch64-linux-android           # a phone
# then the host: ../android-host (the .so into jniLibs, then Gradle)
```

To build against a local checkout of the GPUI fork, override it in
`.cargo/config.toml` with `[patch."https://github.com/Fancy-Mumble/gpui"]`.
