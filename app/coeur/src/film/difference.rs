//! Ce qui change d'une image à l'autre (F19) et la réduction de moitié d'une image trop
//! grande pour le codeur (F4, point 3). Pixels RGBA du cœur (`gel::Pixels`).
//!
//! Socle : signatures, corps inertes (chantier D).

use crate::systeme::gel::{Pixels, Rect};

/// Le plus petit rectangle (pixels de l'image) qui contient les pixels changés entre
/// `avant` et `apres`, de même taille ; `None` : rien n'a changé. Socle : `None`.
pub fn rectangle_change(avant: &Pixels, apres: &Pixels) -> Option<Rect> {
    let _ = (avant, apres);
    None
}

/// Les pixels de `r` dans `apres`, en RGBA ; alpha 0 pour un pixel égal à celui d'`avant`.
/// Socle : vide.
pub fn sous_image(avant: &Pixels, apres: &Pixels, r: Rect) -> Vec<u8> {
    let _ = (avant, apres, r);
    Vec::new()
}

/// `pixels` réduits de moitié sur chaque côté (moyenne de quatre pixels) dans `dest`,
/// réutilisé (F4, point 3). Socle : `dest` vidé.
pub fn moitie(pixels: &Pixels, dest: &mut Pixels) {
    let _ = pixels;
    dest.largeur = 0;
    dest.hauteur = 0;
    dest.rgba.clear();
}
