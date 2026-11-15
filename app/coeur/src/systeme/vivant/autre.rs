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

// ---------------------------------------------------------------------------------------
// Source d'un film (0.7). Hors Windows : jamais.

/// Aucune source ne peut exister ici : le type n'a aucune valeur.
#[derive(Debug)]
pub enum Source {}

pub fn ouvrir_source(_zone: Rect, _ecran: Rect) -> Result<Source, String> {
    Err("images vivantes non disponibles sur ce systeme".into())
}

pub fn moyen_source(source: &Source) -> super::Moyen {
    match *source {}
}

pub fn zone_source(source: &Source) -> Rect {
    match *source {}
}

pub fn prendre_source(
    source: &mut Source,
    _dest: &mut Pixels,
) -> Result<(super::Image, Duration), String> {
    match *source {}
}

pub fn replier_source(source: &mut Source) -> Result<(), String> {
    match *source {}
}

pub fn repli_source(source: &mut Source) -> Option<(super::Moyen, String)> {
    match *source {}
}
