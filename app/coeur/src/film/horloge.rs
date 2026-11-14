//! L'horloge du film (F13) : une grille fixe d'images (`cadence` par seconde), l'image `n`
//! étant l'écran au temps `n / cadence` du film ; les pauses sont retirées du temps ; un
//! pas manqué est sauté, jamais rattrapé (l'horloge fait foi, pas le nombre d'images).

use std::time::{Duration, Instant};

/// Nanosecondes dans une seconde.
const NS: u128 = 1_000_000_000;

/// L'horloge d'un film : son début, ses pauses, sa cadence.
#[derive(Debug, Clone)]
pub struct Horloge {
    cadence: u32,
    debut: Instant,
    /// Le temps passé en pause, pauses finies.
    pauses: Duration,
    /// Le début de la pause en cours.
    en_pause: Option<Instant>,
    /// Le numéro du prochain pas à prendre (tous ceux d'avant sont pris ou sautés).
    prochain: u64,
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
            pauses: Duration::ZERO,
            en_pause: None,
            prochain: 0,
        }
    }

    /// L'horloge s'arrête à `a` (« Pause »). Une seconde pause ne change rien.
    pub fn pause(&mut self, a: Instant) {
        if self.en_pause.is_none() {
            self.en_pause = Some(a.max(self.debut));
        }
    }

    /// L'horloge repart à `a` (« Reprendre »). Sans pause, rien ne change.
    pub fn reprise(&mut self, a: Instant) {
        if let Some(depuis) = self.en_pause.take() {
            self.pauses += a.saturating_duration_since(depuis);
        }
    }

    /// Vrai pendant une pause.
    pub fn en_pause(&self) -> bool {
        self.en_pause.is_some()
    }

    /// Le temps du film à `a`, pauses retirées. Pendant une pause, il ne bouge pas.
    pub fn temps(&self, a: Instant) -> Duration {
        let a = match self.en_pause {
            Some(depuis) => a.min(depuis),
            None => a,
        };
        a.saturating_duration_since(self.debut)
            .saturating_sub(self.pauses)
    }

    /// Le temps du film de l'image `numero` : `numero / cadence`, exact à la nanoseconde.
    pub fn instant_de(&self, numero: u64) -> Duration {
        let ns = u128::from(numero) * NS / u128::from(self.cadence);
        Duration::from_nanos(u64::try_from(ns).unwrap_or(u64::MAX))
    }

    /// La durée d'un pas de la grille.
    pub fn periode(&self) -> Duration {
        self.instant_de(1)
    }

    /// Le pas de la grille à `a` (F13) : le dernier pas échu, s'il n'est pas encore pris ;
    /// ceux d'avant qui ne l'ont pas été sont sautés. En pause : attendre un pas.
    pub fn pas(&mut self, a: Instant) -> Pas {
        if self.en_pause.is_some() {
            return Pas::Attendre(self.periode());
        }
        let t = self.temps(a);
        // Le dernier pas échu : floor(t × cadence).
        let echu = t.as_nanos() * u128::from(self.cadence) / NS;
        let echu = u64::try_from(echu).unwrap_or(u64::MAX);
        if echu < self.prochain {
            let suivant = self.instant_de(self.prochain);
            return Pas::Attendre(suivant.saturating_sub(t));
        }
        let sautes = u32::try_from(echu - self.prochain).unwrap_or(u32::MAX);
        self.prochain = echu.saturating_add(1);
        Pas::Prendre {
            numero: echu,
            instant: self.instant_de(echu),
            sautes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn prendre(h: &mut Horloge, a: Instant) -> (u64, Duration, u32) {
        match h.pas(a) {
            Pas::Prendre {
                numero,
                instant,
                sautes,
            } => (numero, instant, sautes),
            autre => panic!("attendu Prendre, eu {autre:?}"),
        }
    }

    #[test]
    fn grille_a_trente_et_quinze() {
        let t0 = Instant::now();
        let mut h = Horloge::nouvelle(30, t0);
        assert_eq!(prendre(&mut h, t0), (0, Duration::ZERO, 0));
        // Avant le pas 1 (33,33 ms) : attendre ce qu'il reste.
        assert_eq!(
            h.pas(t0 + ms(10)),
            Pas::Attendre(Duration::from_nanos(23_333_333))
        );
        let (n, i, s) = prendre(&mut h, t0 + ms(34));
        assert_eq!((n, s), (1, 0));
        assert_eq!(i, Duration::from_nanos(33_333_333));
        // Un pas pris ne l'est jamais deux fois.
        assert!(matches!(h.pas(t0 + ms(34)), Pas::Attendre(_)));
        // Le temps de l'image 30 est exactement 1 s.
        assert_eq!(h.instant_de(30), Duration::from_secs(1));

        let mut g = Horloge::nouvelle(15, t0);
        assert_eq!(g.periode(), Duration::from_nanos(66_666_666));
        assert_eq!(g.instant_de(15), Duration::from_secs(1));
        let mut pris = 0;
        let mut a = t0;
        while a <= t0 + Duration::from_secs(10) {
            if matches!(g.pas(a), Pas::Prendre { .. }) {
                pris += 1;
            }
            a += ms(1);
        }
        // De 0 à 10 s compris : 151 pas, aucun sauté.
        assert_eq!(pris, 151);
    }

    #[test]
    fn retard_saute_sans_rattraper() {
        let t0 = Instant::now();
        let mut h = Horloge::nouvelle(30, t0);
        prendre(&mut h, t0);
        // Le fil revient avec 200 ms de retard : on prend le pas 6, les pas 1 à 5 sont
        // sautés, et aucun n'est rendu ensuite.
        let (n, i, s) = prendre(&mut h, t0 + ms(200));
        assert_eq!((n, s), (6, 5));
        assert_eq!(i, h.instant_de(6));
        assert!(matches!(h.pas(t0 + ms(201)), Pas::Attendre(_)));
        let (n, _, s) = prendre(&mut h, t0 + ms(234));
        assert_eq!((n, s), (7, 0));
    }

    #[test]
    fn cadence_d_appel_irreguliere() {
        // Le fil appelle à des moments quelconques : l'instant rendu est toujours celui de
        // la grille, les numéros croissent, et pris + sautés = pas échus.
        let t0 = Instant::now();
        let mut h = Horloge::nouvelle(30, t0);
        let ecarts = [1u64, 7, 40, 3, 90, 33, 34, 0, 120, 5, 66, 13];
        let mut a = t0;
        let (mut pris, mut sautes, mut dernier) = (0u64, 0u64, None);
        for k in 0..600 {
            a += ms(ecarts[k % ecarts.len()]);
            if let Pas::Prendre {
                numero,
                instant,
                sautes: s,
            } = h.pas(a)
            {
                assert_eq!(instant, h.instant_de(numero));
                assert!(instant <= h.temps(a));
                assert!(h.temps(a) - instant <= h.periode() + Duration::from_nanos(1));
                if let Some(d) = dernier {
                    assert_eq!(numero, d + 1 + u64::from(s));
                }
                dernier = Some(numero);
                pris += 1;
                sautes += u64::from(s);
            }
        }
        let echus = h.temps(a).as_nanos() * 30 / 1_000_000_000 + 1;
        assert_eq!(u128::from(pris + sautes), echus);
    }

    #[test]
    fn pause_retiree_du_temps() {
        let t0 = Instant::now();
        let mut h = Horloge::nouvelle(30, t0);
        prendre(&mut h, t0);
        h.pause(t0 + ms(500));
        assert!(h.en_pause());
        // Pendant la pause : le temps ne bouge pas, aucun pas.
        assert_eq!(h.temps(t0 + ms(5000)), ms(500));
        assert!(matches!(h.pas(t0 + ms(5000)), Pas::Attendre(_)));
        // Une seconde pause ne change rien.
        h.pause(t0 + ms(3000));
        h.reprise(t0 + ms(10_500));
        assert!(!h.en_pause());
        assert_eq!(h.temps(t0 + ms(10_500)), ms(500));
        assert_eq!(h.temps(t0 + ms(11_000)), ms(1000));
        // Au retour : seul le dernier pas échu avant la pause est pris (le 15, à 500 ms),
        // les pas d'avant la pause manqués sont sautés, rien de la pause n'est compté.
        let (n, i, s) = prendre(&mut h, t0 + ms(10_500));
        assert_eq!((n, s), (15, 14));
        assert_eq!(i, ms(500));
        // Reprise sans pause : rien.
        h.reprise(t0 + ms(20_000));
        assert_eq!(h.temps(t0 + ms(11_000)), ms(1000));
        // Deux pauses s'additionnent.
        h.pause(t0 + ms(11_000));
        h.reprise(t0 + ms(12_000));
        assert_eq!(h.temps(t0 + ms(13_000)), ms(1000) + ms(1000));
    }

    #[test]
    fn cadence_nulle_bornee() {
        let t0 = Instant::now();
        let h = Horloge::nouvelle(0, t0);
        assert_eq!(h.periode(), Duration::from_secs(1));
        // Un instant avant le début (horloge prise avant) ne fait pas reculer.
        let h = Horloge::nouvelle(30, t0 + ms(10));
        assert_eq!(h.temps(t0), Duration::ZERO);
    }
}
