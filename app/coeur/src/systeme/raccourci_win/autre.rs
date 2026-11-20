//! Hors Windows : `Win + Maj + S` est sans objet.

use super::Rappel;

#[derive(Debug)]
pub struct Ecoute;

pub fn disponible() -> bool {
    false
}

pub fn ecouter(_rappel: Rappel) -> Result<Ecoute, String> {
    Err("hors Windows".into())
}
