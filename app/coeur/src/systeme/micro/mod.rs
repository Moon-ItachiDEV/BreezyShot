//! Le micro (contrat 0.7, §13.1, F17, F18) : WASAPI, le point de capture par défaut du rôle
//! `eConsole` (celui de « Son › Entrée »), en mode partagé, converti par Windows en PCM
//! 48 kHz mono 16 bits. L'appareil n'est ouvert que pendant que « Micro » est actif ; le
//! son du système n'est jamais pris.
//!
//! Façade commune : `windows.rs` la réalise, `autre.rs` dit « absent » ailleurs. Ce qui ne
//! dépend pas de Windows est ici, pur et testé partout : la lecture du format que rend
//! l'appareil, la conversion en PCM 48 kHz mono 16 bits (quand Windows ne la fait pas
//! lui-même), et le sens des codes d'erreur.
//!
//! Journal (§8) : `film: micro (<code>)` en WARN, écrit ici une fois par échec (ouverture
//! refusée pour une autre raison que l'absence d'appareil, ou appareil perdu en cours de
//! route) ; le reste (`film micro <ouvert|coupe|indisponible>`) est écrit par le film. Jamais
//! le nom de l'appareil.

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Vrai si le PC a un point de capture actif (le bouton « Micro » est montré).
pub fn present() -> bool {
    plateforme::present()
}

/// Pourquoi le micro ne s'ouvre pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Erreur {
    /// Aucun point de capture.
    Absent,
    /// Refusé par Windows (confidentialité) ou pris en exclusif.
    Refuse,
    Autre(String),
}

/// Le micro ouvert. À sa destruction, le fil `film-son` s'arrête et l'appareil est rendu.
#[derive(Debug)]
pub struct Micro {
    interne: plateforme::Micro,
}

/// Ouvre le micro : `rappel` reçoit du PCM 48 kHz mono 16 bits, depuis le fil `film-son` ;
/// il ne fait que ranger (`film::Calage`).
pub fn ouvrir(rappel: impl FnMut(&[i16]) + Send + 'static) -> Result<Micro, Erreur> {
    plateforme::ouvrir(Box::new(rappel)).map(|interne| Micro { interne })
}

impl Micro {
    /// Vrai si l'appareil a été débranché ou repris par Windows depuis l'ouverture.
    pub fn perdu(&self) -> bool {
        plateforme::perdu(&self.interne)
    }
}

/// La fréquence rendue au rappel (F17).
pub const FREQUENCE: u32 = 48_000;

// ---------------------------------------------------------------------------------------
// Logique pure : format de l'appareil, conversion, codes d'erreur.

/// La forme d'un échantillon dans le tampon de l'appareil.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sorte {
    /// Entier signé de 16 bits.
    I16,
    /// Entier signé de 24 bits, sur 3 octets.
    I24,
    /// Entier signé de 32 bits (ou 24 bits valides cadrés à gauche dans 32).
    I32,
    /// Flottant de 32 bits, de -1 à 1.
    F32,
}

impl Sorte {
    fn octets(self) -> usize {
        match self {
            Sorte::I16 => 2,
            Sorte::I24 => 3,
            Sorte::I32 | Sorte::F32 => 4,
        }
    }
}

/// Le format d'un flux PCM entrelacé, tel que l'appareil le rend.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Format {
    frequence: u32,
    canaux: u16,
    sorte: Sorte,
    /// Octets d'une trame (un échantillon de chaque canal).
    bloc: usize,
}

/// `WAVE_FORMAT_PCM`, `WAVE_FORMAT_IEEE_FLOAT`, `WAVE_FORMAT_EXTENSIBLE`.
const TAG_PCM: u16 = 1;
const TAG_FLOTTANT: u16 = 3;
const TAG_ETENDU: u16 = 0xFFFE;

/// Lit un `WAVEFORMATEX` (ou `WAVEFORMATEXTENSIBLE`) donné en octets, petit-boutiste : le
/// format de mélange de l'appareil (`GetMixFormat`). Erreur pour un format que la
/// conversion ne sait pas lire.
#[cfg_attr(not(windows), allow(dead_code))]
fn lire_format(octets: &[u8]) -> Result<Format, String> {
    let u16_a = |i: usize| {
        octets
            .get(i..i + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    };
    let u32_a = |i: usize| {
        octets
            .get(i..i + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let court = || "format court".to_string();
    let mut tag = u16_a(0).ok_or_else(court)?;
    let canaux = u16_a(2).ok_or_else(court)?;
    let frequence = u32_a(4).ok_or_else(court)?;
    let bloc = u16_a(12).ok_or_else(court)? as usize;
    let bits = u16_a(14).ok_or_else(court)?;
    let mut valides = bits;
    // `cbSize` : les octets qui suivent les 18 du `WAVEFORMATEX` ; tous doivent être là.
    let suite = u16_a(16).unwrap_or(0) as usize;
    if octets.len() < 18 + suite {
        return Err(court());
    }
    if tag == TAG_ETENDU {
        // `cbSize` (16) ≥ 22 : bits valides (18), masque des canaux (20), sous-format (24),
        // un GUID dont le premier mot est le tag du format (KSDATAFORMAT_SUBTYPE_*).
        if u16_a(16).ok_or_else(court)? < 22 {
            return Err("format etendu court".into());
        }
        valides = u16_a(18).ok_or_else(court)?;
        let sous = u32_a(24).ok_or_else(court)?;
        tag = u16::try_from(sous).map_err(|_| format!("sous-format {sous:#x}"))?;
    }
    let sorte = match (tag, bits) {
        (TAG_PCM, 16) => Sorte::I16,
        (TAG_PCM, 24) => Sorte::I24,
        (TAG_PCM, 32) => Sorte::I32,
        (TAG_FLOTTANT, 32) => Sorte::F32,
        _ => return Err(format!("format {tag} de {bits} bits")),
    };
    if valides == 0 || valides > bits {
        return Err(format!("{valides} bits valides sur {bits}"));
    }
    if canaux == 0 || frequence == 0 {
        return Err(format!("{canaux} canaux a {frequence} Hz"));
    }
    if bloc != canaux as usize * sorte.octets() {
        return Err(format!("bloc de {bloc} octets"));
    }
    Ok(Format {
        frequence,
        canaux,
        sorte,
        bloc,
    })
}

/// Convertit un flux d'appareil en PCM 48 kHz mono 16 bits : moyenne des canaux, puis
/// rééchantillonnage linéaire, d'un paquet à l'autre sans trou ni saut (l'état est gardé).
/// Ne sert que si Windows refuse de convertir lui-même (`AUTOCONVERTPCM`, F17).
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone)]
struct Conversion {
    format: Format,
    /// Pas d'entrée par échantillon de sortie (fréquence de l'appareil / 48 000).
    pas: f64,
    /// Position du prochain échantillon de sortie, en échantillons d'entrée, depuis le
    /// début du paquet courant (-1 : le dernier du paquet précédent).
    position: f64,
    /// Le dernier échantillon (mono) du paquet précédent.
    dernier: f32,
    /// Tampon des échantillons mono du paquet, gardé.
    mono: Vec<f32>,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Conversion {
    fn nouvelle(format: Format) -> Conversion {
        Conversion {
            format,
            pas: format.frequence as f64 / FREQUENCE as f64,
            position: 0.0,
            dernier: 0.0,
            mono: Vec::new(),
        }
    }

    /// Un échantillon, de -1 à 1.
    fn lire(sorte: Sorte, b: &[u8]) -> f32 {
        match sorte {
            Sorte::I16 => i16::from_le_bytes([b[0], b[1]]) as f32 / 32_768.0,
            Sorte::I24 => (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0,
            Sorte::I32 => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0,
            Sorte::F32 => {
                let v = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                if v.is_finite() {
                    v
                } else {
                    0.0
                }
            }
        }
    }

    fn en_i16(v: f32) -> i16 {
        (v * 32_767.0).round().clamp(-32_768.0, 32_767.0) as i16
    }

    /// Convertit `trames` trames de `octets` (ou autant de silence si `silence`) et ajoute
    /// le résultat à `sortie`. Les octets d'une trame incomplète sont ignorés.
    fn convertir(&mut self, octets: &[u8], trames: usize, silence: bool, sortie: &mut Vec<i16>) {
        let f = self.format;
        let trames = if silence {
            trames
        } else {
            trames.min(octets.len() / f.bloc)
        };
        self.mono.clear();
        if silence {
            self.mono.resize(trames, 0.0);
        } else {
            let canaux = f.canaux as usize;
            let taille = f.sorte.octets();
            self.mono
                .extend(octets[..trames * f.bloc].chunks_exact(f.bloc).map(|trame| {
                    let somme: f32 = trame
                        .chunks_exact(taille)
                        .map(|b| Conversion::lire(f.sorte, b))
                        .sum();
                    somme / canaux as f32
                }));
        }
        let n = self.mono.len();
        if n == 0 {
            return;
        }
        if f.frequence == FREQUENCE {
            sortie.extend(self.mono.iter().map(|&v| Conversion::en_i16(v)));
            self.dernier = self.mono[n - 1];
            return;
        }
        // Échantillon d'indice `i` du paquet, -1 étant le dernier du précédent.
        let valeur = |mono: &[f32], dernier: f32, i: isize| {
            if i < 0 {
                dernier
            } else {
                mono[i as usize]
            }
        };
        let fin = (n - 1) as f64;
        while self.position < fin {
            let i = self.position.floor();
            let t = (self.position - i) as f32;
            let i = i as isize;
            let a = valeur(&self.mono, self.dernier, i);
            let b = valeur(&self.mono, self.dernier, i + 1);
            sortie.push(Conversion::en_i16(a + (b - a) * t));
            self.position += self.pas;
        }
        self.position -= n as f64;
        self.dernier = self.mono[n - 1];
    }
}

/// `E_NOTFOUND` : aucun point de capture par défaut.
const E_NOTFOUND: u32 = 0x8007_0490;
/// `E_ACCESSDENIED` : refusé par les réglages de confidentialité de Windows.
const E_ACCESSDENIED: u32 = 0x8007_0005;
const AUDCLNT_E_DEVICE_INVALIDATED: u32 = 0x8889_0004;
const AUDCLNT_E_DEVICE_IN_USE: u32 = 0x8889_000A;
const AUDCLNT_E_EXCLUSIVE_MODE_ONLY: u32 = 0x8889_0012;
const AUDCLNT_E_SERVICE_NOT_RUNNING: u32 = 0x8889_0010;
const AUDCLNT_E_RESOURCES_INVALIDATED: u32 = 0x8889_0026;

/// Le sens d'un code d'erreur de WASAPI à l'ouverture (F18) : pas d'appareil (absent,
/// débranché, service audio arrêté), refusé (confidentialité, pris en exclusif), ou autre.
#[cfg_attr(not(windows), allow(dead_code))]
fn erreur_de(code: i32) -> Erreur {
    match code as u32 {
        E_NOTFOUND
        | AUDCLNT_E_DEVICE_INVALIDATED
        | AUDCLNT_E_SERVICE_NOT_RUNNING
        | AUDCLNT_E_RESOURCES_INVALIDATED => Erreur::Absent,
        E_ACCESSDENIED | AUDCLNT_E_DEVICE_IN_USE | AUDCLNT_E_EXCLUSIVE_MODE_ONLY => Erreur::Refuse,
        _ => Erreur::Autre(code_hex(code)),
    }
}

/// Vrai pour une absence ordinaire d'appareil, qui n'est pas écrite au journal (la machine
/// de l'essai n'a aucun micro : aucune ligne WARN ne doit y paraître).
#[cfg_attr(not(windows), allow(dead_code))]
fn absence_ordinaire(code: i32) -> bool {
    code as u32 == E_NOTFOUND
}

/// Un code d'erreur au journal : `0x80070005`.
#[cfg_attr(not(windows), allow(dead_code))]
fn code_hex(code: i32) -> String {
    format!("0x{:08X}", code as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sans micro (Linux ; Windows sans appareil, comme la machine GitHub) : rien ne
    /// s'ouvre, sans panique, et l'erreur est `Absent`.
    #[test]
    fn absent_sans_panique() {
        if !present() {
            let e = ouvrir(|_| {}).unwrap_err();
            assert_eq!(e, Erreur::Absent);
        }
    }

    /// Hors Windows : jamais de micro.
    #[cfg(not(windows))]
    #[test]
    fn jamais_hors_de_windows() {
        assert!(!present());
        assert_eq!(ouvrir(|_| {}).unwrap_err(), Erreur::Absent);
    }

    /// Un `WAVEFORMATEX` en octets (18 octets, `cbSize` 0).
    fn waveformatex(tag: u16, canaux: u16, frequence: u32, bits: u16) -> Vec<u8> {
        let bloc = canaux * bits / 8;
        let mut o = Vec::new();
        o.extend_from_slice(&tag.to_le_bytes());
        o.extend_from_slice(&canaux.to_le_bytes());
        o.extend_from_slice(&frequence.to_le_bytes());
        o.extend_from_slice(&(frequence * bloc as u32).to_le_bytes());
        o.extend_from_slice(&bloc.to_le_bytes());
        o.extend_from_slice(&bits.to_le_bytes());
        o.extend_from_slice(&0u16.to_le_bytes());
        o
    }

    /// Un `WAVEFORMATEXTENSIBLE` en octets (40 octets), le sous-format donné par son tag.
    fn etendu(sous: u16, canaux: u16, frequence: u32, bits: u16, valides: u16) -> Vec<u8> {
        let mut o = waveformatex(TAG_ETENDU, canaux, frequence, bits);
        o[16..18].copy_from_slice(&22u16.to_le_bytes());
        o.extend_from_slice(&valides.to_le_bytes());
        o.extend_from_slice(&3u32.to_le_bytes());
        // KSDATAFORMAT_SUBTYPE_* : {0000000x-0000-0010-8000-00aa00389b71}.
        o.extend_from_slice(&(sous as u32).to_le_bytes());
        o.extend_from_slice(&[0x00, 0x00, 0x10, 0x00]);
        o.extend_from_slice(&[0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71]);
        o
    }

    #[test]
    fn formats_lus() {
        // Le format de mélange ordinaire de Windows : flottant, 2 canaux, 48 kHz.
        assert_eq!(
            lire_format(&etendu(TAG_FLOTTANT, 2, 48_000, 32, 32)),
            Ok(Format {
                frequence: 48_000,
                canaux: 2,
                sorte: Sorte::F32,
                bloc: 8
            })
        );
        assert_eq!(
            lire_format(&waveformatex(TAG_PCM, 1, 44_100, 16)).map(|f| (f.sorte, f.bloc)),
            Ok((Sorte::I16, 2))
        );
        assert_eq!(
            lire_format(&etendu(TAG_PCM, 4, 96_000, 24, 24)).map(|f| (f.sorte, f.bloc)),
            Ok((Sorte::I24, 12))
        );
        // 24 bits valides dans 32 : lus comme 32 (cadrés à gauche).
        assert_eq!(
            lire_format(&etendu(TAG_PCM, 2, 48_000, 32, 24)).map(|f| f.sorte),
            Ok(Sorte::I32)
        );
        assert_eq!(
            lire_format(&waveformatex(TAG_FLOTTANT, 2, 48_000, 32)).map(|f| f.sorte),
            Ok(Sorte::F32)
        );
        // Refusés, sans panique.
        assert!(lire_format(&[]).is_err());
        assert!(lire_format(&waveformatex(TAG_PCM, 2, 48_000, 16)[..10]).is_err());
        assert!(lire_format(&waveformatex(TAG_PCM, 2, 48_000, 8)).is_err());
        assert!(lire_format(&waveformatex(TAG_FLOTTANT, 2, 48_000, 64)).is_err());
        assert!(lire_format(&waveformatex(0x55, 2, 48_000, 16)).is_err());
        assert!(lire_format(&waveformatex(TAG_PCM, 0, 48_000, 16)).is_err());
        assert!(lire_format(&waveformatex(TAG_PCM, 2, 0, 16)).is_err());
        assert!(lire_format(&etendu(TAG_PCM, 2, 48_000, 16, 20)).is_err());
        assert!(lire_format(&etendu(TAG_PCM, 2, 48_000, 16, 16)[..30]).is_err());
        let mut mauvais_bloc = waveformatex(TAG_PCM, 2, 48_000, 16);
        mauvais_bloc[12] = 3;
        assert!(lire_format(&mauvais_bloc).is_err());
        let mut court = etendu(TAG_PCM, 2, 48_000, 16, 16);
        court[16] = 10;
        assert!(lire_format(&court).is_err());
    }

    fn format(frequence: u32, canaux: u16, sorte: Sorte) -> Format {
        Format {
            frequence,
            canaux,
            sorte,
            bloc: canaux as usize * sorte.octets(),
        }
    }

    #[test]
    fn conversion_sans_reechantillonnage() {
        // 48 kHz, 2 canaux flottants : la moyenne des deux, en 16 bits.
        let mut c = Conversion::nouvelle(format(48_000, 2, Sorte::F32));
        let mut octets = Vec::new();
        for (g, d) in [
            (1.0f32, 1.0f32),
            (0.5, -0.5),
            (-1.0, -1.0),
            (0.25, 0.75),
            (2.0, 2.0),
        ] {
            octets.extend_from_slice(&g.to_le_bytes());
            octets.extend_from_slice(&d.to_le_bytes());
        }
        let mut sortie = Vec::new();
        c.convertir(&octets, 5, false, &mut sortie);
        // Bornée : un flottant hors de -1..1 est écrêté, pas replié.
        assert_eq!(sortie, vec![32_767, 0, -32_767, 16_384, 32_767]);
        // Un flottant non fini devient du silence.
        let mut nan = Vec::new();
        nan.extend_from_slice(&f32::NAN.to_le_bytes());
        nan.extend_from_slice(&f32::INFINITY.to_le_bytes());
        sortie.clear();
        c.convertir(&nan, 1, false, &mut sortie);
        assert_eq!(sortie, vec![0]);
        // Silence annoncé : autant de zéros que de trames, sans lire les octets.
        sortie.clear();
        c.convertir(&[], 480, true, &mut sortie);
        assert_eq!(sortie, vec![0; 480]);
        // Une trame incomplète est ignorée ; plus de trames annoncées que d'octets : bornées.
        sortie.clear();
        c.convertir(&octets[..12], 5, false, &mut sortie);
        assert_eq!(sortie.len(), 1);

        // Entiers 16, 24 et 32 bits, mono.
        let mut c = Conversion::nouvelle(format(48_000, 1, Sorte::I16));
        sortie.clear();
        let i16s: Vec<u8> = [i16::MIN, -1, 0, 1000, i16::MAX]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        c.convertir(&i16s, 5, false, &mut sortie);
        assert_eq!(sortie, vec![-32_767, -1, 0, 1000, 32_766]);
        let mut c = Conversion::nouvelle(format(48_000, 1, Sorte::I24));
        sortie.clear();
        // 0x400000 = la moitié de la pleine échelle ; 0xC00000 = moins la moitié.
        c.convertir(&[0, 0, 0x40, 0, 0, 0xC0], 2, false, &mut sortie);
        assert_eq!(sortie, vec![16_384, -16_384]);
        let mut c = Conversion::nouvelle(format(48_000, 1, Sorte::I32));
        sortie.clear();
        c.convertir(&(1i32 << 30).to_le_bytes(), 1, false, &mut sortie);
        assert_eq!(sortie, vec![16_384]);
    }

    /// Rééchantillonnage : le compte d'échantillons suit la durée, sur des paquets de
    /// toutes tailles, et une sinusoïde garde sa fréquence et son amplitude.
    #[test]
    fn conversion_reechantillonnee() {
        for (frequence, paquet) in [
            (44_100u32, 441usize),
            (96_000, 960),
            (16_000, 160),
            (44_100, 7),
        ] {
            let mut c = Conversion::nouvelle(format(frequence, 1, Sorte::F32));
            let secondes = 2usize;
            let total = frequence as usize * secondes;
            let mut sortie = Vec::new();
            let mut i = 0usize;
            while i < total {
                let n = paquet.min(total - i);
                let octets: Vec<u8> = (i..i + n)
                    .flat_map(|k| {
                        let t = k as f64 / frequence as f64;
                        ((2.0 * std::f64::consts::PI * 440.0 * t).sin() as f32 * 0.5).to_le_bytes()
                    })
                    .collect();
                c.convertir(&octets, n, false, &mut sortie);
                i += n;
            }
            // 2 s à 48 kHz, au plus un échantillon d'entrée près (le dernier attend le
            // paquet suivant pour être interpolé).
            let attendu = FREQUENCE as usize * secondes;
            let marge = (FREQUENCE / frequence) as usize + 2;
            assert!(
                sortie.len().abs_diff(attendu) <= marge,
                "{frequence} Hz par paquets de {paquet} : {} echantillons",
                sortie.len()
            );
            // Fréquence : passages par zéro montants sur 2 s (880 attendus).
            let montants = sortie.windows(2).filter(|w| w[0] < 0 && w[1] >= 0).count();
            assert!(
                montants.abs_diff(880) <= 2,
                "{frequence} Hz : {montants} passages"
            );
            // Amplitude : 0,5 de la pleine échelle, à 2 % près (sans trou ni saut entre
            // paquets : un saut ferait un pic).
            let crete = sortie.iter().map(|v| v.unsigned_abs()).max().unwrap();
            assert!(
                (16_000..=16_700).contains(&crete),
                "{frequence} Hz : crete {crete}"
            );
            let saut = sortie
                .windows(2)
                .map(|w| (w[1] as i32 - w[0] as i32).unsigned_abs())
                .max()
                .unwrap();
            // Pente maximale d'une sinusoïde de 440 Hz d'amplitude 16 384 à 48 kHz : 944.
            assert!(saut <= 1_000, "{frequence} Hz : saut de {saut}");
        }
    }

    #[test]
    fn codes_d_erreur() {
        let c = |v: u32| v as i32;
        assert_eq!(erreur_de(c(0x8007_0490)), Erreur::Absent);
        assert_eq!(erreur_de(c(0x8889_0004)), Erreur::Absent);
        assert_eq!(erreur_de(c(0x8889_0010)), Erreur::Absent);
        assert_eq!(erreur_de(c(0x8007_0005)), Erreur::Refuse);
        assert_eq!(erreur_de(c(0x8889_000A)), Erreur::Refuse);
        assert_eq!(erreur_de(c(0x8889_0012)), Erreur::Refuse);
        assert_eq!(
            erreur_de(c(0x8889_0008)),
            Erreur::Autre("0x88890008".into())
        );
        assert!(absence_ordinaire(c(0x8007_0490)));
        assert!(!absence_ordinaire(c(0x8007_0005)));
        assert!(!absence_ordinaire(c(0x8889_0004)));
    }
}
