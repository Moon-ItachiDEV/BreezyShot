//! Hors Windows : pas de Media Foundation, pas de vidéo.

use std::path::Path;
use std::time::Duration;

use super::{Codeur, Lecture, Parametres, Pixels};

const NON_DISPONIBLE: &str = "video non disponible sur ce systeme";

pub fn bibliotheques() -> bool {
    false
}

pub fn codeur_h264_present() -> bool {
    false
}

/// Aucun écrivain ne peut exister ici : le type n'a aucune valeur.
#[derive(Debug)]
pub enum Ecrivain {}

pub fn creer(_chemin: &Path, _p: &Parametres) -> Result<Ecrivain, String> {
    Err(NON_DISPONIBLE.into())
}

pub fn codeur(e: &Ecrivain) -> Codeur {
    match *e {}
}

pub fn image(e: &mut Ecrivain, _pixels: &Pixels, _instant: Duration) -> Result<Duration, String> {
    match *e {}
}

pub fn meme_image(e: &mut Ecrivain, _instant: Duration) -> Result<Duration, String> {
    match *e {}
}

pub fn son(e: &mut Ecrivain, _pcm: &[i16], _instant: Duration) -> Result<(), String> {
    match *e {}
}

pub fn finir(e: Ecrivain) -> Result<Duration, String> {
    match e {}
}

pub fn abandonner(e: Ecrivain) {
    match e {}
}

pub fn lire(
    _chemin: &Path,
    _voir: &mut dyn FnMut(u64, Duration, &Pixels),
) -> Result<Lecture, String> {
    Err(NON_DISPONIBLE.into())
}
