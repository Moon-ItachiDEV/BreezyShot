//! Hors Windows : pas de démarrage avec le système pour l'instant.

use super::EtatDemarrage;

pub fn lire(_run: &str, _approuve: &str, _nom: &str) -> EtatDemarrage {
    EtatDemarrage::Indisponible
}

pub fn ecrire(_cle: &str, _nom: &str, _texte: &str) -> Result<(), String> {
    Err("non disponible".into())
}

pub fn supprimer(_cle: &str, _nom: &str) -> Result<(), String> {
    Err("non disponible".into())
}
