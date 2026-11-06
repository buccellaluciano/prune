//! Información del sistema y estado de elevación (solo lectura).

use crate::catalog::Hive;
use crate::registry as reg;
use serde::Serialize;
use windows::core::PWSTR;
use windows::Win32::System::RemoteDesktop::{
    WTSFreeMemory, WTSQuerySessionInformationW, WTSUserName, WTS_CURRENT_SERVER_HANDLE, WTS_CURRENT_SESSION,
};
use windows::Win32::UI::Shell::IsUserAnAdmin;

#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    pub product: String,
    pub display_version: String,
    pub build: u32,
    pub ubr: u32,
    pub edition: String,
    pub elevated: bool,
    /// Si la app corre con otra cuenta (p. ej. un usuario estándar que elevó con las
    /// credenciales de un admin), el usuario real de la sesión. HKCU y las apps Appx
    /// serían los de la cuenta admin, no los de esta persona.
    pub foreign_user: Option<String>,
}

/// Usuario dueño de la sesión de Windows en la que corre la app.
fn session_user() -> Option<String> {
    unsafe {
        let mut buf = PWSTR::null();
        let mut len = 0u32;
        WTSQuerySessionInformationW(Some(WTS_CURRENT_SERVER_HANDLE), WTS_CURRENT_SESSION, WTSUserName, &mut buf, &mut len)
            .ok()?;
        let name = buf.to_string().ok();
        WTSFreeMemory(buf.0 as _);
        name.filter(|n| !n.is_empty())
    }
}

pub fn foreign_user() -> Option<String> {
    let session = session_user()?;
    let me = crate::user::username()?;
    (!session.eq_ignore_ascii_case(&me)).then_some(session)
}

const NT_CV: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

fn read_text(name: &str) -> String {
    reg::read(Hive::HKLM, NT_CV, name)
        .ok()
        .flatten()
        .map(|v| reg::describe(Some(&v)).trim_matches('"').to_string())
        .unwrap_or_default()
}

pub fn is_elevated() -> bool {
    unsafe { IsUserAnAdmin().as_bool() }
}

pub fn info() -> SystemInfo {
    let build: u32 = read_text("CurrentBuild").parse().unwrap_or(0);
    let ubr: u32 = read_text("UBR").parse().unwrap_or(0);
    // ProductName sigue diciendo "Windows 10" en Windows 11: lo corregimos por build.
    let mut product = read_text("ProductName");
    if build >= 22000 {
        product = product.replacen("Windows 10", "Windows 11", 1);
    }
    SystemInfo {
        product,
        display_version: read_text("DisplayVersion"),
        build,
        ubr,
        edition: read_text("EditionID"),
        elevated: is_elevated(),
        foreign_user: foreign_user(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_account_is_not_foreign() {
        // Corriendo con la cuenta de la sesión, no debe detectarse "otra cuenta".
        assert!(session_user().is_some());
        assert_eq!(foreign_user(), None);
    }
}
