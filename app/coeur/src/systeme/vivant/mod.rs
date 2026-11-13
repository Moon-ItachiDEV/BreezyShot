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

/// Ouvre une prise sur `zone` (pixels réels du bureau). Erreur : zone vide, hors du bureau,
/// tampon refusé, ou système sans images vivantes.
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
    /// montré (même garde que `gel::figer_bureau`) : l'image serait celle de l'écran figé.
    pub fn prendre(&mut self, dest: &mut Pixels) -> Result<Duration, String> {
        plateforme::prendre(&mut self.interne, dest)
    }
}
