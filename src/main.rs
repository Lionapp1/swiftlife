use std::sync::Arc;

use serde::Deserialize;
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};
use wry::{
    http::Request,
    NewWindowResponse,
    PageLoadEvent,
    Rect,
    WebView,
    WebViewBuilder,
};

const TOOLBAR_HEIGHT: u32 = 92;
const HOME_URL: &str = "https://www.google.com";

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum Command {
    Navigate { url: String },
    Back,
    Forward,
    Reload,
    Home,
    FocusAddress,
    OpenDevtools,
}

enum AppEvent {
    Command(Command),
    UrlChanged(String),
    TitleChanged(String),
    Loading(bool),
}

struct App {
    window: Option<Arc<Window>>,
    toolbar: Option<WebView>,
    browser: Option<WebView>,
    proxy: EventLoopProxy<AppEvent>,
}

impl App {
    fn new(proxy: EventLoopProxy<AppEvent>) -> Self {
        Self { window: None, toolbar: None, browser: None, proxy }
    }

    fn bounds(window: &Window, toolbar: bool) -> Rect {
        let size = window.inner_size().to_logical::<u32>(window.scale_factor());
        if toolbar {
            Rect {
                position: LogicalPosition::new(0, 0).into(),
                size: LogicalSize::new(size.width, TOOLBAR_HEIGHT.min(size.height)).into(),
            }
        } else {
            Rect {
                position: LogicalPosition::new(0, TOOLBAR_HEIGHT.min(size.height)).into(),
                size: LogicalSize::new(size.width, size.height.saturating_sub(TOOLBAR_HEIGHT)).into(),
            }
        }
    }

    fn update_layout(&self) {
        let Some(window) = &self.window else { return };
        if let Some(toolbar) = &self.toolbar {
            let _ = toolbar.set_bounds(Self::bounds(window, true));
        }
        if let Some(browser) = &self.browser {
            let _ = browser.set_bounds(Self::bounds(window, false));
        }
    }

    fn send_to_toolbar(&self, script: &str) {
        if let Some(toolbar) = &self.toolbar {
            let _ = toolbar.evaluate_script(script);
        }
    }

    fn navigate(&self, input: &str) {
        let url = normalize_url(input);
        if let Some(browser) = &self.browser {
            let _ = browser.load_url(&url);
        }
    }

    fn command(&mut self, command: Command) {
        match command {
            Command::Navigate { url } => self.navigate(&url),
            Command::Back => {
                if let Some(browser) = &self.browser { let _ = browser.go_back(); }
            }
            Command::Forward => {
                if let Some(browser) = &self.browser { let _ = browser.go_forward(); }
            }
            Command::Reload => {
                if let Some(browser) = &self.browser { let _ = browser.reload(); }
            }
            Command::Home => self.navigate(HOME_URL),
            Command::FocusAddress => self.send_to_toolbar("window.swiftlifeFocus();"),
            Command::OpenDevtools => {
                if let Some(browser) = &self.browser { browser.open_devtools(); }
            }
        }
    }

    fn push_state(&self) {
        if let Some(browser) = &self.browser {
            let back = browser.can_go_back().unwrap_or(false);
            let forward = browser.can_go_forward().unwrap_or(false);
            self.send_to_toolbar(&format!("window.swiftlifeState({back},{forward});"));
        }
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() { return; }

        #[cfg(target_os = "linux")]
        gtk::init().expect("GTK başlatılamadı. Linux'ta WebKitGTK bağımlılıklarını kurun.");

        let proxy = self.proxy.clone();
        let attrs = Window::default_attributes()
            .with_title("SwiftLife — Türkçe Web Tarayıcısı")
            .with_inner_size(LogicalSize::new(1440u32, 900u32))
            .with_min_inner_size(LogicalSize::new(900u32, 620u32));
        let window = Arc::new(event_loop.create_window(attrs).expect("SwiftLife penceresi oluşturulamadı"));

        let toolbar_html = include_str!("../ui/index.html");
        let toolbar_proxy = proxy.clone();
        let toolbar = WebViewBuilder::new()
            .with_id("swiftlife-toolbar")
            .with_bounds(Self::bounds(&window, true))
            .with_html(toolbar_html)
            .with_ipc_handler(move |request: Request<String>| {
                let body = request.into_body();
                if let Ok(command) = serde_json::from_str::<Command>(&body) {
                    let _ = toolbar_proxy.send_event(AppEvent::Command(command));
                }
            })
            .with_focused(true)
            .build_as_child(window.as_ref())
            .expect("SwiftLife araç çubuğu oluşturulamadı");

        let browser_proxy = proxy.clone();
        let browser_init = r#"
            (() => {
                window.addEventListener('keydown', (event) => {
                    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'l') {
                        event.preventDefault();
                        try { window.ipc.postMessage(JSON.stringify({action:'focus_address'})); } catch (_) {}
                    }
                }, true);
            })();
        "#;

        let browser = WebViewBuilder::new()
            .with_id("swiftlife-browser")
            .with_bounds(Self::bounds(&window, false))
            .with_url(HOME_URL)
            .with_clipboard(true)
            .with_autoplay(true)
            .with_hotkeys_zoom(true)
            .with_back_forward_navigation_gestures(true)
            .with_initialization_script(browser_init)
            .with_navigation_handler({
                let proxy = browser_proxy.clone();
                move |url| {
                    let _ = proxy.send_event(AppEvent::UrlChanged(url.clone()));
                    true
                }
            })
            .with_on_page_load_handler({
                let proxy = browser_proxy.clone();
                move |event, url| {
                    let _ = proxy.send_event(AppEvent::Loading(matches!(event, PageLoadEvent::Started)));
                    let _ = proxy.send_event(AppEvent::UrlChanged(url));
                }
            })
            .with_document_title_changed_handler({
                let proxy = browser_proxy.clone();
                move |title| { let _ = proxy.send_event(AppEvent::TitleChanged(title)); }
            })
            .with_new_window_req_handler({
                let proxy = browser_proxy.clone();
                move |url, _features| {
                    let _ = proxy.send_event(AppEvent::Command(Command::Navigate { url }));
                    NewWindowResponse::Deny
                }
            })
            .build_as_child(window.as_ref())
            .expect("SwiftLife web görünümü oluşturulamadı");

        self.window = Some(window);
        self.toolbar = Some(toolbar);
        self.browser = Some(browser);
        self.update_layout();
        self.push_state();
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::Command(command) => self.command(command),
            AppEvent::UrlChanged(url) => {
                let encoded = serde_json::to_string(&url).unwrap_or_else(|_| "\"\"".into());
                self.send_to_toolbar(&format!("window.swiftlifeUrl({encoded});"));
                self.push_state();
            }
            AppEvent::TitleChanged(title) => {
                let encoded = serde_json::to_string(&title).unwrap_or_else(|_| "\"SwiftLife\"".into());
                self.send_to_toolbar(&format!("window.swiftlifeTitle({encoded});"));
            }
            AppEvent::Loading(loading) => {
                self.send_to_toolbar(if loading { "window.swiftlifeLoading(true);" } else { "window.swiftlifeLoading(false);" });
                self.push_state();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::Resized(_) => self.update_layout(),
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
    }

    #[cfg(target_os = "linux")]
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        while gtk::events_pending() {
            gtk::main_iteration_do(false);
        }
    }
}

fn normalize_url(input: &str) -> String {
    let value = input.trim();
    if value.is_empty() { return HOME_URL.to_string(); }
    if value.eq_ignore_ascii_case("about:blank") { return value.to_string(); }
    if value.starts_with("http://") || value.starts_with("https://") || value.starts_with("file://") {
        return value.to_string();
    }
    if value.contains(' ') || !value.contains('.') {
        return format!("https://www.google.com/search?q={}", urlencoding::encode(value));
    }
    format!("https://{value}")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::<AppEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let mut app = App::new(proxy);
    event_loop.run_app(&mut app)?;
    Ok(())
}
