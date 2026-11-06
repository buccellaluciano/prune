// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Las DLLs que se carguen de acá en más solo se buscan en System32, nunca en la carpeta
    // del exe ni en el directorio actual (evita que alguien "plante" una DLL con nombre
    // conocido). Las que Windows carga al abrir el exe las cubre /DEPENDENTLOADFLAG (build.rs).
    unsafe {
        use windows::Win32::System::LibraryLoader::{SetDefaultDllDirectories, LOAD_LIBRARY_SEARCH_SYSTEM32};
        let _ = SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32);
    }
    debloater_lib::run()
}
