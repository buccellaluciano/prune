//! VirusTotal sin clave de API: la app muestra el SHA-256 exacto del instalador (el que
//! declara winget y que winget verifica al descargar) y abre el informe en el navegador
//! para que la persona lo revise ella misma.

use windows::core::PCWSTR;

fn valid_sha256(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Dirección del informe de VirusTotal para un archivo. Solo virustotal.com y un hash válido.
pub fn report_url(sha256: &str) -> Result<String, String> {
    if !valid_sha256(sha256) {
        return Err("Hash inválido".into());
    }
    Ok(format!("https://www.virustotal.com/gui/file/{}", sha256.to_ascii_lowercase()))
}

/// Abre el informe en el navegador predeterminado.
pub fn open_report(sha256: &str) -> Result<(), String> {
    let url = report_url(sha256)?;
    let u: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
    let r = unsafe {
        windows::Win32::UI::Shell::ShellExecuteW(
            None,
            windows::core::w!("open"),
            PCWSTR(u.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        )
    };
    if r.0 as usize > 32 { Ok(()) } else { Err("No se pudo abrir el navegador".into()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_valid_hashes_become_urls() {
        let h = "3FE4AB99FA15629C652944309A2B2BC6E41FC933FC6B63F5DF1EBCF3AEA68263";
        assert_eq!(
            report_url(h).unwrap(),
            "https://www.virustotal.com/gui/file/3fe4ab99fa15629c652944309a2b2bc6e41fc933fc6b63f5df1ebcf3aea68263"
        );
        assert!(report_url("abc").is_err());
        assert!(report_url(&format!("{}/../evil", &h[..58])).is_err());
        assert!(report_url(&"z".repeat(64)).is_err());
    }
}
