//! Images vivantes (contrat 0.6, §13.1) : prendre en continu un rectangle de l'écran, sans
//! les fenêtres de l'app exclues des captures (la fenêtre de contrôle de la capture
//! défilante). Le moyen est caché ici : `BitBlt` de la zone, `SRCCOPY` sans `CAPTUREBLT`
//! (décision R2 après la sonde) ; rien d'autre ne suppose comment l'image est prise.
//!
//! Façade commune : `windows.rs` la réalise, `autre.rs` dit « non disponible » ailleurs
//! (Linux, et macOS : la capture défilante y est absente).
//!
//! Tout se compte en pixels réels du bureau virtuel, comme `gel`.

use std::time::Duration;

use super::gel::{Pixels, Rect};

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Vrai si les images vivantes existent sur ce système. Faux : la capture défilante est
/// absente, même avec une licence (`application::actions_selon`).
pub fn disponible() -> bool {
    plateforme::disponible()
}

/// Une prise d'images vivantes d'un rectangle de l'écran : le tampon du système est gardé
/// et réutilisé d'une image à l'autre ; tout est rendu à la destruction. À garder dans un
/// seul fil (le fil `defilante`).
#[derive(Debug)]
pub struct Prise {
    interne: plateforme::Prise,
}

/// Ouvre une prise sur `zone` (pixels réels du bureau), coupée au bureau virtuel : `zone()`
/// dit ce qui est vraiment pris. Erreur : zone vide ou hors du bureau, tampon refusé, ou
/// système sans images vivantes.
pub fn ouvrir(zone: Rect) -> Result<Prise, String> {
    plateforme::ouvrir(zone).map(|interne| Prise { interne })
}

impl Prise {
    /// Le rectangle pris, en pixels réels du bureau.
    pub fn zone(&self) -> Rect {
        plateforme::zone(&self.interne)
    }

    /// Une image RGBA (alpha 255) de la zone dans `dest`, réutilisé (sa taille est mise à
    /// celle de la zone) ; rend la durée de la prise. Refuse tant que l'écran figé est
    /// montré (`gel::ecran_fige_montre`, la garde de `gel::figer_bureau`) : l'image serait
    /// celle de l'écran figé. La garde porte sur la fenêtre, pas sur la dernière image
    /// composée : juste après avoir caché l'écran figé, attendre le compositeur (au moins un
    /// `DwmFlush`, D4) avant la première prise.
    pub fn prendre(&mut self, dest: &mut Pixels) -> Result<Duration, String> {
        plateforme::prendre(&mut self.interne, dest)
    }
}

/// La zone prise : `zone` coupée au bureau virtuel. Erreur si rien n'en reste.
#[cfg_attr(not(windows), allow(dead_code))]
fn couper_au_bureau(zone: Rect, bureau: Rect) -> Result<Rect, String> {
    if zone.est_vide() {
        return Err(format!("zone vide ({}x{})", zone.largeur, zone.hauteur));
    }
    zone.intersection(bureau)
        .ok_or_else(|| "zone hors du bureau".to_string())
}

/// Met `dest` à la taille `largeur` × `hauteur` (RGBA) en gardant son tampon : aucune
/// allocation quand la taille ne change pas (ni quand elle diminue). Rend le nombre
/// d'octets.
#[cfg_attr(not(windows), allow(dead_code))]
fn preparer_destination(dest: &mut Pixels, largeur: u32, hauteur: u32) -> Result<usize, String> {
    let octets = (largeur as usize)
        .checked_mul(hauteur as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or("zone trop grande")?;
    dest.largeur = largeur;
    dest.hauteur = hauteur;
    // `resize` ne réalloue que si la capacité manque.
    dest.rgba.resize(octets, 0);
    Ok(octets)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, largeur: i32, hauteur: i32) -> Rect {
        Rect {
            x,
            y,
            largeur,
            hauteur,
        }
    }

    #[test]
    fn zone_coupee_au_bureau() {
        // Deux écrans : un à gauche du principal (x négatifs).
        let bureau = r(-1920, 0, 1920 + 2560, 1440);
        let dedans = r(100, 200, 600, 450);
        assert_eq!(couper_au_bureau(dedans, bureau), Ok(dedans));
        assert_eq!(
            couper_au_bureau(r(-2000, -10, 600, 450), bureau),
            Ok(r(-1920, 0, 520, 440))
        );
        assert_eq!(
            couper_au_bureau(r(2400, 1400, 600, 450), bureau),
            Ok(r(2400, 1400, 160, 40))
        );
        assert_eq!(couper_au_bureau(bureau, bureau), Ok(bureau));
        assert!(couper_au_bureau(r(2560, 0, 10, 10), bureau).is_err());
        assert!(couper_au_bureau(r(-1930, 0, 10, 10), bureau).is_err());
        assert!(couper_au_bureau(r(0, 0, 0, 10), bureau).is_err());
        assert!(couper_au_bureau(r(0, 0, 10, -1), bureau).is_err());
    }

    #[test]
    fn destination_reutilisee() {
        let mut dest = Pixels {
            largeur: 0,
            hauteur: 0,
            rgba: Vec::new(),
        };
        assert_eq!(preparer_destination(&mut dest, 600, 450), Ok(600 * 450 * 4));
        assert_eq!((dest.largeur, dest.hauteur), (600, 450));
        assert_eq!(dest.rgba.len(), 600 * 450 * 4);
        let adresse = dest.rgba.as_ptr();
        let capacite = dest.rgba.capacity();
        dest.rgba.fill(7);
        // Même taille : même tampon, contenu laissé (il sera écrasé par la prise).
        preparer_destination(&mut dest, 600, 450).unwrap();
        assert_eq!(dest.rgba.as_ptr(), adresse);
        assert_eq!(dest.rgba.capacity(), capacite);
        // Plus petite : même tampon.
        preparer_destination(&mut dest, 300, 200).unwrap();
        assert_eq!((dest.largeur, dest.hauteur), (300, 200));
        assert_eq!(dest.rgba.len(), 300 * 200 * 4);
        assert_eq!(dest.rgba.as_ptr(), adresse);
        // Une taille qui déborde : erreur, pas de panique.
        let mut autre = Pixels {
            largeur: 1,
            hauteur: 1,
            rgba: vec![0; 4],
        };
        assert!(preparer_destination(&mut autre, u32::MAX, u32::MAX).is_err());
    }

    /// Hors Windows : rien n'est disponible, et `ouvrir` le dit sans paniquer.
    #[cfg(not(windows))]
    #[test]
    fn absent_hors_de_windows() {
        assert!(!disponible());
        let e = ouvrir(r(0, 0, 100, 100)).unwrap_err();
        assert!(e.contains("non disponibles"), "{e}");
    }

    /// Sous Windows : disponible (R2).
    #[cfg(windows)]
    #[test]
    fn disponible_sous_windows() {
        assert!(disponible());
    }
}
