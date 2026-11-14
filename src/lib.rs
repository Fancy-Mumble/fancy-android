//! Fancy Mumble's native client on Android: the entry point the host's
//! NativeActivity calls, which starts gpui_mobile's platform and runs the
//! client's own view (`fancy_gpui::Launch`) on it, as `main` does on a
//! desktop.

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    use gpui::Application;
    use gpui_mobile::android::jni;

    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("fancy-mumble"),
    );
    jni::install_panic_hook();
    // The app's own files directory: identities, settings, history.
    let data_dir = app.internal_data_path();
    let _platform = jni::init_platform(&app);
    let Some(shared) = jni::shared_platform() else {
        log::error!("fancy-mumble: the Android platform did not start");
        return;
    };
    let mut launch = fancy_gpui::Launch::begin();
    if let Some(dir) = data_dir {
        launch = launch.with_data_dir(dir);
    }
    Application::with_platform(shared.into_rc())
        .with_assets(fancy_gpui::Launch::assets())
        .run(move |cx| launch.run(cx));
    log::info!("fancy-mumble: the application returned");
}
