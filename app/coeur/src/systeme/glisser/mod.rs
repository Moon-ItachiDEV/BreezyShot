//! Glisser-déposer d'un fichier vers une autre application (étape 9, vignette).
//!
//! Façade commune : `windows.rs` donne le fichier au système par `SHDoDragDrop`, avec
//! l'objet de données que le Shell fabrique pour ce fichier (les mêmes formats que
//! l'Explorateur) et la source par défaut du système (bouton principal, Échap) ; `autre.rs`
//! répond « non disponible » ailleurs. Rien ici ne décide : l'Application interprète le
//! résultat (`application::vignette::depot`).

use std::path::Path;

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Ce que le système dit à la fin d'un glisser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depot {
    /// Une cible a accepté le fichier (en copie : elle ne peut ni le déplacer ni l'effacer).
    Depose,
    /// Échap, lâché sur rien, ou cible qui refuse.
    Annule,
}

/// Lance le glisser du fichier `chemin` depuis la fenêtre `fenetre` (identifiant système)
/// et attend sa fin. Fil principal seulement : le système y fait tourner sa propre boucle
/// de messages tant que le bouton est tenu. Le bouton principal doit être enfoncé (sinon
/// le système déposerait tout de suite sous la souris) : voir `bouton_principal_enfonce`.
pub fn glisser_fichier(fenetre: isize, chemin: &Path) -> Result<Depot, String> {
    plateforme::glisser_fichier(fenetre, chemin)
}

/// Un bouton physique de la souris.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoutonPhysique {
    Gauche,
    Droit,
}

/// Le bouton physique qui fait le clic principal : le droit quand les boutons sont
/// inversés (réglage « gaucher » du système), sinon le gauche.
pub fn bouton_principal(boutons_inverses: bool) -> BoutonPhysique {
    if boutons_inverses {
        BoutonPhysique::Droit
    } else {
        BoutonPhysique::Gauche
    }
}

/// Vrai si le bouton principal de la souris (celui du glisser) est enfoncé en ce moment.
pub fn bouton_principal_enfonce() -> bool {
    plateforme::bouton_enfonce(bouton_principal(plateforme::boutons_inverses()))
}

/// La fenêtre de premier niveau sous un point (pixels réels du bureau), `None` s'il n'y en
/// a pas ou hors Windows.
pub fn fenetre_sous(point: (i32, i32)) -> Option<isize> {
    plateforme::fenetre_sous(point)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bouton_principal_suit_l_inversion() {
        assert_eq!(bouton_principal(false), BoutonPhysique::Gauche);
        assert_eq!(bouton_principal(true), BoutonPhysique::Droit);
    }
}
