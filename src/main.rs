mod security;

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    process::Command as ProcessCommand,
    sync::{mpsc, mpsc::Sender, Arc},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};
use wry::{NewWindowResponse, PageLoadEvent, Rect, WebContext, WebView, WebViewBuilder};

const HOME_URL: &str = "https://www.google.com";

#[cfg(target_os = "windows")]
const CHROME_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";
#[cfg(target_os = "linux")]
const CHROME_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";
#[cfg(target_os = "macos")]
const CHROME_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum Command { Navigate { url: String }, Back, Forward, Reload, Home, FocusAddress, OpenDownloads, OpenHistory }
enum AppEvent { Command(Command), UrlChanged(String), TitleChanged(String), Loading(bool) }

struct App { window: Option<Arc<Window>>, browser: Option<WebView>, web_context: Option<WebContext>, proxy: EventLoopProxy<AppEvent>, last_history_url: String, last_ui_url: String, history_tx: Sender<HistoryEntry> }

struct HistoryEntry { url: String, visited_at: u64 }

fn spawn_history_writer() -> Sender<HistoryEntry> {
    let (tx, rx) = mpsc::channel::<HistoryEntry>();
    thread::Builder::new().name("swiftlife-history".into()).spawn(move || {
        let history = history_path();
        let session = session_path();
        if let Some(path) = history.as_ref() { if let Some(parent) = path.parent() { let _ = fs::create_dir_all(parent); } }
        for entry in rx {
            if let Some(path) = history.as_ref() {
                if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
                    let line = serde_json::json!({"visited_at": entry.visited_at, "url": entry.url}).to_string();
                    let _ = writeln!(file, "{line}");
                }
            }
            if let Some(path) = session.as_ref() { let _ = fs::write(path, &entry.url); }
        }
    }).expect("SwiftLife history worker");
    tx
}

impl App {
    fn new(proxy: EventLoopProxy<AppEvent>) -> Self { Self { window: None, browser: None, web_context: None, proxy, last_history_url: String::new(), last_ui_url: String::new(), history_tx: spawn_history_writer() } }
    fn bounds(window: &Window) -> Rect {
        let size = window.inner_size();
        Rect {
            position: PhysicalPosition::new(0i32, 0i32).into(),
            size: PhysicalSize::new(size.width, size.height).into(),
        }
    }
    fn send_script(&self, script: &str) { if let Some(browser) = &self.browser { let _ = browser.evaluate_script(script); } }
    fn navigate(&self, input: &str) { let url = normalize_url(input); if let Some(browser) = &self.browser { let _ = browser.load_url(&url); } }
    fn open_downloads(&self) { let Some(dir) = downloads_dir() else { return }; let _ = fs::create_dir_all(&dir); #[cfg(target_os = "windows")] let _ = ProcessCommand::new("explorer").arg(&dir).spawn(); #[cfg(target_os = "macos")] let _ = ProcessCommand::new("open").arg(&dir).spawn(); #[cfg(all(unix, not(target_os = "macos")))] let _ = ProcessCommand::new("xdg-open").arg(&dir).spawn(); }
    fn open_history(&self) { let Some(path) = history_path() else { return }; if !path.exists() { let _ = fs::write(&path, "SwiftLife geçmişi henüz boş.\n"); } #[cfg(target_os = "windows")] let _ = ProcessCommand::new("notepad").arg(&path).spawn(); #[cfg(target_os = "macos")] let _ = ProcessCommand::new("open").arg(&path).spawn(); #[cfg(all(unix, not(target_os = "macos")))] let _ = ProcessCommand::new("xdg-open").arg(&path).spawn(); }
    fn save_history(&mut self, url: &str) {
        if url.is_empty() || url == "about:blank" || url == self.last_history_url { return; }
        self.last_history_url = url.to_string();
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_default();
        let _ = self.history_tx.send(HistoryEntry { url: url.to_string(), visited_at: stamp });
    }
    fn command(&mut self, command: Command) { match command { Command::Navigate { url } => self.navigate(&url), Command::Back => if let Some(browser) = &self.browser { let _ = browser.go_back(); }, Command::Forward => if let Some(browser) = &self.browser { let _ = browser.go_forward(); }, Command::Reload => if let Some(browser) = &self.browser { let _ = browser.reload(); }, Command::Home => self.navigate(HOME_URL), Command::FocusAddress => self.send_script("window.swiftlifeFocusAddress&&window.swiftlifeFocusAddress();"), Command::OpenDownloads => self.open_downloads(), Command::OpenHistory => self.open_history() } }
    fn push_state(&self) { if let Some(browser) = &self.browser { let back = browser.can_go_back().unwrap_or(false); let forward = browser.can_go_forward().unwrap_or(false); self.send_script(&format!("window.swiftlifeState&&window.swiftlifeState({back},{forward});")); } }
}

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() { return; }
        #[cfg(target_os = "linux")] gtk::init().expect("GTK/WebKitGTK başlatılamadı. Linux WebKitGTK paketlerini kurun.");
        let proxy = self.proxy.clone();
        let attrs = Window::default_attributes().with_title("SwiftLife — Türkçe Web Tarayıcısı").with_inner_size(LogicalSize::new(1440u32, 900u32)).with_min_inner_size(LogicalSize::new(900u32, 620u32));
        let window = Arc::new(event_loop.create_window(attrs).expect("SwiftLife penceresi oluşturulamadı"));
        let data_dir = app_data_dir(); let _ = fs::create_dir_all(&data_dir); let mut web_context = WebContext::new(Some(data_dir));
        let start_url = load_session_url().unwrap_or_else(|| HOME_URL.to_string()); let download_dir = downloads_dir(); if let Some(dir) = &download_dir { let _ = fs::create_dir_all(dir); }
        let browser_proxy = proxy.clone(); let init = chrome_script();
        let builder = WebViewBuilder::new_with_web_context(&mut web_context).with_id("swiftlife-browser").with_bounds(Self::bounds(&window)).with_url(&start_url).with_user_agent(CHROME_USER_AGENT).with_clipboard(true).with_autoplay(true).with_hotkeys_zoom(true).with_back_forward_navigation_gestures(true).with_devtools(false).with_background_color((245, 246, 248, 255)).with_initialization_script_for_main_only(init, true).with_download_started_handler(move |_url, path| { if let Some(dir) = &download_dir { if let Some(name) = path.file_name().and_then(|n| n.to_str()) { *path = dir.join(security::safe_download_name(name)); } } path.is_absolute() }).with_navigation_handler({ let proxy = browser_proxy.clone(); move |url| { if security::navigation_allowed(&url) { let _ = proxy.send_event(AppEvent::UrlChanged(url)); true } else { false } } }).with_on_page_load_handler({ let proxy = browser_proxy.clone(); move |event, url| { let _ = proxy.send_event(AppEvent::Loading(matches!(event, PageLoadEvent::Started))); let _ = proxy.send_event(AppEvent::UrlChanged(url)); } }).with_document_title_changed_handler({ let proxy = browser_proxy.clone(); move |title| { let _ = proxy.send_event(AppEvent::TitleChanged(title)); } }).with_new_window_req_handler({ let proxy = browser_proxy.clone(); move |url, _features| { let _ = proxy.send_event(AppEvent::Command(Command::Navigate { url })); NewWindowResponse::Deny } });
        let browser = builder.build_as_child(window.as_ref()).expect("SwiftLife web görünümü oluşturulamadı");
        self.window = Some(window); self.browser = Some(browser); self.web_context = Some(web_context); self.update_layout(); self.push_state();
    }
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AppEvent) { match event { AppEvent::Command(command) => self.command(command), AppEvent::UrlChanged(url) => { self.save_history(&url); if url != self.last_ui_url { self.last_ui_url = url.clone(); let value = serde_json::to_string(&url).unwrap_or_else(|_| "\"\"".into()); self.send_script(&format!("window.swiftlifeUrl&&window.swiftlifeUrl({value});")); } self.push_state(); }, AppEvent::TitleChanged(title) => { let value = serde_json::to_string(&title).unwrap_or_else(|_| "\"SwiftLife\"".into()); self.send_script(&format!("window.swiftlifeTitle&&window.swiftlifeTitle({value});")); }, AppEvent::Loading(is_loading) => { self.send_script(if is_loading { "window.swiftlifeLoading&&window.swiftlifeLoading(true);" } else { "window.swiftlifeLoading&&window.swiftlifeLoading(false);" }); self.push_state(); } } }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) { match event { WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => self.update_layout(), WindowEvent::CloseRequested => event_loop.exit(), _ => {} } }
    #[cfg(target_os = "linux")] fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) { while gtk::events_pending() { gtk::main_iteration_do(false); } }
}
impl App { fn update_layout(&self) { if let (Some(window), Some(browser)) = (&self.window, &self.browser) { let _ = browser.set_bounds(Self::bounds(window)); } } }
fn app_data_dir() -> PathBuf { dirs::data_local_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".local/share")).join("SwiftLife") }
fn downloads_dir() -> Option<PathBuf> { dirs::download_dir().or_else(|| dirs::home_dir().map(|h| h.join("Downloads"))) }
fn history_path() -> Option<PathBuf> { Some(app_data_dir().join("history.jsonl")) }
fn session_path() -> Option<PathBuf> { Some(app_data_dir().join("last-session.url")) }
fn load_session_url() -> Option<String> { let path = session_path()?; let value = fs::read_to_string(path).ok()?.trim().to_string(); if value.starts_with("http://") || value.starts_with("https://") { Some(value) } else { None } }
fn normalize_url(input: &str) -> String { let value = input.trim(); if value.is_empty() { return HOME_URL.to_string(); } if value.eq_ignore_ascii_case("about:blank") { return value.to_string(); } if value.starts_with("http://") || value.starts_with("https://") { return value.to_string(); } if value.contains(':') && !value.contains(' ') { return format!("https://www.google.com/search?q={}", urlencoding::encode(value)); } if value.contains(' ') || !value.contains('.') { return format!("https://www.google.com/search?q={}", urlencoding::encode(value)); } format!("https://{value}") }

fn chrome_script() -> String {
    r#"(() => {
if (window.__swiftlifeInstalled) return; window.__swiftlifeInstalled=true;
const style=document.createElement('style');
style.textContent=`#swiftlife-host{all:initial;position:fixed;z-index:2147483647;left:0;top:0;width:100%;height:78px;display:block;pointer-events:none;font-family:Inter,-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif}#swiftlife-host *{box-sizing:border-box}#swiftlife-host .bar{height:78px;padding:10px 12px 9px;display:flex;align-items:center;gap:8px;pointer-events:auto;color:#f6f8fb;background:linear-gradient(180deg,rgba(18,21,28,.98),rgba(12,15,21,.96));border-bottom:1px solid rgba(255,255,255,.09);box-shadow:0 3px 12px rgba(0,0,0,.22);backdrop-filter:none}#swiftlife-host .brand{display:flex;align-items:center;gap:9px;min-width:128px}#swiftlife-host .logo{width:34px;height:34px;border-radius:11px;display:grid;place-items:center;background:linear-gradient(135deg,#815cff,#4e8cff);box-shadow:0 6px 18px rgba(91,82,255,.28)}#swiftlife-host .logo svg{width:20px;height:20px;fill:none;stroke:#fff;stroke-width:2;stroke-linecap:round;stroke-linejoin:round}#swiftlife-host .name{font-size:15px;font-weight:800;letter-spacing:-.3px}#swiftlife-host .sub{font-size:9px;color:#737c8c;text-transform:uppercase;letter-spacing:.08em;margin-top:1px}#swiftlife-host .nav{display:flex;gap:2px}#swiftlife-host button{font:inherit}#swiftlife-host .btn{width:35px;height:35px;border:1px solid transparent;border-radius:10px;background:transparent;color:#c6cdd9;display:grid;place-items:center;cursor:pointer}#swiftlife-host .btn:hover{background:#252a34;border-color:#343b49;color:#fff}#swiftlife-host .btn:active{transform:scale(.95)}#swiftlife-host .btn:disabled{opacity:.3;cursor:default}#swiftlife-host .btn svg{width:18px;height:18px;fill:none;stroke:currentColor;stroke-width:1.9;stroke-linecap:round;stroke-linejoin:round}#swiftlife-host .address{height:43px;min-width:180px;flex:1;display:flex;align-items:center;gap:8px;padding:0 12px;border:1px solid #343b48;border-radius:14px;background:linear-gradient(180deg,#171b24,#0d1016);box-shadow:inset 0 1px 0 rgba(255,255,255,.035),0 2px 10px rgba(0,0,0,.16)}#swiftlife-host .address:focus-within{border-color:#6955d5;box-shadow:0 0 0 3px rgba(118,88,255,.13)}#swiftlife-host .lock{width:20px;height:20px;display:grid;place-items:center;color:#65d8a1;flex:none}#swiftlife-host .lock svg{width:15px;height:15px;fill:none;stroke:currentColor;stroke-width:2;stroke-linecap:round;stroke-linejoin:round}#swiftlife-host input{all:unset;display:block;width:100%;min-width:0;color:#f2f4f8;font-size:14px;font-weight:500}#swiftlife-host input::selection{background:#5a47b9;color:#fff}#swiftlife-host .https{font-size:10px;color:#697385;display:none;white-space:nowrap}#swiftlife-host .dot{width:8px;height:8px;border-radius:50%;background:#62d9a0;flex:none;box-shadow:0 0 9px rgba(98,217,160,.45)}#swiftlife-host .dot.busy{background:#ffc46c;box-shadow:0 0 10px rgba(255,196,108,.5);animation:slpulse .7s infinite}@keyframes slpulse{50%{opacity:.3;transform:scale(.72)}}#swiftlife-host .actions{display:flex;gap:3px}.sl-menu{position:fixed;z-index:2147483647;right:10px;top:65px;width:280px;padding:7px;border:1px solid #39404d;border-radius:14px;background:rgba(25,29,37,.99);box-shadow:0 22px 70px rgba(0,0,0,.55);display:none;color:#e9edf5}.sl-menu.open{display:block}.sl-menu button{width:100%;height:38px;border:0;border-radius:9px;background:transparent;color:inherit;text-align:left;padding:0 10px;display:flex;align-items:center;gap:10px;cursor:pointer}.sl-menu button:hover{background:#303641}.sl-menu .ico{width:18px;text-align:center;color:#aeb8c9}.sl-menu .key{margin-left:auto;color:#687286;font-size:10px}.sl-menu .sep{height:1px;background:#343b48;margin:6px 4px}.sl-menu .meta{padding:7px 10px;color:#747e90;font-size:10px;line-height:1.45}.sl-context{position:fixed;z-index:2147483647;min-width:260px;max-width:340px;padding:6px;border:1px solid rgba(255,255,255,.13);border-radius:14px;background:rgba(23,27,35,.99);box-shadow:0 22px 70px rgba(0,0,0,.55);display:none;color:#f3f5f9}.sl-context.open{display:block}.sl-context button{width:100%;height:36px;border:0;border-radius:9px;background:transparent;color:inherit;text-align:left;padding:0 10px;display:flex;align-items:center;gap:10px;cursor:pointer;font-size:13px}.sl-context button:hover{background:#303641}.sl-context .ico{width:18px;text-align:center;color:#aeb8c9}.sl-context .kind{padding:7px 10px 5px;color:#707b8e;font-size:9px;text-transform:uppercase;letter-spacing:.09em}.sl-context .sep{height:1px;background:#343b48;margin:5px 4px}`;
document.documentElement.appendChild(style);
const host=document.createElement('div');host.id='swiftlife-host';const shadow=host.attachShadow({mode:'open'});document.documentElement.appendChild(host);
shadow.innerHTML=`<div class="bar"><div class="brand"><div class="logo"><svg viewBox="0 0 24 24"><path d="M5 12h10M12 7l5 5-5 5"/></svg></div><div><div class="name">SwiftLife</div><div class="sub">Hızlı web</div></div></div><div class="nav"><button class="btn" id="back" title="Geri"><svg viewBox="0 0 24 24"><path d="M15 18l-6-6 6-6"/></svg></button><button class="btn" id="forward" title="İleri"><svg viewBox="0 0 24 24"><path d="M9 18l6-6-6-6"/></svg></button><button class="btn" id="reload" title="Yenile"><svg viewBox="0 0 24 24"><path d="M20 11a8 8 0 0 0-14.9-3M4 5v5h5M4 13a8 8 0 0 0 14.9 3M20 19v-5h-5"/></svg></button><button class="btn" id="home" title="Ana sayfa"><svg viewBox="0 0 24 24"><path d="M3 11.5L12 4l9 7.5M5.5 10v9h13v-9M9.5 19v-5h5v5"/></svg></button></div><label class="address"><span class="lock"><svg viewBox="0 0 24 24"><rect x="5" y="10" width="14" height="10" rx="2"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/></svg></span><input id="url" autocomplete="off" spellcheck="false" placeholder="Web adresi veya arama yapın…"><span class="https" id="https">HTTPS</span><span class="dot" id="dot"></span></label><div class="actions"><button class="btn" id="downloads" title="İndirilenler"><svg viewBox="0 0 24 24"><path d="M12 4v10M8 11l4 4 4-4M5 19h14"/></svg></button><button class="btn" id="menuBtn" title="Menü"><svg viewBox="0 0 24 24"><circle cx="5" cy="12" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/></svg></button></div></div><div class="sl-menu" id="menu"><button data-a="focus"><span class="ico">⌕</span>Adres çubuğuna odaklan<span class="key">Ctrl L</span></button><button data-a="reload"><span class="ico">↻</span>Sayfayı yenile<span class="key">Ctrl R</span></button><button data-a="downloads"><span class="ico">↓</span>İndirilenler klasörünü aç</button><button data-a="history"><span class="ico">◷</span>Geçmişi aç</button><div class="sep"></div><div class="meta"><b>SwiftLife</b> • Tek WebView mimarisi<br>Sağ tık: bağlantı, görsel, video ve seçim araçları</div></div><div class="sl-context" id="ctx"></div>`;
const q=s=>shadow.querySelector(s),send=o=>{try{window.ipc.postMessage(JSON.stringify(o))}catch(_){}};
const url=q('#url'),dot=q('#dot'),https=q('#https),menu=q('#menu'),ctx=q('#ctx');
const go=()=>{const v=url.value.trim();if(v)send({action:'navigate',url:v})};
q('#back').onclick=()=>send({action:'back'});q('#forward').onclick=()=>send({action:'forward'});q('#reload').onclick=()=>send({action:'reload'});q('#home').onclick=()=>send({action:'home'});q('#downloads').onclick=()=>send({action:'open_downloads'});
q('#menuBtn').onclick=e=>{e.stopPropagation();menu.classList.toggle('open')};menu.addEventListener('click',e=>{const b=e.target.closest('button');if(!b)return;menu.classList.remove('open');const a=b.dataset.a;if(a==='focus')window.swiftlifeFocusAddress();if(a==='reload')send({action:'reload'});if(a==='downloads')send({action:'open_downloads'});if(a==='history')send({action:'open_history'})});
url.addEventListener('keydown',e=>{if(e.key==='Enter'){e.preventDefault();go()}});url.addEventListener('focus',()=>url.select());
window.swiftlifeUrl=u=>{if(document.activeElement!==url)url.value=u;const ok=/^https:\/\//i.test(u);https.style.display=ok?'block':'none'};window.swiftlifeTitle=t=>{document.title=(t&&t.trim()?t+' — ':'')+'SwiftLife'};window.swiftlifeState=(b,f)=>{q('#back').disabled=!b;q('#forward').disabled=!f};window.swiftlifeLoading=x=>dot.classList.toggle('busy',x);window.swiftlifeFocusAddress=()=>{url.focus();url.select()};
const editable=e=>{const t=(e?.tagName||'').toLowerCase();return ['input','textarea','select','button'].includes(t)||!!e?.isContentEditable};
const abs=v=>{try{return new URL(v,location.href).href}catch(_){return ''}};const hostName=()=>location.hostname.toLowerCase();const social=()=>/(^|\.)x\.com$|(^|\.)twitter\.com$|(^|\.)threads\.net$/.test(hostName());
const nearest=e=>e?.closest?.('a[href],img,video,[role="img"]')||e;
const bgUrl=e=>{try{const b=getComputedStyle(e).backgroundImage,m=b&&b.match(/url\(["']?(.*?)["']?\)/i);return m?abs(m[1]):''}catch(_){return ''}};
const selected=()=>((window.getSelection?.().toString()||'').trim());
const copy=async v=>{if(!v)return;try{await navigator.clipboard.writeText(v);return}catch(_){}try{const t=document.createElement('textarea');t.value=v;t.style.cssText='position:fixed;opacity:0';document.body.appendChild(t);t.select();document.execCommand('copy');t.remove()}catch(_){}};
const download=v=>{if(!v)return;if(v.startsWith('blob:')){fetch(v).then(r=>r.blob()).then(b=>{const u=URL.createObjectURL(b),a=document.createElement('a');a.href=u;a.download='swiftlife-download';a.style.display='none';document.body.appendChild(a);a.click();setTimeout(()=>{a.remove();URL.revokeObjectURL(u)},2000)}).catch(()=>{})}else{const a=document.createElement('a');a.href=v;a.download='';a.rel='noreferrer';a.style.display='none';document.body.appendChild(a);a.click();setTimeout(()=>a.remove(),1000)}};
const open=v=>{if(v)location.href=v};const hide=()=>ctx.classList.remove('open');
function item(icon,label,fn){const b=document.createElement('button');b.innerHTML='<span class="ico">'+icon+'</span>'+label;b.onclick=e=>{e.preventDefault();e.stopPropagation();hide();fn()};ctx.appendChild(b)}
function sep(){const d=document.createElement('div');d.className='sep';ctx.appendChild(d)}
function context(info){ctx.replaceChildren();const title=document.createElement('div');title.className='kind';title.textContent=social()?'SwiftLife • sosyal medya araçları':'SwiftLife • hızlı işlemler';ctx.appendChild(title);
if(info.kind==='image'){item('◉','Görseli aç',()=>open(info.url));item('⧉','Görsel URL’sini kopyala',()=>copy(info.url));item('↓','Görseli indir',()=>download(info.url));if(info.link){sep();item('↗','Gönderiyi aç',()=>open(info.link))}}
else if(info.kind==='video'){item('▶','Videoyu aç',()=>open(info.url));item('⧉','Video URL’sini kopyala',()=>copy(info.url));item('↓','Videoyu indir',()=>download(info.url))}
else if(info.kind==='link'){item('↗',social()?'Gönderiyi / bağlantıyı aç':'Bağlantıyı aç',()=>open(info.url));item('⧉','Bağlantı adresini kopyala',()=>copy(info.url));item('↓','Bağlantıyı indir',()=>download(info.url))}
else if(info.kind==='selection'){item('⧉','Seçimi kopyala',()=>copy(info.text));item('⌕','Seçimde ara',()=>open('https://www.google.com/search?q='+encodeURIComponent(info.text)))}
else{item('↻','Sayfayı yenile',()=>location.reload());item('⌂','Ana sayfayı aç',()=>open('https://www.google.com'))}
sep();item('×','Kapat',hide)}
document.addEventListener('contextmenu',e=>{if(host.contains(e.target)||editable(e.target))return;const base=nearest(e.target);const a=base?.closest?.('a[href]');const img=base?.closest?.('img');const vid=base?.closest?.('video');let info=null;if(img){info={kind:'image',url:abs(img.currentSrc||img.src||img.getAttribute('src')),link:a?abs(a.href):''}}else if(vid){info={kind:'video',url:abs(vid.currentSrc||vid.src||vid.getAttribute('src'))}}else{const bg=bgUrl(base);if(bg)info={kind:'image',url:bg,link:a?abs(a.href):''};else if(a)info={kind:'link',url:abs(a.href)}}if(!info){const s=selected();info=s?{kind:'selection',text:s}:{kind:'page'}}e.preventDefault();e.stopPropagation();context(info);const w=ctx.offsetWidth||290,h=ctx.offsetHeight||280,p=8;ctx.style.left=Math.max(p,Math.min(e.clientX,innerWidth-w-p))+'px';ctx.style.top=Math.max(84,Math.min(e.clientY,innerHeight-h-p))+'px';ctx.classList.add('open')},true);
document.addEventListener('mousedown',e=>{if(!ctx.contains(e.target))hide();if(!menu.contains(e.target))menu.classList.remove('open')},true);document.addEventListener('keydown',e=>{if(e.key==='Escape'){hide();menu.classList.remove('open')}if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='l'){e.preventDefault();window.swiftlifeFocusAddress()}},true);
window.addEventListener('blur',()=>{hide();menu.classList.remove('open')});
})();"#.to_string()
}

fn main() -> Result<(), Box<dyn std::error::Error>> { let event_loop = EventLoop::<AppEvent>::with_user_event().build()?; let proxy = event_loop.create_proxy(); let mut app = App::new(proxy); event_loop.run_app(&mut app)?; Ok(()) }
