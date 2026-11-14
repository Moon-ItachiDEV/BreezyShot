//! L'horloge du film (F13) : une grille fixe d'images (`cadence` par seconde), l'image `n`
//! étant l'écran au temps `n / cadence` du film ; les pauses sont retirées du temps ; un
//! pas manqué est sauté, jamais rattrapé (l'horloge fait foi, pas le nombre d'images).
//!
//! Socle : signatures, corps inertes (chantier D).

use std::time::{Duration, Instant};

/// L'horloge d'un film : son début, ses pauses, sa cadence.
#[derive(Debug, Clone)]
pub struct Horloge {
    cadence: u32,
    debut: Instant,
}

/// Ce que l'horloge dit de faire à l'instant donné.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pas {
    /// Rien avant ce délai (le prochain pas de la grille).
    Attendre(Duration),
    /// Prendre l'image `numero`, du temps `instant` du film ; `sautes` pas manqués depuis
    /// la précédente (comptés dans `sautees`, jamais rattrapés).
    Prendre {
        numero: u64,
        instant: Duration,
        sautes: u32,
    },
}

impl Horloge {
    /// Une horloge à `cadence` images par seconde, partie à `debut`.
    pub fn nouvelle(cadence: u32, debut: Instant) -> Horloge {
        Horloge {
            cadence: cadence.max(1),
            debut,
        }
    }

    /// L'horloge s'arrête à `a` (« Pause »).
    pub fn pause(&mut self, a: Instant) {
        let _ = a;
    }

    /// L'horloge repart à `a` (« Reprendre »).
    pub fn reprise(&mut self, a: Instant) {
        let _ = a;
    }

    /// Le temps du film à `a`, pauses retirées. Socle : le temps depuis le début.
    pub fn temps(&self, a: Instant) -> Duration {
        a.saturating_duration_since(self.debut)
    }

    /// Le pas de la grille à `a` (F13). Socle : toujours attendre un pas.
    pub fn pas(&mut self, a: Instant) -> Pas {
        let _ = a;
        Pas::Attendre(Duration::from_secs(1) / self.cadence)
    }
}
