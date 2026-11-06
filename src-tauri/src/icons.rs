//! Íconos de los programas del catálogo: se buscan en el sitio oficial de cada uno
//! (`site` en apps.toml), se validan como imagen y se guardan en caché local.

use crate::catalog;
use crate::http;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const HTML_MAX: usize = 2 * 1024 * 1024;
const ICON_MAX: usize = 512 * 1024;
/// Si no se encontró ícono, no reintentar hasta dentro de una semana.
const RETRY_AFTER: Duration = Duration::from_secs(7 * 24 * 3600);

#[derive(Debug, Clone, PartialEq)]
struct Candidate {
    url: String,
    score: u32,
}

/// Tipo de imagen por su firma (no por la extensión ni el content-type).
fn sniff(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(("png", "image/png"))
    } else if bytes.starts_with(&[0, 0, 1, 0]) {
        Some(("ico", "image/x-icon"))
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(("jpg", "image/jpeg"))
    } else if bytes.starts_with(b"GIF8") {
        Some(("gif", "image/gif"))
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(("webp", "image/webp"))
    } else {
        let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).to_ascii_lowercase();
        (head.contains("<svg")).then_some(("svg", "image/svg+xml"))
    }
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find(name) {
        let start = from + i;
        from = start + name.len();
        // Tiene que ser el atributo completo: precedido por espacio y seguido de '='.
        if !lower[..start].ends_with(|c: char| c.is_whitespace()) {
            continue;
        }
        let rest = lower[from..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else { continue };
        let offset = tag.len() - rest.len();
        let raw = tag[offset..].trim_start();
        let value = match raw.chars().next()? {
            q @ ('"' | '\'') => raw[1..].split(q).next()?.to_string(),
            _ => raw.split(|c: char| c.is_whitespace() || c == '>').next()?.to_string(),
        };
        return Some(value.trim().to_string());
    }
    None
}

/// Resuelve un href contra la URL del sitio. Solo devuelve https.
fn resolve(base: &str, href: &str) -> Option<String> {
    let href = href.trim().replace("&amp;", "&");
    if href.starts_with("https://") {
        return Some(href);
    }
    if let Some(rest) = href.strip_prefix("//") {
        return Some(format!("https://{rest}"));
    }
    if href.starts_with("http:") || href.starts_with("data:") || href.contains(':') && !href.starts_with('/') {
        return None;
    }
    let (host, port, path) = http::split_https(base).ok()?;
    let origin = if port == 443 { format!("https://{host}") } else { format!("https://{host}:{port}") };
    if href.starts_with('/') {
        return Some(format!("{origin}{href}"));
    }
    let dir = path.split(['?', '#']).next().unwrap_or("/");
    let dir = &dir[..dir.rfind('/').map_or(0, |i| i + 1)];
    Some(format!("{origin}{}{href}", if dir.is_empty() { "/" } else { dir }))
}

/// Las direcciones de íconos salen del HTML de cada sitio: solo se siguen las que apuntan
/// a un nombre público en el puerto 443. Nada de IPs, `localhost` ni nombres de red local,
/// para que una página no pueda hacer que la app consulte equipos de la red de la persona.
fn public_https(url: &str) -> bool {
    let Ok((host, port, _)) = http::split_https(url) else { return false };
    let h = host.trim_matches(['[', ']']).to_ascii_lowercase();
    let local_suffix = [".local", ".lan", ".home", ".internal", ".localdomain", ".corp", ".intranet"];
    port == 443
        && h.contains('.')
        && h.parse::<std::net::IpAddr>().is_err()
        && h != "localhost"
        && !h.ends_with(".localhost")
        && !local_suffix.iter().any(|s| h.ends_with(s))
        && h.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

/// Íconos declarados en el HTML, de mejor a peor.
fn candidates(base: &str, html: &str) -> Vec<Candidate> {
    let lower = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = lower[from..].find("<link") {
        let start = from + i;
        let end = lower[start..].find('>').map_or(lower.len(), |e| start + e);
        from = end;
        let tag = &html[start..end];
        let rel = attr(tag, "rel").unwrap_or_default().to_ascii_lowercase();
        if !rel.split_whitespace().any(|r| r == "icon" || r == "apple-touch-icon" || r == "apple-touch-icon-precomposed") {
            continue;
        }
        let Some(url) = attr(tag, "href").and_then(|h| resolve(base, &h)).filter(|u| public_https(u)) else { continue };
        let sizes = attr(tag, "sizes").unwrap_or_default();
        let declared: u32 = sizes
            .split_whitespace()
            .filter_map(|s| s.split(['x', 'X']).next()?.parse().ok())
            .max()
            .unwrap_or(0);
        let lurl = url.to_ascii_lowercase();
        let ty = attr(tag, "type").unwrap_or_default();
        let score = if lurl.contains(".svg") || ty.contains("svg") {
            200
        } else if rel.contains("apple-touch-icon") {
            declared.max(180)
        } else if lurl.contains(".ico") {
            declared.max(32).min(64)
        } else {
            declared.max(48)
        };
        // Más grande es mejor hasta 256; más allá no suma.
        out.push(Candidate { url, score: score.min(256) });
    }
    out.sort_by(|a, b| b.score.cmp(&a.score));
    out.dedup_by(|a, b| a.url == b.url);
    out
}

fn fetch_image(url: &str) -> Option<(Vec<u8>, &'static str)> {
    let r = http::get(url, &[], ICON_MAX).ok()?;
    if r.status != 200 || r.body.len() < 16 {
        return None;
    }
    let (ext, _) = sniff(&r.body)?;
    Some((r.body, ext))
}

/// Busca el mejor ícono para un sitio.
fn find_icon(site: &str) -> Option<(Vec<u8>, &'static str)> {
    let mut list = Vec::new();
    if let Ok(r) = http::get(site, &[], HTML_MAX) {
        if r.status == 200 {
            list = candidates(site, &String::from_utf8_lossy(&r.body));
        }
    }
    for fallback in ["/apple-touch-icon.png", "/favicon.ico"] {
        if let Some(u) = resolve(site, fallback) {
            list.push(Candidate { url: u, score: 0 });
        }
    }
    list.into_iter().take(6).find_map(|c| fetch_image(&c.url))
}

fn file_stem(id: &str) -> String {
    id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect()
}

fn data_url(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let (_, mime) = sniff(&bytes)?;
    Some(format!("data:{mime};base64,{}", crate::user::base64(&bytes)))
}

fn cached(dir: &Path, id: &str) -> Result<Option<String>, ()> {
    let stem = file_stem(id);
    for ext in ["png", "svg", "ico", "jpg", "gif", "webp"] {
        let p = dir.join(format!("{stem}.{ext}"));
        if p.exists() {
            return Ok(data_url(&p));
        }
    }
    let none = dir.join(format!("{stem}.none"));
    let fresh = std::fs::metadata(&none)
        .and_then(|m| m.modified())
        .is_ok_and(|t| SystemTime::now().duration_since(t).unwrap_or_default() < RETRY_AFTER);
    if fresh { Ok(None) } else { Err(()) }
}

/// Íconos de todo el catálogo como data URLs. Si `fetch`, busca en internet los que
/// falten (en paralelo); si no, devuelve solo lo que ya está en caché.
pub fn all(dir: &Path, fetch: bool) -> HashMap<String, String> {
    let _ = std::fs::create_dir_all(dir);
    let mut out = HashMap::new();
    let mut missing = Vec::new();
    for app in &catalog::apps().app {
        match cached(dir, &app.id) {
            Ok(Some(url)) => {
                out.insert(app.id.clone(), url);
            }
            Ok(None) => {}
            Err(()) => {
                if let Some(site) = &app.site {
                    missing.push((app.id.clone(), site.clone()));
                }
            }
        }
    }
    if !fetch || missing.is_empty() {
        return out;
    }
    let queue = std::sync::Mutex::new(missing);
    let found = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..6 {
            s.spawn(|| loop {
                let Some((id, site)) = queue.lock().unwrap().pop() else { return };
                let stem = file_stem(&id);
                match find_icon(&site) {
                    Some((bytes, ext)) => {
                        let path: PathBuf = dir.join(format!("{stem}.{ext}"));
                        if std::fs::write(&path, &bytes).is_ok() {
                            if let Some(u) = data_url(&path) {
                                found.lock().unwrap().push((id, u));
                            }
                        }
                    }
                    None => {
                        let _ = std::fs::write(dir.join(format!("{stem}.none")), b"");
                    }
                }
            });
        }
    });
    out.extend(found.into_inner().unwrap());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_and_ranks_icons_in_html() {
        let html = r#"<head>
            <link rel="stylesheet" href="/x.css">
            <link rel="shortcut icon" href="/favicon.ico">
            <LINK REL="icon" type="image/png" sizes="32x32" href="img/i32.png">
            <link href='https://cdn.example.com/touch.png' rel='apple-touch-icon' sizes='180x180'>
            <link rel="icon" href="http://insecure.example.com/i.png">
            <link rel="mask-icon" href="/mask.svg">
        </head>"#;
        let c = candidates("https://example.com/app/index.html", html);
        let urls: Vec<_> = c.iter().map(|c| c.url.as_str()).collect();
        assert_eq!(urls[0], "https://cdn.example.com/touch.png");
        assert!(urls.contains(&"https://example.com/app/img/i32.png"));
        assert!(urls.contains(&"https://example.com/favicon.ico"));
        assert!(!urls.iter().any(|u| u.contains("insecure") || u.contains("mask")));
    }

    #[test]
    fn icon_urls_must_be_public_hosts() {
        for ok in ["https://cdn.example.com/i.png", "https://www.mozilla.org/favicon.ico"] {
            assert!(public_https(ok), "{ok}");
        }
        for bad in [
            "https://192.168.1.1/i.png",
            "https://10.0.0.5/x",
            "https://[::1]/x",
            "https://localhost/x",
            "https://router.local/x",
            "https://nas/x",
            "https://example.com:8443/x",
            "http://example.com/x",
        ] {
            assert!(!public_https(bad), "{bad}");
        }
        let html = r#"<link rel="icon" href="https://192.168.0.1/admin.png"><link rel="icon" href="/ok.png">"#;
        let urls: Vec<_> = candidates("https://example.com/", html).into_iter().map(|c| c.url).collect();
        assert_eq!(urls, ["https://example.com/ok.png"]);
    }

    #[test]
    fn sniffs_real_image_types_only() {
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\n....").map(|s| s.0), Some("png"));
        assert_eq!(sniff(&[0, 0, 1, 0, 1, 0]).map(|s| s.0), Some("ico"));
        assert_eq!(sniff(b"<?xml version='1.0'?><svg xmlns=...>").map(|s| s.0), Some("svg"));
        assert_eq!(sniff(b"<!doctype html><html>"), None);
        assert_eq!(sniff(b"MZ\x90\x00 ejecutable"), None);
    }

    #[test]
    fn every_catalog_app_has_an_https_site() {
        for a in &catalog::apps().app {
            let site = a.site.as_deref().unwrap_or_else(|| panic!("{} sin site", a.id));
            assert!(http::split_https(site).is_ok(), "{}: {site}", a.id);
        }
    }

    /// Busca íconos reales para todo el catálogo. `cargo test -- --ignored --nocapture real_icons`
    #[test]
    #[ignore]
    fn real_icons() {
        let dir = std::env::temp_dir().join(format!("debloater-icons-{}", std::process::id()));
        let got = all(&dir, true);
        let total = catalog::apps().app.len();
        for a in &catalog::apps().app {
            println!("{} {}", if got.contains_key(&a.id) { "OK  " } else { "FALTA" }, a.id);
        }
        println!("{}/{total} íconos", got.len());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
