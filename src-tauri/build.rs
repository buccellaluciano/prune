fn main() {
    let mut attrs = tauri_build::Attributes::new();
    // En release el exe pide administrador al iniciar (UAC).
    // En debug no, para que `tauri dev` funcione desde una terminal normal:
    // la UI avisa que los tweaks de HKLM quedan deshabilitados.
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        attrs = attrs.windows_attributes(
            tauri_build::WindowsAttributes::new().app_manifest(include_str!("app.manifest")),
        );
    }
    tauri_build::try_build(attrs).expect("error en tauri-build");
}
