//! Información del sistema y estado de elevación (solo lectura).

use crate::catalog::Hive;
use crate::registry as reg;
use serde::Serialize;
use windows::Win32::UI::Shell::IsUserAnAdmin;

#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    pub product: String,
    pub display_version: String,
    pub build: u32,
    pub ubr: u32,
    pub edition: String,
    pub elevated: bool,
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
    }
}
