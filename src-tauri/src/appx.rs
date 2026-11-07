//! Bloatware: listar y quitar paquetes Appx del catálogo vía WinRT PackageManager.
//! Solo se pueden quitar apps que estén en `catalog/bloat.toml`.

use crate::catalog::{self, Risk};
use serde::Serialize;
use windows::ApplicationModel::Package;
use windows::Management::Deployment::PackageManager;
use windows::core::HSTRING;

#[derive(Debug, Clone, Serialize)]
pub struct InstalledPkg {
    pub full_name: String,
    pub family_name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BloatView {
    pub id: String,
    pub title: String,
    pub tier: Risk,
    pub note: Option<String>,
    pub packages: Vec<InstalledPkg>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoveResult {
    pub id: String,
    pub ok: bool,
    pub removed: Vec<String>,
    pub error: Option<String>,
}

struct Pkg {
    name: String,
    info: InstalledPkg,
}

fn user_packages(pm: &PackageManager) -> windows::core::Result<Vec<Pkg>> {
    let mut out = Vec::new();
    // SID vacío = usuario actual.
    for p in pm.FindPackagesByUserSecurityId(&HSTRING::new())? {
        if let Some(pkg) = describe(&p) {
            out.push(pkg);
        }
    }
    Ok(out)
}

fn describe(p: &Package) -> Option<Pkg> {
    if p.IsFramework().unwrap_or(true) || p.IsResourcePackage().unwrap_or(true) {
        return None;
    }
    let id = p.Id().ok()?;
    let v = id.Version().ok()?;
    Some(Pkg {
        name: id.Name().ok()?.to_string(),
        info: InstalledPkg {
            full_name: id.FullName().ok()?.to_string(),
            family_name: id.FamilyName().ok()?.to_string(),
            version: format!("{}.{}.{}.{}", v.Major, v.Minor, v.Build, v.Revision),
        },
    })
}

pub fn list() -> Result<Vec<BloatView>, String> {
    let pm = PackageManager::new().map_err(|e| e.to_string())?;
    let installed = user_packages(&pm).map_err(|e| e.to_string())?;
    Ok(catalog::bloat()
        .app
        .iter()
        .filter_map(|a| {
            let packages: Vec<_> = installed
                .iter()
                .filter(|p| a.matches(&p.name))
                .map(|p| p.info.clone())
                .collect();
            (!packages.is_empty()).then(|| BloatView {
                id: a.name.clone(),
                title: crate::i18n::bloat_title(a),
                tier: a.tier,
                note: crate::i18n::bloat_note(a),
                packages,
            })
        })
        .collect())
}

pub fn remove(ids: &[String], deprovision: bool) -> Result<Vec<RemoveResult>, String> {
    let pm = PackageManager::new().map_err(|e| e.to_string())?;
    let installed = user_packages(&pm).map_err(|e| e.to_string())?;
    let mut results = Vec::new();
    for id in ids {
        let Some(app) = catalog::bloat().app.iter().find(|a| &a.name == id) else {
            results.push(RemoveResult { id: id.clone(), ok: false, removed: vec![], error: Some(t!("not_in_catalog")) });
            continue;
        };
        let mut removed = Vec::new();
        let mut error = None;
        for p in installed.iter().filter(|p| app.matches(&p.name)) {
            let r = pm
                .RemovePackageAsync(&HSTRING::from(&p.info.full_name))
                .and_then(|op| op.join());
            match r {
                Ok(_) => {
                    removed.push(p.info.full_name.clone());
                    if deprovision {
                        // Evita que se reinstale en cuentas nuevas. Si no estaba
                        // aprovisionado, el error se ignora.
                        let _ = pm
                            .DeprovisionPackageForAllUsersAsync(&HSTRING::from(&p.info.family_name))
                            .and_then(|op| op.join());
                    }
                }
                Err(e) => {
                    error = Some(e.message().to_string());
                    break;
                }
            }
        }
        results.push(RemoveResult { id: id.clone(), ok: error.is_none(), removed, error });
    }
    Ok(results)
}
