//! Le texte de la licence, gardé tel que collé dans `<dossier des réglages>/licence.txt`
//! (avenant 0.5 « Licence », §3.1), écrit de façon atomique comme les réglages.
//!
//! Socle : inerte (chantier L pour les corps).

/// Le texte rangé, `None` s'il n'y en a pas ou s'il est illisible.
pub fn lire_licence() -> Option<String> {
    None
}

/// Range `texte` (remplace l'ancien).
pub fn ecrire_licence(_texte: &str) -> Result<(), String> {
    Err("non construit".into())
}

/// Efface le texte rangé (absent : pas une erreur).
pub fn effacer_licence() -> Result<(), String> {
    Ok(())
}
