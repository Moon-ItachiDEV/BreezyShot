//! Images vivantes sous Windows (chantier A, 0.6) : `BitBlt` du DC de l'écran sur le seul
//! rectangle de la zone, `SRCCOPY` sans `CAPTUREBLT`, dans un DIB réutilisé (R2).
//!
//! Socle : corps inertes. `disponible` reste faux tant que `prendre` n'est pas fait : la
//! capture défilante reste absente.

use std::time::Duration;

use super::{Pixels, Rect};

pub fn disponible() -> bool {
    false
}

/// Le tampon de la prise (à remplir par le chantier A : DC mémoire, DIB, sa taille).
#[derive(Debug)]
pub struct Prise {
    zone: Rect,
}

pub fn ouvrir(zone: Rect) -> Result<Prise, String> {
    let _ = Prise { zone };
    Err("images vivantes non construites".into())
}

pub fn zone(prise: &Prise) -> Rect {
    prise.zone
}

pub fn prendre(_prise: &mut Prise, _dest: &mut Pixels) -> Result<Duration, String> {
    Err("images vivantes non construites".into())
}
