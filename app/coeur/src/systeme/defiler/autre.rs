//! Hors Windows : rien ne défile (Linux pour le développement ; macOS : la capture
//! défilante y est absente).

use super::{Depart, Envoi, FenetreInfo, Moyen, PageTrouvee, Position, RappelMolette, Rect};

pub fn preparer_pages() {}

pub fn chercher_pages(
    _fenetres: &[FenetreInfo],
    _max: usize,
    _depart: Depart,
    rappel: &mut dyn FnMut(&[PageTrouvee]),
) {
    rappel(&[]);
}

/// Aucun défileur ne peut exister ici : le type n'a aucune valeur.
#[derive(Debug)]
pub enum Defileur {}

pub fn preparer(_zone: Rect, _fenetre: Option<isize>) -> Result<Defileur, String> {
    Err("defilement non disponible sur ce systeme".into())
}

pub fn moyen(d: &Defileur) -> Option<Moyen> {
    match *d {}
}

pub fn envoyes(d: &Defileur) -> u32 {
    match *d {}
}

pub fn abandonner(d: &mut Defileur) -> Option<Moyen> {
    match *d {}
}

pub fn constater(d: &mut Defileur, _avance: u32) {
    match *d {}
}

pub fn avance_par_cran(d: &Defileur) -> Option<u32> {
    match *d {}
}

pub fn descendre(d: &mut Defileur, _pixels: i32) -> Result<Envoi, String> {
    match *d {}
}

pub fn position(d: &Defileur) -> Option<Position> {
    match *d {}
}

/// Aucune écoute ne peut exister ici.
#[derive(Debug)]
pub enum Ecoute {}

pub fn ecouter_molette(_rappel: RappelMolette) -> Result<Ecoute, String> {
    Err("ecoute de la molette non disponible sur ce systeme".into())
}

pub fn active(e: &Ecoute) -> bool {
    match *e {}
}
