//! "Confirmá que sos vos" antes de instalar una actualización.
//!
//! Se usa el diálogo seguro de Windows, nunca un campo de la app: P-Rune no ve ni maneja la
//! contraseña o el PIN, solo recibe si se verificó o no.
//!
//! 1. Windows Hello (`UserConsentVerifier`): PIN, huella, rostro o lo que use la cuenta,
//!    atado a la ventana de P-Rune.
//! 2. Si la cuenta no tiene Windows Hello: el diálogo de credenciales de Windows
//!    (`CredUIPromptForWindowsCredentials`) limitado al usuario actual, y Windows valida la
//!    contraseña (`LogonUser`). Ese búfer sí pasa por la app un instante: se borra de la
//!    memoria apenas se valida.
//!
//! Si el usuario cancela o falla, la actualización no sigue.

use windows::Win32::Foundation::HWND;
use windows::core::HSTRING;

/// Resultado de la verificación.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Verified,
    /// Canceló o no coincidió.
    Rejected,
}

pub fn confirm(hwnd: HWND, message: &str) -> Result<Outcome, String> {
    match hello(hwnd, message) {
        Ok(Some(o)) => Ok(o),
        // Sin Windows Hello configurado: diálogo de credenciales de Windows.
        Ok(None) => credui(hwnd, message),
        Err(e) => Err(e),
    }
}

/// `Ok(None)` si Windows Hello no está disponible para esta cuenta.
fn hello(hwnd: HWND, message: &str) -> Result<Option<Outcome>, String> {
    use windows::Security::Credentials::UI::{
        UserConsentVerificationResult, UserConsentVerifier, UserConsentVerifierAvailability,
    };
    use windows::Win32::System::WinRT::IUserConsentVerifierInterop;
    use windows_future::IAsyncOperation;

    let available = UserConsentVerifier::CheckAvailabilityAsync()
        .and_then(|op| op.join())
        .map_err(|e| e.message())?;
    if available != UserConsentVerifierAvailability::Available {
        return Ok(None);
    }
    let interop: IUserConsentVerifierInterop =
        windows::core::factory::<UserConsentVerifier, IUserConsentVerifierInterop>().map_err(|e| e.message())?;
    let op: IAsyncOperation<UserConsentVerificationResult> =
        unsafe { interop.RequestVerificationForWindowAsync(hwnd, &HSTRING::from(message)) }.map_err(|e| e.message())?;
    let result = op.join().map_err(|e| e.message())?;
    Ok(Some(if result == UserConsentVerificationResult::Verified { Outcome::Verified } else { Outcome::Rejected }))
}

fn credui(hwnd: HWND, message: &str) -> Result<Outcome, String> {
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, HANDLE};
    use windows::Win32::Security::Credentials::{
        CREDUI_INFOW, CREDUIWIN_ENUMERATE_CURRENT_USER, CRED_PACK_FLAGS, CRED_PACK_PROTECTED_CREDENTIALS, CredPackAuthenticationBufferW,
        CredUIPromptForWindowsCredentialsW, CredUnPackAuthenticationBufferW,
    };
    use windows::Win32::Security::{LOGON32_LOGON_INTERACTIVE, LOGON32_PROVIDER_DEFAULT, LogonUserW};
    use windows::Win32::System::Com::CoTaskMemFree;

    let user = crate::user::username().ok_or_else(|| t!("update_verify_failed"))?;
    let caption = HSTRING::from("P-Rune");
    let text = HSTRING::from(message);
    let info = CREDUI_INFOW {
        cbSize: std::mem::size_of::<CREDUI_INFOW>() as u32,
        hwndParent: hwnd,
        pszMessageText: PCWSTR(text.as_ptr()),
        pszCaptionText: PCWSTR(caption.as_ptr()),
        ..Default::default()
    };

    // Precarga el usuario actual: el diálogo solo pide su contraseña.
    let wuser = HSTRING::from(user.as_str());
    let mut in_size = 0u32;
    let _ = unsafe { CredPackAuthenticationBufferW(CRED_PACK_FLAGS(0), PCWSTR(wuser.as_ptr()), PCWSTR(windows::core::w!("").as_ptr()), None, &mut in_size) };
    let mut in_buf = vec![0u8; in_size as usize];
    unsafe {
        CredPackAuthenticationBufferW(CRED_PACK_FLAGS(0), PCWSTR(wuser.as_ptr()), windows::core::w!(""), Some(in_buf.as_mut_ptr()), &mut in_size)
    }
    .map_err(|e| e.message())?;

    let mut package = 0u32;
    let mut out: *mut core::ffi::c_void = std::ptr::null_mut();
    let mut out_size = 0u32;
    let r = unsafe {
        CredUIPromptForWindowsCredentialsW(
            Some(&info),
            0,
            &mut package,
            Some(in_buf.as_ptr().cast()),
            in_size,
            &mut out,
            &mut out_size,
            None,
            CREDUIWIN_ENUMERATE_CURRENT_USER,
        )
    };
    if r == ERROR_CANCELLED.0 {
        return Ok(Outcome::Rejected);
    }
    if r != 0 || out.is_null() {
        return Err(t!("update_verify_failed"));
    }

    // Borra el búfer del diálogo (y lo libera) pase lo que pase.
    struct Wipe(*mut core::ffi::c_void, u32);
    impl Drop for Wipe {
        fn drop(&mut self) {
            unsafe {
                std::ptr::write_bytes(self.0.cast::<u8>(), 0, self.1 as usize);
                CoTaskMemFree(Some(self.0));
            }
        }
    }
    let _wipe = Wipe(out, out_size);

    let mut name = [0u16; 514];
    let mut domain = [0u16; 338];
    let mut pass = [0u16; 514];
    let (mut nl, mut dl, mut pl) = (name.len() as u32, domain.len() as u32, pass.len() as u32);
    // CRED_PACK_PROTECTED_CREDENTIALS: el diálogo entrega la contraseña cifrada; con esta
    // bandera Windows la descifra para poder validarla.
    let unpacked = unsafe {
        CredUnPackAuthenticationBufferW(
            CRED_PACK_PROTECTED_CREDENTIALS,
            out,
            out_size,
            Some(PWSTR(name.as_mut_ptr())),
            &mut nl,
            Some(PWSTR(domain.as_mut_ptr())),
            Some(&mut dl),
            Some(PWSTR(pass.as_mut_ptr())),
            &mut pl,
        )
    };
    let result = (|| {
        unpacked.map_err(|e| e.message())?;
        // "EQUIPO\usuario" o "usuario@dominio": LogonUser quiere el dominio por separado.
        let full = String::from_utf16_lossy(&name[..nl.saturating_sub(1) as usize]);
        let given_domain = String::from_utf16_lossy(&domain[..dl.saturating_sub(1) as usize]);
        let (dom, usr) = match full.split_once('\\') {
            Some((d, u)) => (d.to_string(), u.to_string()),
            None => (given_domain, full.clone()),
        };
        let wusr = HSTRING::from(usr.as_str());
        let wdom = HSTRING::from(dom.as_str());
        let mut token = HANDLE::default();
        let ok = unsafe {
            LogonUserW(
                PCWSTR(wusr.as_ptr()),
                if dom.is_empty() { PCWSTR::null() } else { PCWSTR(wdom.as_ptr()) },
                PCWSTR(pass.as_ptr()),
                LOGON32_LOGON_INTERACTIVE,
                LOGON32_PROVIDER_DEFAULT,
                &mut token,
            )
        };
        if let Err(e) = ok {
            // Diagnóstico sin datos sensibles (nunca la contraseña).
            eprintln!("[verify] LogonUser rechazó a {dom}\\{usr}: {}", e.message());
            return Ok(Outcome::Rejected);
        }
        // Tiene que ser la misma cuenta que corre la app: se compara la identidad (SID), no el nombre.
        let me = current_token();
        let same = token_sid(token).is_some() && token_sid(token) == token_sid(me);
        unsafe {
            let _ = CloseHandle(me);
        }
        unsafe {
            let _ = CloseHandle(token);
        }
        if !same {
            eprintln!("[verify] la cuenta validada no es la de la sesión");
        }
        Ok(if same { Outcome::Verified } else { Outcome::Rejected })
    })();
    let _ = &user;
    // Contraseña fuera de la memoria.
    unsafe { std::ptr::write_volatile(&mut pass, [0u16; 514]) };
    result
}

fn current_token() -> windows::Win32::Foundation::HANDLE {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Security::TOKEN_QUERY;
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    let mut h = HANDLE::default();
    let _ = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut h) };
    h
}

/// SID del usuario de un token, como texto ("S-1-5-21-…"). None si no se puede leer.
fn token_sid(token: windows::Win32::Foundation::HANDLE) -> Option<String> {
    use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows::Win32::Security::{GetTokenInformation, TOKEN_USER, TokenUser};
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::core::PWSTR;
    if token.is_invalid() {
        return None;
    }
    let mut size = 0u32;
    let _ = unsafe { GetTokenInformation(token, TokenUser, None, 0, &mut size) };
    let mut buf = vec![0u8; size as usize];
    unsafe { GetTokenInformation(token, TokenUser, Some(buf.as_mut_ptr().cast()), size, &mut size) }.ok()?;
    let user = unsafe { &*(buf.as_ptr() as *const TOKEN_USER) };
    let mut s = PWSTR::null();
    unsafe { ConvertSidToStringSidW(user.User.Sid, &mut s) }.ok()?;
    let out = unsafe { s.to_string() }.ok();
    unsafe {
        let _ = LocalFree(Some(HLOCAL(s.0.cast())));
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn sid_de_la_sesion() {
        let t = super::current_token();
        let sid = super::token_sid(t).expect("SID");
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(t);
        }
        assert!(sid.starts_with("S-1-5-"), "{sid}");
    }

    /// Solo consulta si Windows Hello está disponible (no muestra nada).
    #[test]
    #[ignore]
    fn real_disponibilidad_hello() {
        use windows::Security::Credentials::UI::UserConsentVerifier;
        let a = UserConsentVerifier::CheckAvailabilityAsync().unwrap().join().unwrap();
        println!("Windows Hello: {:?}", a.0);
    }
}
