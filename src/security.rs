use std::path::Path;

pub fn navigation_allowed(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://") || lower == "about:blank"
}

pub fn safe_download_name(name: &str) -> String {
    let base = Path::new(name).file_name().and_then(|v| v.to_str()).unwrap_or("download");
    let cleaned: String = base.chars().map(|c| if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c }).collect();
    let cleaned = cleaned.trim_matches('.').trim();
    if cleaned.is_empty() { "download".into() } else { cleaned.chars().take(180).collect() }
}
