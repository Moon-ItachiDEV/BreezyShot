//! Le son de la capture (0.8.5) : « Goutte », une goutte d'eau douce de 0,4 s, synthétisée
//! pour BreezyShot (script `outils/son/synth.py`, aucun échantillon tiers).
//! Façade commune : `windows.rs` (`PlaySoundW`, en mémoire, sans attendre), `autre.rs`
//! (rien : le son de macOS viendra avec macOS).

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Le fichier WAV du son (mono, 44,1 kHz, 16 bits), compilé dans l'exécutable.
pub(crate) static CAPTURE: &[u8] = include_bytes!("../../../assets/son-capture.wav");

/// Joue le son de la capture sans attendre sa fin ; un second appel coupe le premier.
/// Ne dit rien et ne bloque jamais si le système n'a pas de sortie audio.
pub fn capture() {
    plateforme::jouer(CAPTURE);
}

#[cfg(test)]
mod tests {
    use super::CAPTURE;

    /// Le fichier compilé est bien un WAV PCM mono 16 bits de 0,4 s environ.
    #[test]
    fn le_fichier_est_un_wav_court() {
        assert_eq!(&CAPTURE[0..4], b"RIFF");
        assert_eq!(&CAPTURE[8..12], b"WAVE");
        assert_eq!(u16::from_le_bytes([CAPTURE[20], CAPTURE[21]]), 1, "PCM");
        assert_eq!(u16::from_le_bytes([CAPTURE[22], CAPTURE[23]]), 1, "mono");
        assert_eq!(
            u16::from_le_bytes([CAPTURE[34], CAPTURE[35]]),
            16,
            "16 bits"
        );
        let octets = CAPTURE.len() - 44;
        let secondes = octets as f64 / 2.0 / 44100.0;
        assert!((0.3..0.6).contains(&secondes), "{secondes} s");
    }

    #[test]
    fn jouer_ne_panique_pas() {
        super::capture();
    }
}
