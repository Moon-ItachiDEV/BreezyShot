fn main() {
    // Manifeste Windows : contrôles communs v6 (comme Tauri par défaut) et conscience
    // d'échelle de chaque écran (Per-Monitor V2) déclarée dès le lancement.
    let windows =
        tauri_build::WindowsAttributes::new().app_manifest(include_str!("manifeste-windows.xml"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("echec du script de construction Tauri");
}
