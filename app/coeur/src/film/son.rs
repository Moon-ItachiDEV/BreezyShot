//! Le calage du son du micro sur l'horloge du film (F18) : la piste est continue ; un
//! manque de plus de 100 ms est comblé par du silence, une avance de plus de 100 ms est
//! retirée. Le fil `film-son` range (`ranger`), le fil `film-video` tire (`tirer`) : une
//! file bornée à 2 s entre eux. PCM 48 kHz, mono, 16 bits.
//!
//! Socle : signatures, corps inertes (chantier D).

use std::time::Duration;

/// Le son rangé en attente d'être écrit, et ce qui a déjà été écrit.
#[derive(Debug, Clone, Default)]
pub struct Calage {
    _ranges: Vec<i16>,
}

impl Calage {
    /// Range des échantillons venus du micro (file bornée à 2 s). Socle : rien.
    pub fn ranger(&mut self, pcm: &[i16]) {
        let _ = pcm;
    }

    /// Ce qu'il faut écrire pour amener la piste à `jusqu_a` : le son rangé si `ouvert`,
    /// du silence sinon ou s'il manque plus de 100 ms (F18). Rend l'instant de la piste
    /// au début de `dest`. Socle : rien d'ajouté, la piste à zéro.
    pub fn tirer(&mut self, jusqu_a: Duration, ouvert: bool, dest: &mut Vec<i16>) -> Duration {
        let _ = (jusqu_a, ouvert, dest);
        Duration::ZERO
    }
}
