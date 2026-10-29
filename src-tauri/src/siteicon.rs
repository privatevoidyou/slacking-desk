//! 网址图标：添加网址时去这个网站取它自己的图标（apple-touch-icon > 最大的 icon > /favicon.ico）。
//! 只请求这一个网站，取不到就返回 None（首页显示首字母）。

use std::io::Read;
use std::time::Duration;

const MAX_PAGE: u64 = 1024 * 1024;
const MAX_ICON: u64 = 1024 * 1024;

/// 返回 (图片内容, 扩展名)
pub fn fetch(page_url: &str) -> Option<(Vec<u8>, &'static str)> {
    if !page_url.starts_with("http://") && !page_url.starts_with("https://") {
        return None;
    }
    let tls = native_tls::TlsConnector::new().ok()?;
    let agent = ureq::AgentBuilder::new()
        .tls_connector(std::sync::Arc::new(tls))
        .timeout(Duration::from_secs(6))
        .try_proxy_from_env(true)
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36")
        .build();

    let mut candidates: Vec<(u32, url::Url)> = vec![];
    let base = url::Url::parse(page_url).ok()?;
    let mut final_url = base.clone();
    if let Ok(resp) = agent.get(page_url).call() {
        final_url = url::Url::parse(resp.get_url()).unwrap_or(base);
        let mut html = String::new();
        let _ = resp.into_reader().take(MAX_PAGE).read_to_string(&mut html);
        for (score, href) in links(&html) {
            if let Ok(u) = final_url.join(&href) {
                candidates.push((score, u));
            }
        }
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    if let Ok(u) = final_url.join("/favicon.ico") {
        candidates.push((0, u));
    }
    for (_, u) in candidates.into_iter().take(4) {
        if !matches!(u.scheme(), "http" | "https") {
            continue;
        }
        let Ok(resp) = agent.get(u.as_str()).call() else { continue };
        let mut bytes = vec![];
        if resp.into_reader().take(MAX_ICON + 1).read_to_end(&mut bytes).is_err() || bytes.len() as u64 > MAX_ICON {
            continue;
        }
        if let Some(ext) = image_ext(&bytes) {
            return Some((bytes, ext));
        }
    }
    None
}

/// 从网页里找 <link rel="...icon..." href="..." sizes="..."> ，返回 (分数, href)
fn links(html: &str) -> Vec<(u32, String)> {
    let lower = html.to_ascii_lowercase();
    let mut out = vec![];
    let mut pos = 0;
    while let Some(i) = lower[pos..].find("<link") {
        let start = pos + i;
        let end = lower[start..].find('>').map_or(lower.len(), |j| start + j);
        let tag = &html[start..end];
        pos = end;
        let rel = attr(tag, "rel").unwrap_or_default().to_ascii_lowercase();
        let Some(href) = attr(tag, "href") else { continue };
        if href.starts_with("data:") || !rel.contains("icon") || rel.contains("mask-icon") {
            continue;
        }
        let size = attr(tag, "sizes")
            .and_then(|s| s.to_ascii_lowercase().split('x').next().and_then(|n| n.trim().parse::<u32>().ok()))
            .unwrap_or(if rel.contains("apple-touch-icon") { 180 } else { 32 });
        let svg = href.to_ascii_lowercase().ends_with(".svg");
        let score = if rel.contains("apple-touch-icon") { 1000 + size } else if svg { 900 } else { size.min(512) };
        out.push((score, href.replace("&amp;", "&")));
    }
    out
}

/// 读标签里的属性值（大小写不敏感，支持双引号、单引号和不带引号）
fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find(name) {
        let at = from + i;
        from = at + name.len();
        // 前面必须是空白，避免 data-href 之类
        if at == 0 || !lower.as_bytes()[at - 1].is_ascii_whitespace() {
            continue;
        }
        let rest = tag[from..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else { continue };
        let rest = rest.trim_start();
        let value = match rest.chars().next()? {
            q @ ('"' | '\'') => rest[1..].split(q).next()?,
            _ => rest.split(char::is_whitespace).next()?,
        };
        return Some(value.trim().to_string());
    }
    None
}

fn image_ext(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(b"\x89PNG") {
        Some("png")
    } else if b.starts_with(&[0xFF, 0xD8]) {
        Some("jpg")
    } else if b.starts_with(&[0, 0, 1, 0]) {
        Some("ico")
    } else if b.starts_with(b"GIF8") {
        Some("gif")
    } else if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("webp")
    } else {
        let head = String::from_utf8_lossy(&b[..b.len().min(1024)]).to_ascii_lowercase();
        (head.contains("<svg")).then_some("svg")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_links() {
        let html = r#"<head><LINK rel="icon" href="/a.png" sizes="16x16"><link rel='apple-touch-icon' href='/t.png'>
            <link rel=stylesheet href=/x.css><link rel="shortcut icon" href="/f.ico"></head>"#;
        let mut l = links(html);
        l.sort_by(|a, b| b.0.cmp(&a.0));
        assert_eq!(l[0].1, "/t.png");
        assert_eq!(l.len(), 3);
    }
}
