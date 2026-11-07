//! Verificación de firmas digitales (Authenticode) con WinVerifyTrust de Windows.

use std::path::Path;
use windows::core::{GUID, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::Security::Cryptography::{CertGetNameStringW, CERT_NAME_SIMPLE_DISPLAY_TYPE};
use windows::Win32::Security::WinTrust::{
    WTHelperGetProvSignerFromChain, WTHelperProvDataFromStateData, WinVerifyTrust, WINTRUST_ACTION_GENERIC_VERIFY_V2,
    WINTRUST_DATA, WINTRUST_DATA_0, WINTRUST_FILE_INFO, WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE,
    WTD_STATEACTION_VERIFY, WTD_UI_NONE,
};

/// Comprueba que el archivo tenga una firma válida (cadena hasta una raíz de confianza
/// de Windows) y devuelve el nombre del firmante.
pub fn signer(path: &Path) -> Result<String, String> {
    let wide: Vec<u16> = path.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
    let mut file = WINTRUST_FILE_INFO {
        cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: PCWSTR(wide.as_ptr()),
        ..Default::default()
    };
    let mut data = WINTRUST_DATA {
        cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        // Sin consultar revocación en línea: la verificación no depende de internet.
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 { pFile: &mut file },
        dwStateAction: WTD_STATEACTION_VERIFY,
        ..Default::default()
    };
    let mut action: GUID = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    let status = unsafe { WinVerifyTrust(HWND::default(), &mut action, &mut data as *mut _ as *mut _) };

    let name = (status == 0)
        .then(|| unsafe {
            let prov = WTHelperProvDataFromStateData(data.hWVTStateData);
            if prov.is_null() {
                return None;
            }
            let sgnr = WTHelperGetProvSignerFromChain(prov, 0, false, 0);
            if sgnr.is_null() || (*sgnr).csCertChain == 0 {
                return None;
            }
            let cert = (*(*sgnr).pasCertChain).pCert;
            let mut buf = [0u16; 256];
            let n = CertGetNameStringW(cert, CERT_NAME_SIMPLE_DISPLAY_TYPE, 0, None, Some(&mut buf));
            (n > 1).then(|| String::from_utf16_lossy(&buf[..n as usize - 1]))
        })
        .flatten();

    // Siempre cerrar el estado que abrió WinVerifyTrust.
    data.dwStateAction = WTD_STATEACTION_CLOSE;
    unsafe { WinVerifyTrust(HWND::default(), &mut action, &mut data as *mut _ as *mut _) };

    if status != 0 {
        return Err(t!("signature_invalid", code = format!("0x{:08X}", status as u32)));
    }
    name.ok_or_else(|| t!("signer_unreadable"))
}

/// La firma tiene que ser válida y de Microsoft.
pub fn require_microsoft(path: &Path) -> Result<(), String> {
    let who = signer(path)?;
    if who == "Microsoft Corporation" {
        Ok(())
    } else {
        Err(t!("signed_by_other", signer = who))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsigned_file_is_rejected() {
        let p = std::env::temp_dir().join(format!("debloater-unsigned-{}.exe", std::process::id()));
        std::fs::write(&p, b"MZ no es un ejecutable firmado").unwrap();
        let r = signer(&p);
        std::fs::remove_file(&p).unwrap();
        assert!(r.is_err());
    }

    #[test]
    fn real_winget_is_signed_by_microsoft() {
        let exe = crate::winget::winget_exe_for_tests();
        assert_eq!(signer(&exe).unwrap(), "Microsoft Corporation");
        require_microsoft(&exe).unwrap();
    }
}
