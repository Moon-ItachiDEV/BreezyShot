//! WASAPI sous Windows (chantier C). Socle : aucun micro n'est vu ni ouvert.

use super::Erreur;

/// Le rappel du son, du fil `film-son`.
pub type Rappel = Box<dyn FnMut(&[i16]) + Send + 'static>;

pub fn present() -> bool {
    false
}

/// Aucun micro ne peut exister ici : le type n'a aucune valeur.
#[derive(Debug)]
pub enum Micro {}

pub fn ouvrir(_rappel: Rappel) -> Result<Micro, Erreur> {
    Err(Erreur::Absent)
}

pub fn perdu(m: &Micro) -> bool {
    match *m {}
}
