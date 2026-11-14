//! Hors Windows : réponses neutres (Linux pour le développement ; macOS viendra ensuite).

use std::sync::Arc;
use std::time::Duration;

use super::{Changement, EcranInfo, FenetreInfo, Pixels, Pointeur, Rect};

pub const DISPONIBLE: bool = false;

/// Aucune image ne peut exister ici : le type n'a aucune valeur.
#[derive(Debug)]
pub enum Image {}

pub fn rect_image(image: &Image) -> Rect {
    match *image {}
}

pub fn duree_image(image: &Image) -> Duration {
    match *image {}
}

pub fn lire(image: &Image, _zone: Rect) -> Result<Pixels, String> {
    match *image {}
}

pub fn figer_bureau() -> Result<Image, String> {
    Err("gel non disponible sur ce systeme".into())
}

pub fn liberer_image() {}

pub fn ecrans() -> Vec<EcranInfo> {
    Vec::new()
}

pub fn fenetres_visibles() -> Vec<FenetreInfo> {
    Vec::new()
}

pub fn preparer_ecran_fige() -> Result<(), String> {
    Ok(())
}

pub fn montrer_ecran_fige(image: &Image) -> Result<(), String> {
    match *image {}
}

pub fn cacher_ecran_fige() {}

pub fn detruire_ecran_fige() {}

pub fn garder_ecran_fige(_delai: Duration, _au_blocage: Box<dyn Fn() + Send + 'static>) {}

pub fn quand_bureau_change(_rappel: Arc<dyn Fn(Changement) + Send + Sync + 'static>) {}

pub fn surveillance_session() -> bool {
    false
}

pub fn verrouillages_de_session() -> u64 {
    0
}

pub fn mises_en_veille() -> u64 {
    0
}

pub fn quand_session_finit(_rappel: Box<dyn Fn() + Send + Sync>) {}

pub fn poignee_ecran_fige() -> Option<isize> {
    None
}

pub fn ecran_fige_montre() -> bool {
    false
}

pub fn bureau_virtuel() -> Result<Rect, String> {
    Err("gel non disponible sur ce systeme".into())
}

pub fn lire_pointeur() -> Result<Option<Pointeur>, String> {
    Ok(None)
}
