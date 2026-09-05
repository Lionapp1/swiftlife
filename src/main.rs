use std::{path::PathBuf, process::Command as ProcessCommand, sync::Arc};

use serde::Deserialize;
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};
use wry::{http::Request, NewWindowResponse, PageLoadEvent, Rect, WebView, WebViewBuilder};

const TOOLBAR_HEIGHT: u32 = 76;
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
    OpenDownloads,
}

enum AppEvent { Command(Command), UrlChanged(String), TitleChanged(String), Loading(bool) }

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
            Rect { position: LogicalPosition::new(0, 0).into(), size: LogicalSize::new(size.width, TOOLBAR_HEIGHT.min(size.height)).into() }
        } else {
            Rect { position: LogicalPosition::new(0, TOOLBAR_HEIGHT.min(size.height)).into(), size: LogicalSize::new(size.width, size.height.saturating_sub(TOOLBAR_HEIGHT)).into() }
        }
    }

    fn update_layout(&self) {
        let Some(window) = &self.window else { return };
        if let Some(toolbar) = &self.toolbar { let _ = toolbar.set_bounds(Self::bounds(window, true)); }
        if let Some(browser) = &self.browser { let _ = browser.set_bounds(Self::bounds(window, false)); }
    }

    fn send_to_toolbar(&self, script: &str) {
        if let Some(toolbar) = &self.toolbar { let _ = toolbar.evaluate_script(script); }
    }

    fn navigate(&self, input: &str) {
        let url = normalize_url(input);
        if let Some(browser) = &self.browser { let _ = browser.load_url(&url); }
    }

    fn open_downloads(&self) {
        let dir = dirs::download_dir().or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Downloads")));
        let Some(dir) = dir else { return };
        #[cfg(target_os = "windows")]
        { let _ = ProcessCommand::new("explorer").arg(&dir).spawn(); }
        #[cfg(target_os = "macos")]
        { let _ = ProcessCommand::new("open").arg(&dir).spawn(); }
        #[cfg(all(unix, not(target_os = "macos")))]
        { let _ = ProcessCommand::new("xdg-open").arg(&dir).spawn(); }
    }

    fn command(&mut self, command: Command) {
        match command {
            Command::Navigate { url } => self.navigate(&url),
            Command::Back => if let Some(browser) = &self.browser { let _ = browser.go_back(); },
            Command::Forward => if let Some(browser) = &self.browser { let _ = browser.go_forward(); },
            Command::Reload => if let Some(browser) = &self.browser { let _ = browser.reload(); },
            Command::Home => self.navigate(HOME_URL),
            Command::FocusAddress => self.send_to_toolbar("window.swiftlifeFocus();"),
            Command::OpenDevtools => if let Some(browser) = &self.browser { browser.open_devtools(); },
            Command::OpenDownloads => self.open_downloads(),
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
        let attrs = Window::default_attributes().with_title("SwiftLife — Türkçe Web Tarayıcısı").with_inner_size(LogicalSize::new(1440u32, 900u32)).with_min_inner_size(LogicalSize::new(900u32, 620u32));
        let window = Arc::new(event_loop.create_window(attrs).expect("SwiftLife penceresi oluşturulamadı"));

        let toolbar_html = include_str!("../ui/index.html");
        let toolbar_proxy = proxy.clone();
        let toolbar = WebViewBuilder::new()
            .with_id("swiftlife-toolbar")
            .with_bounds(Self::bounds(&window, true))
            .with_html(toolbar_html)
            .with_ipc_handler(move |request: Request<String>| {
                if let Ok(command) = serde_json::from_str::<Command>(&request.into_body()) { let _ = toolbar_proxy.send_event(AppEvent::Command(command)); }
            })
            .with_focused(true)
            .with_background_color((20, 22, 28, 255))
            .build_as_child(window.as_ref())
            .expect("SwiftLife araç çubuğu oluşturulamadı");

        let browser_proxy = proxy.clone();
        let browser_init = r#"
            (() => {
                const style=document.createElement('style');
                style.textContent=`#swiftlife-context-menu{position:fixed;z-index:2147483647;min-width:250px;padding:6px;border:1px solid rgba(255,255,255,.12);border-radius:14px;background:rgba(24,27,34,.98);box-shadow:0 20px 60px rgba(0,0,0,.45),0 2px 12px rgba(0,0,0,.25);backdrop-filter:blur(16px);font:13px -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;color:#f4f6fb;display:none;user-select:none}#swiftlife-context-menu.open{display:block}#swiftlife-context-menu button{width:100%;height:36px;border:0;border-radius:9px;background:transparent;color:inherit;text-align:left;padding:0 11px;display:flex;align-items:center;gap:10px;cursor:pointer}#swiftlife-context-menu button:hover{background:#303542}#swiftlife-context-menu .sl-ico{width:19px;text-align:center;color:#aeb7c8;font-size:15px}#swiftlife-context-menu .sl-sep{height:1px;background:#343945;margin:5px 4px}#swiftlife-context-menu .sl-muted{padding:7px 11px 5px;color:#7e8798;font-size:10px;letter-spacing:.08em;text-transform:uppercase}`;
                document.documentElement.appendChild(style);
                const menu=document.createElement('div');menu.id='swiftlife-context-menu';document.documentElement.appendChild(menu);
                const editable=el=>{if(!el)return false;const t=(el.tagName||'').toLowerCase();return t==='input'||t==='textarea'||t==='select'||el.isContentEditable};
                const closest=(el,s)=>el&&el.closest?el.closest(s):null;
                const text=()=>((window.getSelection?window.getSelection().toString():'')||'').trim();
                const abs=v=>{try{return new URL(v,location.href).href}catch(_){return ''}};
                const host=()=>{try{return location.hostname.toLowerCase()}catch(_){return ''}};
                const isX=()=>/(^|\.)x\.com$|(^|\.)twitter\.com$/.test(host());
                const copy=async v=>{if(!v)return;try{await navigator.clipboard.writeText(v);return}catch(_){}try{const a=document.createElement('textarea');a.value=v;a.style.position='fixed';a.style.opacity='0';document.body.appendChild(a);a.focus();a.select();document.execCommand('copy');a.remove()}catch(_){}};
                const download=v=>{if(!v)return;const a=document.createElement('a');a.href=v;a.download='';a.rel='noreferrer';a.style.display='none';document.body.appendChild(a);a.click();setTimeout(()=>a.remove(),1000)};
                const open=v=>{if(v)location.href=v}; const hide=()=>{menu.classList.remove('open')};
                const add=(icon,label,fn)=>{const b=document.createElement('button');b.type='button';b.innerHTML='<span class="sl-ico">'+icon+'</span><span>'+label+'</span>';b.onclick=e=>{e.preventDefault();e.stopPropagation();hide();fn()};menu.appendChild(b)};
                const sep=()=>{const d=document.createElement('div');d.className='sl-sep';menu.appendChild(d)};
                const build=info=>{menu.replaceChildren();if(info.kind==='link'){add('↗',isX()?'X bağlantısını aç':'Bağlantıyı aç',()=>open(info.url));add('⧉',isX()?'X bağlantısını kopyala':'Bağlantı adresini kopyala',()=>copy(info.url));add('↓','Bağlantıyı indir',()=>download(info.url))}else if(info.kind==='image'){add('◉','Görseli aç',()=>open(info.url));add('⧉','Görsel URL’sini kopyala',()=>copy(info.url));add('↓','Görseli indir',()=>download(info.url));if(info.link){sep();add('↗',isX()?'Gönderiyi aç':'Görsel bağlantısını aç',()=>open(info.link))}}else if(info.kind==='video'){add('▶','Videoyu aç',()=>open(info.url));add('⧉','Video URL’sini kopyala',()=>copy(info.url));add('↓','Video bağlantısını indir',()=>download(info.url))}else if(info.kind==='selection'){add('⧉','Seçimi kopyala',()=>copy(info.text));add('⌕','Seçimde ara',()=>open('https://www.google.com/search?q='+encodeURIComponent(info.text)))}else{add('↻','Sayfayı yenile',()=>location.reload());add('⌂','Ana sayfayı aç',()=>open('https://www.google.com'))}sep();const n=document.createElement('div');n.className='sl-muted';n.textContent=isX()?'SwiftLife • X araçları':'SwiftLife • Hızlı işlemler';menu.appendChild(n)};
                document.addEventListener('contextmenu',e=>{if(editable(e.target))return;const l=closest(e.target,'a[href]'),i=closest(e.target,'img[src],picture img'),v=closest(e.target,'video[src]');const lu=l?abs(l.href):'',iu=i?abs(i.currentSrc||i.src||i.getAttribute('src')):'',vu=v?abs(v.currentSrc||v.src||v.getAttribute('src')):'',st=text();let info;if(iu)info={kind:'image',url:iu,link:lu};else if(vu)info={kind:'video',url:vu};else if(lu)info={kind:'link',url:lu};else if(st)info={kind:'selection',text:st};else info={kind:'page'};e.preventDefault();build(info);const pad=8,mw=270,mh=Math.min(menu.scrollHeight||260,innerHeight-16);menu.style.left=Math.max(pad,Math.min(e.clientX,innerWidth-mw-pad))+'px';menu.style.top=Math.max(pad,Math.min(e.clientY,innerHeight-mh-pad))+'px';menu.classList.add('open')},true);
                document.addEventListener('mousedown',e=>{if(!menu.contains(e.target))hide()},true);document.addEventListener('keydown',e=>{if(e.key==='Escape')hide()},true);window.addEventListener('blur',hide);
                window.addEventListener('keydown',e=>{if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='l'){e.preventDefault();try{window.ipc.postMessage(JSON.stringify({action:'focus_address'}))}catch(_){}}},true);
            })();
        "#;

        let download_dir = dirs::download_dir().or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Downloads")));
        let download_dir = download_dir.map(|path| { let _=std::fs::create_dir_all(&path); path });
        let browser = WebViewBuilder::new()
            .with_id("swiftlife-browser")
            .with_bounds(Self::bounds(&window, false))
            .with_url(HOME_URL)
            .with_clipboard(true)
            .with_autoplay(true)
            .with_hotkeys_zoom(true)
            .with_back_forward_navigation_gestures(true)
            .with_devtools(true)
            .with_initialization_script(browser_init)
            .with_download_started_handler(move |_url, path| { if let Some(dir)=&download_dir { if let Some(name)=path.file_name(){*path=dir.join(name)} } path.is_absolute() })
            .with_navigation_handler({let proxy=browser_proxy.clone();move|url|{let _=proxy.send_event(AppEvent::UrlChanged(url.clone()));true}})
            .with_on_page_load_handler({let proxy=browser_proxy.clone();move|event,url|{let _=proxy.send_event(AppEvent::Loading(matches!(event,PageLoadEvent::Started)));let _=proxy.send_event(AppEvent::UrlChanged(url));}})
            .with_document_title_changed_handler({let proxy=browser_proxy.clone();move|title|{let _=proxy.send_event(AppEvent::TitleChanged(title));}})
            .with_new_window_req_handler({let proxy=browser_proxy.clone();move|url,_features|{let _=proxy.send_event(AppEvent::Command(Command::Navigate{url}));NewWindowResponse::Deny}})
            .build_as_child(window.as_ref())
            .expect("SwiftLife web görünümü oluşturulamadı");

        self.window=Some(window);self.toolbar=Some(toolbar);self.browser=Some(browser);self.update_layout();self.push_state();
    }

    fn user_event(&mut self,_:&ActiveEventLoop,event:AppEvent){match event{AppEvent::Command(c)=>self.command(c),AppEvent::UrlChanged(url)=>{let e=serde_json::to_string(&url).unwrap_or_else(|_|"\"\"".into());self.send_to_toolbar(&format!("window.swiftlifeUrl({e});"));self.push_state()},AppEvent::TitleChanged(title)=>{let e=serde_json::to_string(&title).unwrap_or_else(|_|"\"SwiftLife\"".into());self.send_to_toolbar(&format!("window.swiftlifeTitle({e});"))},AppEvent::Loading(x)=>{self.send_to_toolbar(if x{"window.swiftlifeLoading(true);"}else{"window.swiftlifeLoading(false);"});self.push_state()}}}

    fn window_event(&mut self,event_loop:&ActiveEventLoop,_:WindowId,event:WindowEvent){match event{WindowEvent::Resized(_)=>self.update_layout(),WindowEvent::CloseRequested=>event_loop.exit(),_=>{}}}

    #[cfg(target_os="linux")]
    fn about_to_wait(&mut self,_:&ActiveEventLoop){while gtk::events_pending(){gtk::main_iteration_do(false);}}
}

fn normalize_url(input:&str)->String{let value=input.trim();if value.is_empty(){return HOME_URL.to_string()}if value.eq_ignore_ascii_case("about:blank"){return value.to_string()}if value.starts_with("http://")||value.starts_with("https://")||value.starts_with("file://"){return value.to_string()}if value.contains(' ')||!value.contains('.') {return format!("https://www.google.com/search?q={}",urlencoding::encode(value))}format!("https://{value}")}

fn main()->Result<(),Box<dyn std::error::Error>>{let event_loop=EventLoop::<AppEvent>::with_user_event().build()?;let proxy=event_loop.create_proxy();let mut app=App::new(proxy);event_loop.run_app(&mut app)?;Ok(())}
