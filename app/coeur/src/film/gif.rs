//! L'écriture du GIF au fil de l'eau (F19), par la crate `gif` : GIF89a, boucle infinie ;
//! la première image entière et opaque ; ensuite le seul rectangle changé, ses pixels
//! inchangés transparents, une palette locale (`Frame::from_rgba_speed`, vitesse 10), et
//! l'image précédente gardée dessous (`DisposalMethod::Keep`). Une image identique à la
//! précédente n'est pas écrite : elle allonge le délai de la précédente. Délais en
//! centièmes de seconde, sur les temps réels, avec report de l'arrondi. Jamais tout le GIF
//! en mémoire.
//!
//! Socle : signatures, corps inertes (chantier D).

use std::io::Write;
use std::time::Duration;

use super::NON_CONSTRUIT;
use crate::systeme::gel::Pixels;

/// Un GIF en cours d'écriture dans `W` (le fichier de travail, dans le fil `film-gif`).
#[derive(Debug)]
pub struct Gif<W: Write> {
    _sortie: W,
}

impl<W: Write> Gif<W> {
    /// Commence un GIF de `largeur` × `hauteur` pixels dans `sortie` ; `vitesse` est celle
    /// de la quantification de la crate (10, D13). Socle : refusé.
    pub fn nouveau(sortie: W, largeur: u16, hauteur: u16, vitesse: i32) -> Result<Gif<W>, String> {
        let _ = (sortie, largeur, hauteur, vitesse);
        Err(NON_CONSTRUIT.into())
    }

    /// L'image du temps `instant` du film. Faux : image identique à la précédente, rien
    /// d'écrit (F19). Socle : refusé.
    pub fn image(&mut self, pixels: &Pixels, instant: Duration) -> Result<bool, String> {
        let _ = (pixels, instant);
        Err(NON_CONSTRUIT.into())
    }

    /// Écrit l'image en attente avec le délai qui mène à `duree`, puis la fin du GIF ; rend
    /// la sortie. Socle : refusé.
    pub fn finir(self, duree: Duration) -> Result<W, String> {
        let _ = duree;
        Err(NON_CONSTRUIT.into())
    }
}

/// Le délai d'une image, en centièmes de seconde, de `depuis` à `jusqu_a` (temps du film),
/// avec report de l'arrondi : la somme des délais égale la durée à un centième près.
/// Socle : 0.
pub fn delai_cs(depuis: Duration, jusqu_a: Duration) -> u16 {
    let _ = (depuis, jusqu_a);
    0
}

/// F19 : aucun pixel opaque à l'indice `transparent` ; un pixel opaque (alpha non nul dans
/// `alpha`) qui l'a reçu est ramené à l'indice opaque le plus proche de `palette` (RGB).
/// Socle : rien n'est changé.
pub fn opaques_hors_du_transparent(
    indices: &mut [u8],
    alpha: &[u8],
    palette: &[u8],
    transparent: u8,
) {
    let _ = (indices, alpha, palette, transparent);
}
