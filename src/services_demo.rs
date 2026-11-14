//! A row of buttons that exercises what the client needs from Android
//! beyond drawing (gpui_mobile's `android::services`): permissions, the
//! foreground service, a notification, a link, a file pick, a share in, and
//! a web view over the surface that talks back. Everything is logged under
//! `fancy-hello`.

use gpui::{App, Bounds, Context, Window, div, point, prelude::*, px, rgb, size};
use gpui_mobile::android::services::{connection, notifications, permissions, share_in, web_view};

/// The page the web view shows: local, from a string, with an https base.
const PAGE: &str = r#"<!doctype html><html><body style="margin:0;background:#2a6;color:#fff;font:20px sans-serif">
<p style="margin:12px">A page in the system WebView</p>
<button id="b" style="margin:12px;font-size:20px"
  onclick="window.fancyNative.postMessage('clicked at ' + location.origin)">Post a message</button>
</body></html>"#;

/// The demo's state: the web view, while shown.
pub struct ServicesDemo {
    web: Option<web_view::WebView>,
}

impl ServicesDemo {
    /// Makes the demo and starts listening for what Android reports.
    pub fn new(cx: &mut Context<Self>) -> Self {
        connection::on_disconnect(Box::new(|| {
            log::info!("fancy-hello service: Disconnect tapped, stopping");
            let _ = connection::stop();
        }));
        share_in::on_shared(Box::new(|shared| {
            let sizes = shared
                .files
                .iter()
                .map(|path| (path.display().to_string(), std::fs::metadata(path).map(|m| m.len()).ok()))
                .collect::<Vec<_>>();
            log::info!("fancy-hello shared in: files {sizes:?}, text {:?}", shared.text);
        }));
        web_view::on_message(Box::new(|id, message| {
            log::info!("fancy-hello web view {id} says {message:?}");
        }));
        let _ = cx;
        Self { web: None }
    }

    fn ask(permission: &'static str, cx: &mut App) {
        let answer = permissions::request(permission);
        cx.spawn(async move |_| {
            let granted = answer.await.unwrap_or(false);
            log::info!("fancy-hello permission {permission}: granted {granted}");
        })
        .detach();
    }

    fn notify(cx: &mut App) {
        let answer = permissions::request(permissions::NOTIFICATIONS);
        cx.spawn(async move |_| {
            if answer.await.unwrap_or(false) {
                let result = notifications::post(7, "Fancy Mumble", "A new message in #general");
                log::info!("fancy-hello notification posted: {result:?}");
            } else {
                log::info!("fancy-hello notification: not allowed");
            }
        })
        .detach();
    }

    fn toggle_service() {
        let result = if connection::is_running() {
            connection::stop()
        } else {
            connection::start("Fancy Mumble", "Connected to test server, #general")
        };
        log::info!(
            "fancy-hello service: {result:?}, running {}",
            connection::is_running()
        );
    }

    fn pick(cx: &mut App) {
        let picked = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: None,
        });
        cx.spawn(async move |_| {
            let picked = picked.await;
            log::info!("fancy-hello picked: {picked:?}");
        })
        .detach();
    }

    fn toggle_web(&mut self, window: &Window) {
        if self.web.take().is_some() {
            log::info!("fancy-hello web view closed");
            return;
        }
        let top = window.insets().safe_area.top + px(260.);
        let bounds = Bounds::new(point(px(24.), top), size(px(320.), px(180.)));
        match web_view::WebView::new(bounds, PAGE, "https://example.invalid/", false) {
            Ok(web) => {
                log::info!("fancy-hello web view {} shown", web.id());
                self.web = Some(web);
            }
            Err(error) => log::warn!("fancy-hello web view: {error}"),
        }
    }
}

fn button(id: &'static str, label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_2()
        .py_1()
        .rounded_md()
        .bg(rgb(0xbb9af7))
        .text_color(rgb(0x101018))
        .child(label)
}

impl Render for ServicesDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_wrap()
            .gap_1()
            .px_4()
            .text_size(px(14.))
            .child(button("mic", "Mic").on_click(|_, _, cx| Self::ask(permissions::MICROPHONE, cx)))
            .child(button("notify", "Notify").on_click(|_, _, cx| Self::notify(cx)))
            .child(button("service", "Service").on_click(|_, _, _| Self::toggle_service()))
            .child(button("link", "Link").on_click(|_, _, cx| {
                log::info!("fancy-hello opening a link");
                cx.open_url("http://127.0.0.1:9/fancy-link-test");
            }))
            .child(button("pick", "Pick").on_click(|_, _, cx| Self::pick(cx)))
            .child(button("web", "Web").on_click(cx.listener(|demo, _, window, _| {
                demo.toggle_web(window);
            })))
    }
}
