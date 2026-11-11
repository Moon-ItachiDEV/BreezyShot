fn main() {
    // Manifeste Windows : contrôles communs v6 (comme Tauri par défaut) et conscience
    // d'échelle de chaque écran (Per-Monitor V2) déclarée dès le lancement.
    let windows =
        tauri_build::WindowsAttributes::new().app_manifest(include_str!("manifeste-windows.xml"));
    // Commandes de l'app : déclarées ici, elles ne sont permises qu'aux fenêtres que
    // capabilities/ nomme (sinon toute page pourrait les appeler).
    let commandes = tauri_build::AppManifest::new().commands(&[
        "bandeau_pret",
        "bandeau_zone",
        "bandeau_action",
        "bandeau_termine",
        "selection_chargee",
        "selection_prete",
        "selection_terminee",
        "selection_annulee",
    ]);
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .windows_attributes(windows)
            .app_manifest(commandes),
    )
    .expect("echec du script de construction Tauri");
}
