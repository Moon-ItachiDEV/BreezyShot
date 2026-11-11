//! Réponses neutres hors Windows (Linux pour le développement ; macOS en attendant macos.rs).

pub type RappelTheme = Box<dyn Fn(super::Theme) + Send + 'static>;

pub fn valeur_theme_barre() -> Option<u32> {
    None
}

pub fn surveiller_theme_barre(_rappel: RappelTheme) -> Result<(), String> {
    Ok(())
}

pub fn echelle_par_ecran_v2() -> Option<bool> {
    None
}

pub fn version_windows() -> Option<u32> {
    None
}

/// `None` : pas de touche réservée ici.
pub fn lecture_impr_ecran() -> Option<(Option<u32>, Option<u32>)> {
    None
}

pub fn animations_reduites() -> Option<bool> {
    None
}

pub fn valeur_transparence() -> Option<u32> {
    None
}
