//! Masquage temporaire des icônes du bureau pendant une capture plein écran ou un film.
//!
//! Façade commune : `windows.rs` agit sur la liste d'icônes d'Explorer (`SysListView32`
//! sous `SHELLDLL_DefView`) ; `autre.rs` ne fait rien ailleurs. Si les icônes étaient déjà
//! cachées par l'utilisateur, la garde n'y touche pas et ne les réaffiche pas.

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Garde RAII : tant qu'elle vit, les icônes du bureau masquées ici restent cachées ; sa
/// destruction les réaffiche automatiquement (capture finie, annulée, erreur ou arrêt).
#[derive(Debug)]
#[must_use]
pub struct GardeIcones {
    fenetre: Option<isize>,
}

impl GardeIcones {
    /// Vrai si cette garde a effectivement masqué les icônes (et devra les restaurer).
    pub fn actif(&self) -> bool {
        self.fenetre.is_some()
    }
}

impl Drop for GardeIcones {
    fn drop(&mut self) {
        if let Some(fenetre) = self.fenetre.take() {
            plateforme::restaurer(fenetre);
            log::info!("bureau: icones restaurees");
        }
    }
}

/// Masque les icônes du bureau si elles sont visibles. Rend une garde qui les restaure à
/// sa destruction ; inactive si elles étaient déjà cachées ou hors Windows.
pub fn masquer_icones() -> GardeIcones {
    let fenetre = plateforme::masquer();
    if fenetre.is_some() {
        log::info!("bureau: icones masquees");
    }
    GardeIcones { fenetre }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garde_inactive_n_est_pas_active() {
        let g = GardeIcones { fenetre: None };
        assert!(!g.actif());
        drop(g);
    }
}
