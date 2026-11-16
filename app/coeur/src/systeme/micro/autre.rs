//! Hors Windows : aucun micro.

use super::{Erreur, Origine};

/// Le rappel du son, du fil `film-son`.
pub type Rappel = Box<dyn FnMut(&[i16]) + Send + 'static>;

pub fn present(_origine: Origine) -> bool {
    false
}

/// Aucun micro ne peut exister ici : le type n'a aucune valeur.
#[derive(Debug)]
pub enum Micro {}

pub fn ouvrir(_rappel: Rappel, _origine: Origine) -> Result<Micro, Erreur> {
    Err(Erreur::Absent)
}

pub fn perdu(m: &Micro) -> bool {
    match *m {}
}
