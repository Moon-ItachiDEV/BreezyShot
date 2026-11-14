fn main() {
    // comctl32 chargé à la demande (MSVC) : les exécutables de tests n'ont pas le manifeste
    // des contrôles communs v6, et TaskDialogIndirect (importé par une dépendance) n'existe
    // pas dans la version 5 que Windows charge sans lui : l'exécutable ne démarrait pas
    // (STATUS_ENTRYPOINT_NOT_FOUND). Le .exe de l'app garde son manifeste : rien n'y change.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg=/DELAYLOAD:comctl32.dll");
        println!("cargo:rustc-link-arg=delayimp.lib");
    }
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
        "selection_cliquee",
        "selection_annulee",
        "reglages_lire",
        "reglages_changer",
        "reglages_apercu_nom",
        "reglages_parcourir",
        "reglages_saisie",
        "reglages_raccourci",
        "reglages_retablir",
        "vignette_pret",
        "vignette_image",
        "vignette_zone",
        "vignette_entree",
        "vignette_action",
        "vignette_glisser",
        "vignette_terminee",
        "souffle_pret",
        "souffle_termine",
        "editeur_pret",
        "editeur_image",
        "editeur_affiche",
        "editeur_vue",
        "editeur_etat",
        "editeur_exporter",
        "editeur_fermer",
        "editeur_garder",
        "editeur_copier_texte",
        "historique_pret",
        "historique_apercu",
        "historique_action",
        "historique_annuler",
        "historique_tout_effacer",
        "historique_glisser",
        "historique_affiche",
        "historique_fermer",
        "licence_etat",
        "licence_activer",
        "licence_retirer",
        "defilante_pret",
        "defilante_zone",
        "defilante_terminer",
        "defilante_apercu",
        "installateur_pret",
        "installateur_action",
        "menu_pret",
        "menu_choisi",
        "menu_fermer",
    ]);
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .windows_attributes(windows)
            .app_manifest(commandes),
    )
    .expect("echec du script de construction Tauri");
}
