//! Cliente HTTPS mínimo sobre WinHTTP (el de Windows): TLS y proxy del sistema,
//! redirecciones automáticas, sin dependencias nuevas. Solo `https://`.

use windows::core::{w, PCWSTR};
use windows::Win32::Networking::WinHttp::{
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders, WinHttpReadData,
    WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetTimeouts, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
    WINHTTP_FLAG_SECURE, WINHTTP_QUERY_CONTENT_TYPE, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
};

pub struct Response {
    pub status: u16,
    #[allow(dead_code)] // útil para diagnóstico; los íconos se validan por su firma
    pub content_type: String,
    pub body: Vec<u8>,
}

/// Cierra un handle de WinHTTP al salir del alcance.
struct Handle(*mut core::ffi::c_void);
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            let _ = unsafe { WinHttpCloseHandle(self.0) };
        }
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Separa "https://host[:puerto]/ruta?query" en (host, puerto, ruta).
pub fn split_https(url: &str) -> Result<(String, u16, String), String> {
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| format!("Solo se permiten direcciones https: {url}"))?;
    let (authority, path) = match rest.find(['/', '?', '#']) {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let path = path.split('#').next().unwrap_or("/");
    let path = if path.starts_with('?') { format!("/{path}") } else { path.to_string() };
    if authority.is_empty() || authority.contains('@') || authority.contains(char::is_whitespace) {
        return Err(format!("Dirección inválida: {url}"));
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) if !h.contains(']') || h.ends_with(']') => {
            (h.to_string(), p.parse::<u16>().map_err(|_| format!("Puerto inválido: {url}"))?)
        }
        _ => (authority.to_string(), 443),
    };
    Ok((host, port, if path.is_empty() { "/".into() } else { path }))
}

/// GET con encabezados extra. `max_bytes` corta descargas demasiado grandes.
pub fn get(url: &str, headers: &[(&str, &str)], max_bytes: usize) -> Result<Response, String> {
    let (host, port, path) = split_https(url)?;
    let err = |what: &str, e: windows::core::Error| format!("{what}: {}", e.message());
    unsafe {
        let session = Handle(WinHttpOpen(w!("Debloater/0.1"), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0));
        if session.0.is_null() {
            return Err("No se pudo iniciar WinHTTP".into());
        }
        // Resolver, conectar, enviar, recibir (ms).
        let _ = WinHttpSetTimeouts(session.0, 10_000, 10_000, 15_000, 20_000);
        let host_w = wide(&host);
        let connect = Handle(WinHttpConnect(session.0, PCWSTR(host_w.as_ptr()), port, 0));
        if connect.0.is_null() {
            return Err(format!("No se pudo conectar con {host}"));
        }
        let path_w = wide(&path);
        let request = Handle(WinHttpOpenRequest(
            connect.0,
            w!("GET"),
            PCWSTR(path_w.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        ));
        if request.0.is_null() {
            return Err("No se pudo armar la solicitud".into());
        }
        let extra: String = headers.iter().map(|(k, v)| format!("{k}: {v}\r\n")).collect();
        let extra_w: Vec<u16> = extra.encode_utf16().collect();
        WinHttpSendRequest(request.0, (!extra_w.is_empty()).then_some(&extra_w[..]), None, 0, 0, 0)
            .map_err(|e| err(&format!("No se pudo contactar a {host}"), e))?;
        WinHttpReceiveResponse(request.0, std::ptr::null_mut()).map_err(|e| err(&format!("{host} no respondió"), e))?;

        let mut status = 0u32;
        let mut len = 4u32;
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some(&mut status as *mut u32 as *mut _),
            &mut len,
            std::ptr::null_mut(),
        )
        .map_err(|e| err("Respuesta sin código de estado", e))?;

        let mut ct = [0u16; 256];
        let mut ct_len = (ct.len() * 2) as u32;
        let content_type = match WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_CONTENT_TYPE,
            PCWSTR::null(),
            Some(ct.as_mut_ptr() as *mut _),
            &mut ct_len,
            std::ptr::null_mut(),
        ) {
            Ok(()) => String::from_utf16_lossy(&ct[..ct_len as usize / 2]).to_ascii_lowercase(),
            Err(_) => String::new(),
        };

        let mut body = Vec::new();
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            let mut read = 0u32;
            WinHttpReadData(request.0, buf.as_mut_ptr() as *mut _, buf.len() as u32, &mut read)
                .map_err(|e| err("Se cortó la descarga", e))?;
            if read == 0 {
                break;
            }
            body.extend_from_slice(&buf[..read as usize]);
            if body.len() > max_bytes {
                return Err(format!("Respuesta demasiado grande (más de {} KB)", max_bytes / 1024));
            }
        }
        Ok(Response { status: status as u16, content_type, body })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_urls() {
        assert_eq!(split_https("https://7-zip.org").unwrap(), ("7-zip.org".into(), 443, "/".into()));
        assert_eq!(
            split_https("https://www.virustotal.com/api/v3/files/abc?x=1#frag").unwrap(),
            ("www.virustotal.com".into(), 443, "/api/v3/files/abc?x=1".into())
        );
        assert_eq!(split_https("https://host:8443/a").unwrap(), ("host".into(), 8443, "/a".into()));
        assert_eq!(split_https("https://host?q=1").unwrap(), ("host".into(), 443, "/?q=1".into()));
        assert!(split_https("http://insecure.com").is_err());
        assert!(split_https("https://user@host/").is_err());
        assert!(split_https("file:///c:/x").is_err());
    }

    /// Conexión real (solo lectura) para comprobar TLS y redirecciones.
    #[test]
    #[ignore]
    fn real_https_get() {
        let r = get("https://7-zip.org/", &[], 2 * 1024 * 1024).unwrap();
        assert_eq!(r.status, 200);
        assert!(r.content_type.contains("html"), "{}", r.content_type);
    }
}
