//! A small GPUI program for the Android platform: a heading, a counter
//! button, a text field, and a list of a hundred rows to scroll.
//!
//! It logs to logcat under the tag `fancy-hello`: each tap on the button,
//! each keystroke, and the scroll position, so a scripted run can be read.

mod input;

use gpui::{
    App, Context, Entity, ScrollHandle, Window, div, prelude::*, px, rgb,
};

/// The window's view.
pub struct Hello {
    taps: u32,
    field: Entity<input::InputExample>,
    scroll: ScrollHandle,
}

impl Hello {
    /// Makes the view, its field focused.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let field = input::new_example(window, cx);
        let keys = field.clone();
        cx.observe_keystrokes(move |_, event, _, cx| {
            log::info!("fancy-hello key {:?} -> {:?}", event.keystroke.key, event.keystroke.key_char);
            keys.update(cx, |field, cx| {
                field.note(event.keystroke.clone());
                cx.notify();
            });
        })
        .detach();
        Self {
            taps: 0,
            field,
            scroll: ScrollHandle::new(),
        }
    }
}

impl Render for Hello {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        log::info!(
            "fancy-hello frame: taps {}, text {:?}, scrolled {:?}",
            self.taps,
            self.field.read(cx).text(cx),
            self.scroll.offset().y
        );
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x1e1e2e))
            .text_color(rgb(0xe0e0e0))
            .child(
                div()
                    .p_4()
                    .text_xl()
                    .child("Hello from GPUI on Android"),
            )
            .child(
                div()
                    .id("counter")
                    .mx_4()
                    .p_3()
                    .rounded_md()
                    .bg(rgb(0x7aa2f7))
                    .text_color(rgb(0x101018))
                    .child(format!("Tapped {} times", self.taps))
                    .on_click(cx.listener(|hello, _, _, cx| {
                        hello.taps += 1;
                        log::info!("fancy-hello tap {}", hello.taps);
                        cx.notify();
                    })),
            )
            .child(div().m_4().h(px(160.)).child(self.field.clone()))
            .child(
                div()
                    .id("rows")
                    .flex_1()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .children((0..100).map(|i| {
                        div()
                            .px_4()
                            .py_3()
                            .border_b_1()
                            .border_color(rgb(0x313244))
                            .child(format!("Row {i}: the quick brown fox jumps over the lazy dog"))
                    })),
            )
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    use gpui::{Application, WindowOptions};
    use gpui_mobile::android::jni;

    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("fancy-hello"),
    );
    jni::install_panic_hook();
    let _platform = jni::init_platform(&app);
    let Some(shared) = jni::shared_platform() else {
        log::error!("fancy-hello: no platform");
        return;
    };
    Application::with_platform(shared.into_rc()).run(|cx: &mut App| {
        let window = cx
            .open_window(WindowOptions::default(), |window, cx| {
                cx.new(|cx| Hello::new(window, cx))
            })
            .expect("a window");
        let _ = window.update(cx, |hello, window, cx| {
            let focus = hello.field.read(cx).text_input_focus(cx);
            window.focus(&focus, cx);
        });
    });
    log::info!("fancy-hello: application returned");
}
