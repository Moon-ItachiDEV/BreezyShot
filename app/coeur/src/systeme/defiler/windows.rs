//! Défiler une autre app sous Windows (chantier B, 0.6) : pages sous un clic (fenêtres
//! filles, classe de rendu de Chromium, barre verticale ; UI Automation en complément),
//! `WM_MOUSEWHEEL` posté, UI Automation `Scroll(SmallIncrement)`, molette `SendInput`
//! marquée, position lue, crochet `WH_MOUSE_LL` dans le fil `defilante-molette`.
//!
//! Socle : corps inertes (aucune page, rien ne défile, aucune écoute).

use super::{Envoi, FenetreInfo, Moyen, PageTrouvee, Position, RappelMolette, Rect};

pub fn pages(_fenetres: &[FenetreInfo], _max: usize) -> Vec<PageTrouvee> {
    Vec::new()
}

/// Le défileur d'une capture (à remplir par le chantier B : zone, cible, moyen en cours,
/// élément UI Automation).
#[derive(Debug)]
pub struct Defileur {
    moyen: Option<Moyen>,
}

pub fn preparer(_zone: Rect, _fenetre: Option<isize>) -> Result<Defileur, String> {
    let _ = Defileur { moyen: None };
    Err("defilement non construit".into())
}

pub fn moyen(d: &Defileur) -> Option<Moyen> {
    d.moyen
}

pub fn abandonner(d: &mut Defileur) -> Option<Moyen> {
    d.moyen = None;
    None
}

pub fn descendre(_d: &mut Defileur, _pixels: i32) -> Result<Envoi, String> {
    Ok(Envoi::Saute)
}

pub fn position(_d: &Defileur) -> Option<Position> {
    None
}

/// L'écoute de la molette (à remplir par le chantier B : crochet, fil, son arrêt).
#[derive(Debug)]
pub struct Ecoute {
    active: bool,
}

pub fn ecouter_molette(rappel: RappelMolette) -> Result<Ecoute, String> {
    drop(rappel);
    let _ = Ecoute { active: false };
    Err("ecoute de la molette non construite".into())
}

pub fn active(e: &Ecoute) -> bool {
    e.active
}
