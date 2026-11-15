# android-host

The Gradle project that packages a GPUI program built with gpui-next's
Android platform into an APK: a `NativeActivity` that loads the Rust
library named in `app/build.gradle`, and the Java the Rust side calls.

| Java (`app/src/main/java/dev/gpui/mobile/`) | What it is for |
| --- | --- |
| `GpuiInputActivity` | The activity (gpui-mobile's, extended): the input method proxy and keyboard configuration, window insets and the keyboard's animation, `onTrimMemory`, permission answers, shares into the app |
| `FancySystem` | Runtime permissions, message notifications, copying shared files into the cache |
| `FancyConnectionService` | The foreground service that keeps a voice connection alive, with its notification and Disconnect action |
| `GpuiPlatformView` | Native views over the GPUI surface, each in a sub-window at its rectangle; the web view's HTML loading, scripts and `postMessage` bridge |
| `GpuiPathPicker` | Opening files through the Storage Access Framework |
| `GpuiClipboard`, `GpuiCamera`, `GpuiVideoPlayer`, `GpuiHelper` | From gpui-mobile, as imported |

The manifest declares what the client needs: the microphone and audio
settings, Bluetooth headsets, the foreground service (`remoteMessaging` and
`microphone`), the wake lock, notifications, shares in (`SEND`,
`SEND_MULTIPLE`), every configuration change the activity handles itself
(rotation, size, density, keyboard, night mode) and `adjustResize`.

## Build

Offline, four workers, output on F: (`build.gradle` puts it in
`F:/fancy-build/android-host`):

```powershell
gradle --offline --max-workers=4 -p F:\fancy-migrate\android-host assembleDebug
# the test program (default)
#   app/src/main/jniLibs/x86_64/libfancy_android_hello.so
# the client
gradle ... -PnativeLibraryName=fancy_mumble_android -PapplicationId=com.fancymumble.gpui assembleDebug
#   app/src/main/jniLibs/x86_64/libfancy_mumble_android.so
```

The client's identifier is its own (`com.fancymumble.gpui`), never the web
client's `com.fancymumble.app`: installing one must not replace or read the
other's data.
