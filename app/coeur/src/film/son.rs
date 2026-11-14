//! Le calage du son du micro sur l'horloge du film (F18) : la piste est continue ; un
//! manque de plus de 100 ms est comblé par du silence, une avance de plus de 100 ms est
//! retirée. Le fil `film-son` range (`ranger`), le fil `film-video` tire (`tirer`) : une
//! file bornée à 2 s entre eux. PCM 48 kHz, mono, 16 bits.

use std::collections::VecDeque;
use std::time::Duration;

/// Échantillons par seconde.
pub const FREQUENCE: u64 = 48_000;
/// La file : 2 s au plus ; au-delà, le plus ancien est perdu.
pub const FILE_MAX: usize = 2 * FREQUENCE as usize;
/// Écart toléré entre la piste et l'horloge du film : 100 ms.
pub const ECART_MAX: u64 = FREQUENCE / 10;

/// Le son rangé en attente d'être écrit, et ce qui a déjà été écrit.
#[derive(Debug, Clone, Default)]
pub struct Calage {
    ranges: VecDeque<i16>,
    /// Échantillons déjà donnés à la piste (sa position).
    ecrits: u64,
}

/// Le nombre d'échantillons de `t` (arrondi à l'échantillon inférieur).
fn echantillons(t: Duration) -> u64 {
    let n = t.as_nanos() * u128::from(FREQUENCE) / 1_000_000_000;
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// La durée de `n` échantillons.
fn duree(n: u64) -> Duration {
    let ns = u128::from(n) * 1_000_000_000 / u128::from(FREQUENCE);
    Duration::from_nanos(u64::try_from(ns).unwrap_or(u64::MAX))
}

impl Calage {
    /// Range des échantillons venus du micro (file bornée à 2 s : au-delà, les plus
    /// anciens sont perdus).
    pub fn ranger(&mut self, pcm: &[i16]) {
        if pcm.len() >= FILE_MAX {
            self.ranges.clear();
            self.ranges.extend(&pcm[pcm.len() - FILE_MAX..]);
            return;
        }
        let trop = (self.ranges.len() + pcm.len()).saturating_sub(FILE_MAX);
        self.ranges.drain(..trop);
        self.ranges.extend(pcm);
    }

    /// Ce qu'il faut écrire pour amener la piste à `jusqu_a` : le son rangé si `ouvert`,
    /// du silence sinon ou s'il manque plus de 100 ms (F18). Rend l'instant de la piste
    /// au début de ce qui est ajouté à `dest`.
    ///
    /// - Micro coupé : du silence jusqu'à `jusqu_a`, et la file est vidée.
    /// - Avance (la file tient plus de 100 ms de trop) : le plus ancien est retiré, seul
    ///   le plus récent est écrit.
    /// - Manque de plus de 100 ms : du silence d'abord, puis tout le son rangé.
    /// - Écart de 100 ms au plus : le son rangé tel quel, la piste suit à cet écart près.
    pub fn tirer(&mut self, jusqu_a: Duration, ouvert: bool, dest: &mut Vec<i16>) -> Duration {
        let debut = duree(self.ecrits);
        let voulu = echantillons(jusqu_a);
        let manque = voulu.saturating_sub(self.ecrits);
        if !ouvert {
            self.ranges.clear();
            self.silence(manque, dest);
            return debut;
        }
        let dispo = self.ranges.len() as u64;
        if dispo > manque + ECART_MAX {
            // Avance : le son rangé dépasse de plus de 100 ms ; le plus ancien part.
            self.ranges.drain(..(dispo - manque) as usize);
            self.prendre(manque, dest);
        } else if dispo >= manque {
            self.prendre(manque, dest);
        } else if manque - dispo > ECART_MAX {
            // Manque : du silence, puis tout le son rangé, qui finit à l'heure.
            self.silence(manque - dispo, dest);
            self.prendre(dispo, dest);
        } else {
            self.prendre(dispo, dest);
        }
        debut
    }

    /// La position de la piste (temps déjà écrit).
    pub fn piste(&self) -> Duration {
        duree(self.ecrits)
    }

    /// Le son rangé en attente.
    pub fn en_attente(&self) -> usize {
        self.ranges.len()
    }

    fn silence(&mut self, n: u64, dest: &mut Vec<i16>) {
        dest.resize(dest.len() + n as usize, 0);
        self.ecrits += n;
    }

    fn prendre(&mut self, n: u64, dest: &mut Vec<i16>) {
        let n = (n as usize).min(self.ranges.len());
        dest.extend(self.ranges.drain(..n));
        self.ecrits += n as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// `n` échantillons de valeur `v`.
    fn son(n: usize, v: i16) -> Vec<i16> {
        vec![v; n]
    }

    #[test]
    fn son_a_l_heure_ecrit_tel_quel() {
        let mut c = Calage::default();
        let mut d = Vec::new();
        c.ranger(&son(4800, 7));
        assert_eq!(c.tirer(ms(100), true, &mut d), Duration::ZERO);
        assert_eq!(d, son(4800, 7));
        // Un peu en retard (50 ms) : le son tel quel, pas de silence.
        c.ranger(&son(2400, 8));
        d.clear();
        assert_eq!(c.tirer(ms(200), true, &mut d), ms(100));
        assert_eq!(d, son(2400, 8));
        assert_eq!(c.piste(), ms(150));
        // Un peu en avance (moins de 100 ms) : le surplus attend.
        c.ranger(&son(7200, 9));
        d.clear();
        c.tirer(ms(250), true, &mut d);
        assert_eq!(d.len(), 4800);
        assert_eq!(c.en_attente(), 2400);
    }

    #[test]
    fn manque_de_plus_de_100_ms_comble_par_du_silence() {
        let mut c = Calage::default();
        let mut d = Vec::new();
        c.ranger(&son(960, 5));
        // 500 ms voulues, 20 ms rangées : 480 ms de silence, puis le son.
        assert_eq!(c.tirer(ms(500), true, &mut d), Duration::ZERO);
        assert_eq!(d.len(), 24_000);
        assert!(d[..23_040].iter().all(|&s| s == 0));
        assert!(d[23_040..].iter().all(|&s| s == 5));
        assert_eq!(c.piste(), ms(500));
    }

    #[test]
    fn avance_de_plus_de_100_ms_retiree() {
        let mut c = Calage::default();
        let mut d = Vec::new();
        // 1 s rangée, 100 ms voulues : le plus ancien est retiré, le plus récent écrit.
        let mut pcm = son(43_200, 1);
        pcm.extend(son(4800, 2));
        c.ranger(&pcm);
        c.tirer(ms(100), true, &mut d);
        assert_eq!(d, son(4800, 2));
        assert_eq!(c.en_attente(), 0);
        assert_eq!(c.piste(), ms(100));
    }

    #[test]
    fn micro_coupe_puis_ouvert() {
        let mut c = Calage::default();
        let mut d = Vec::new();
        // Coupé : du silence, la piste avance avec l'horloge.
        assert_eq!(c.tirer(ms(1000), false, &mut d), Duration::ZERO);
        assert_eq!(d.len(), 48_000);
        assert!(d.iter().all(|&s| s == 0));
        // Ce qui aurait été rangé pendant ce temps est jeté.
        c.ranger(&son(100, 3));
        d.clear();
        c.tirer(ms(1500), false, &mut d);
        assert_eq!(d.len(), 24_000);
        assert_eq!(c.en_attente(), 0);
        // Ouvert : le son arrive, la piste reste continue.
        c.ranger(&son(4800, 4));
        d.clear();
        assert_eq!(c.tirer(ms(1600), true, &mut d), ms(1500));
        assert_eq!(d, son(4800, 4));
        assert_eq!(c.piste(), ms(1600));
        // Un temps qui recule n'écrit rien.
        d.clear();
        assert_eq!(c.tirer(ms(1000), true, &mut d), ms(1600));
        assert!(d.is_empty());
    }

    #[test]
    fn file_bornee_a_2_s() {
        let mut c = Calage::default();
        for _ in 0..10 {
            c.ranger(&son(48_000, 1));
        }
        assert_eq!(c.en_attente(), FILE_MAX);
        c.ranger(&son(FILE_MAX + 10, 2));
        assert_eq!(c.en_attente(), FILE_MAX);
        let mut d = Vec::new();
        c.tirer(Duration::from_secs(2), true, &mut d);
        assert!(d.iter().all(|&s| s == 2));
    }

    #[test]
    fn longue_piste_continue() {
        // Dix minutes par tours de 33 ms, le micro un peu plus lent que l'horloge : la
        // piste suit l'horloge à 100 ms près, sans trou ni recouvrement.
        let mut c = Calage::default();
        let mut total = 0u64;
        let mut d = Vec::new();
        for tour in 1..=18_000u64 {
            c.ranger(&son(1580, 1));
            d.clear();
            let debut = c.tirer(ms(tour * 33 + tour / 3), true, &mut d);
            assert_eq!(debut, duree(total));
            total += d.len() as u64;
            let voulu = echantillons(ms(tour * 33 + tour / 3));
            assert!(voulu.abs_diff(total) <= ECART_MAX, "tour {tour}");
        }
    }
}
