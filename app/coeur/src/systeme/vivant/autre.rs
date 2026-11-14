//! Hors Windows : pas d'images vivantes (Linux pour le développement ; macOS : la capture
//! défilante y est absente).

use std::time::Duration;

use super::{Pixels, Rect};

pub fn disponible() -> bool {
    false
}

/// Aucune prise ne peut exister ici : le type n'a aucune valeur.
#[derive(Debug)]
pub enum Prise {}

pub fn ouvrir(_zone: Rect) -> Result<Prise, String> {
    Err("images vivantes non disponibles sur ce systeme".into())
}

pub fn zone(prise: &Prise) -> Rect {
    match *prise {}
}

pub fn prendre(prise: &mut Prise, _dest: &mut Pixels) -> Result<Duration, String> {
    match *prise {}
}
