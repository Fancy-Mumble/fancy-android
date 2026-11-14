//! A small GPUI program for the Android platform, laid out as a messaging
//! app is: a header with a counter button and a panel button, a list of a
//! hundred rows to scroll, fling and long-press, and a text field at the
//! bottom that the soft keyboard must not cover.
//!
//! It logs to logcat under the tag `fancy-hello` so a scripted run can be
//! read: taps, row clicks and secondary clicks, keystrokes, the panel, the
//! window's insets and the scroll position.

mod input;

use gpui::{
    App, Context, Entity, KeyBinding, MouseButton, ScrollHandle, Window, actions, div, prelude::*,
    px, rgb,
};

actions!(hello, [Back]);

/// The window's view.
pub struct Hello {
    taps: u32,
    field: Entity<input::InputExample>,
    scroll: ScrollHandle,
    /// The row whose menu a secondary click opened.
    menu_row: Option<usize>,
    /// Whether the panel the back button closes is open.
    panel_open: bool,
    /// The insets last logged, to log only changes.
    logged_insets: Option<gpui::Edges<gpui::Pixels>>,
    /// The root's focus, so keys (back is Escape) reach it with no field focused.
    focus: gpui::FocusHandle,
}

impl Hello {
    /// Makes the view.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.bind_keys([KeyBinding::new("escape", Back, None)]);
        let field = input::new_example(window, cx);
        let keys = field.clone();
        cx.observe_keystrokes(move |_, event, _, cx| {
            log::info!(
                "fancy-hello key {:?} -> {:?}",
                event.keystroke.key,
                event.keystroke.key_char
            );
            keys.update(cx, |field, cx| {
                field.note(event.keystroke.clone());
                cx.notify();
            });
        })
        .detach();
        cx.on_app_lifecycle(|phase, _| log::info!("fancy-hello lifecycle {phase:?}"))
            .detach();
        cx.on_memory_warning(|_| log::info!("fancy-hello memory warning"))
            .detach();
        // What the system gave back after it killed the process: the
        // counter and the draft (see `save`).
        let (taps, draft) = restored().unwrap_or_default();
        if !draft.is_empty() || taps > 0 {
            log::info!("fancy-hello restored: taps {taps}, draft {draft:?}");
            field.update(cx, |field, cx| field.set_text(&draft, cx));
        }
        Self {
            taps,
            field,
            scroll: ScrollHandle::new(),
            menu_row: None,
            panel_open: false,
            logged_insets: None,
            focus: cx.focus_handle(),
        }
    }

    /// Back closes the menu, then the panel; with neither open it is left
    /// unhandled, and the platform leaves the app.
    fn back(&mut self, _: &Back, _window: &mut Window, cx: &mut Context<Self>) {
        if self.menu_row.take().is_some() {
            log::info!("fancy-hello back: closed the menu");
        } else if self.panel_open {
            self.panel_open = false;
            log::info!("fancy-hello back: closed the panel");
        } else {
            log::info!("fancy-hello back: nothing to close");
            cx.propagate();
            return;
        }
        cx.notify();
    }

    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .child(div().text_xl().child("Hello from GPUI on Android"))
            .child("Colour emoji: \u{1F600} \u{1F389} \u{1F44D}\u{1F3FD} \u{2764}\u{FE0F}")
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .id("counter")
                            .flex_1()
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
                    .child(
                        div()
                            .id("panel-button")
                            .p_3()
                            .rounded_md()
                            .bg(rgb(0x9ece6a))
                            .text_color(rgb(0x101018))
                            .child("Panel")
                            .on_click(cx.listener(|hello, _, _, cx| {
                                hello.panel_open = true;
                                log::info!("fancy-hello panel opened");
                                cx.notify();
                            })),
                    ),
            )
    }

    fn rows(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("rows")
            .flex_1()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .children((0..100).map(|i| {
                let menu = (self.menu_row == Some(i)).then(|| {
                    div()
                        .p_2()
                        .bg(rgb(0x414868))
                        .child("Row menu: Copy / Reply / Delete")
                });
                div()
                    .id(("row", i))
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(rgb(0x313244))
                    .child(format!(
                        "Row {i}: the quick brown fox jumps over the lazy dog"
                    ))
                    .children(menu)
                    .on_click(cx.listener(move |_, event: &gpui::ClickEvent, _, _| {
                        log::info!(
                            "fancy-hello row {i} click (secondary {})",
                            event.is_secondary()
                        );
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |hello, _, _, cx| {
                            log::info!("fancy-hello row {i} secondary click, copied");
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(format!("Row {i}")));
                            hello.menu_row = Some(i);
                            cx.notify();
                        }),
                    )
            }))
    }

    fn panel(&self) -> Option<impl IntoElement> {
        self.panel_open.then(|| {
            div()
                .absolute()
                .top(px(120.))
                .left(px(24.))
                .right(px(24.))
                .p_4()
                .rounded_md()
                .bg(rgb(0xe0af68))
                .text_color(rgb(0x101018))
                .child("A panel: the back button closes it before it leaves the app")
        })
    }

    fn log_insets(&mut self, window: &Window) -> gpui::Edges<gpui::Pixels> {
        let insets = window.insets();
        let effective = insets.effective();
        if self.logged_insets != Some(effective) {
            log::info!(
                "fancy-hello insets: safe {:?} ime {:?}",
                insets.safe_area,
                insets.ime
            );
            self.logged_insets = Some(effective);
        }
        effective
    }
}

impl Render for Hello {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        log::info!(
            "fancy-hello frame: taps {}, text {:?}, scrolled {:?}",
            self.taps,
            self.field.read(cx).text(cx),
            self.scroll.offset().y
        );
        let insets = self.log_insets(window);
        save(self.taps, &self.field.read(cx).text(cx));
        div()
            .key_context("Hello")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::back))
            .size_full()
            .flex()
            .flex_col()
            .pt(insets.top)
            .pb(insets.bottom)
            .pl(insets.left)
            .pr(insets.right)
            .bg(rgb(0x1e1e2e))
            .text_color(rgb(0xe0e0e0))
            .child(self.header(cx))
            .child(self.rows(cx))
            .child(div().p_2().child(self.field.clone()))
            .children(self.panel())
    }
}

/// Keeps the counter and the draft where the system finds them if it kills
/// the process in the background.
fn save(taps: u32, draft: &str) {
    #[cfg(target_os = "android")]
    gpui_mobile::android::lifecycle::set_saved_state(format!("{taps}\n{draft}").into_bytes());
    #[cfg(not(target_os = "android"))]
    let _ = (taps, draft);
}

/// The counter and draft [`save`] kept, if the process was recreated.
fn restored() -> Option<(u32, String)> {
    #[cfg(target_os = "android")]
    {
        let bytes = gpui_mobile::android::lifecycle::restored_state()?;
        let text = String::from_utf8(bytes).ok()?;
        let (taps, draft) = text.split_once('\n')?;
        Some((taps.parse().ok()?, draft.to_string()))
    }
    #[cfg(not(target_os = "android"))]
    None
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
        cx.open_window(WindowOptions::default(), |window, cx| {
            cx.new(|cx| Hello::new(window, cx))
        })
        .and_then(|window| {
            window.update(cx, |hello, window, cx| window.focus(&hello.focus, cx))
        })
        .expect("a window");
    });
    log::info!("fancy-hello: application returned");
}
