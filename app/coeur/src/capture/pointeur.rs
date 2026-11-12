//! Le pointeur posé dans les pixels découpés (0.4) : fonctions pures.
//!
//! Le pointeur est lu deux fois par `systeme::gel::lire_pointeur`, peint sur du noir puis
//! sur du blanc ; la comparaison des deux dit, pixel par pixel, ce qu'il fait à l'image :
//! rien, une couleur opaque, un mélange (ombre, bord adouci) ou une inversion (curseur
//! texte). Socle : les signatures seules ; le chantier « Pointeur et loupe » écrit les corps.

use crate::systeme::gel::{Pixels, Pointeur};
use crate::systeme::Rect;

/// Ce qu'un pixel du pointeur fait au pixel de l'image dessous.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Couche {
    /// Rien : le pixel de l'image reste tel quel.
    Transparent,
    /// Le pixel prend cette couleur.
    Opaque([u8; 3]),
    /// Cette couleur, mêlée à l'image avec cette opacité.
    Mele { rgb: [u8; 3], alpha: u8 },
    /// Le pixel de l'image est inversé (XOR).
    Inverse,
}

/// La couche d'un pixel, d'après sa couleur sur noir et sur blanc : égales → opaque ;
/// 0 sur noir et 255 sur blanc → rien ; 255 sur noir et 0 sur blanc → inverse ; sinon mêlé.
/// Socle : rend toujours `Transparent` (aucun pointeur posé).
pub fn couche(noir: [u8; 3], blanc: [u8; 3]) -> Couche {
    let _ = (noir, blanc);
    Couche::Transparent
}

/// Pose le pointeur dans `pixels`, qui sont ceux de `zone` (pixels réels du bureau) ; seule
/// la partie dans la zone est posée, l'alpha des pixels est gardé. Rend vrai si le pointeur
/// recoupe la zone. Socle : ne touche à rien et rend faux.
pub fn poser_pointeur(pixels: &mut Pixels, zone: Rect, p: &Pointeur) -> bool {
    let _ = (pixels, zone, p);
    false
}
