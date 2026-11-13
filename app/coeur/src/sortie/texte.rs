//! Le texte vers le presse-papiers (contrat 0.5, §3.1, R6) : une voie a part de `sortir`,
//! qui est pour les pixels.
//!
//! Socle : signature (chantier A pour le corps).

use super::Etape;

/// Pose `texte` dans le presse-papiers (`arboard`, 250 ms d'essais).
pub fn copier_texte(_texte: &str) -> Etape {
    Etape::Echec("non construit".into())
}
