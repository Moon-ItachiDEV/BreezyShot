//! Le fichier MP4 d'une vidéo (contrat 0.7, §13.1, F15, F16, F31, F32) : Media Foundation,
//! et rien d'autre. Une piste vidéo H.264 (profil « High », 4:2:0, 8 bits, BT.709) écrite
//! par un `IMFSinkWriter`, nourrie d'images NV12 converties ici depuis les pixels RGBA du
//! cœur (BT.709, plage réduite) ; une piste AAC (48 kHz, mono, 96 kbit/s) si le PC a un
//! micro. MP4 ordinaire, sans index au début (R15, R17).
//!
//! **Garde (F31)** : les DLL de Media Foundation sont chargées à la demande (`build.rs`) ;
//! avant tout appel, `bibliotheques` les charge par `LoadLibraryExW` (système 32 seulement)
//! et garde le résultat. Faux : aucun autre appel de ce module ne part (`creer` rend une
//! erreur, `codeur_h264_present` rend faux), et l'app vit sans vidéo.
//!
//! Façade commune : `windows.rs` la réalise, `autre.rs` dit « non disponible » ailleurs.
//!
//! Socle : signatures ; `windows.rs` ne fait encore rien (chantier B).

use std::marker::PhantomData;
use std::path::Path;
use std::time::Duration;

use super::gel::Pixels;

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Variable d'essai : la garde répond « absent » sans rien charger (essai 15, cas J).
pub const VARIABLE_SANS_CODEUR: &str = "BREEZYSHOT_ESSAI_SANS_CODEUR";

/// Plus grande image donnée au codeur, pixels : la valeur que Microsoft donne pour son
/// codeur logiciel (contrat 0.7, §13.1, à vérifier par S2 point 2 ; chantier B).
pub const LARGEUR_MAX: u32 = 4096;
pub const HAUTEUR_MAX: u32 = 2304;

/// La garde F31 : les trois DLL de Media Foundation (`mfplat`, `mfreadwrite`, `mf`) se
/// chargent. Résultat gardé. Aucun autre appel de ce module ne part si elle rend faux.
pub fn bibliotheques() -> bool {
    plateforme::bibliotheques()
}

/// Un codeur H.264 existe (F32 : `MFStartup`, `MFTEnumEx` sans activer aucun codeur,
/// `MFShutdown`). Bloquant : fil `film-codeurs` seulement. Faux si la garde l'est.
pub fn codeur_h264_present() -> bool {
    plateforme::codeur_h264_present()
}

/// Vrai si le codeur accepte une image de `largeur` × `hauteur` pixels (fonction pure,
/// F4) : côtés pairs, non nuls, dans les bornes `LARGEUR_MAX` × `HAUTEUR_MAX`.
pub fn taille_acceptee(largeur: u32, hauteur: u32) -> bool {
    largeur > 0
        && hauteur > 0
        && largeur.is_multiple_of(2)
        && hauteur.is_multiple_of(2)
        && largeur <= LARGEUR_MAX
        && hauteur <= HAUTEUR_MAX
}

/// Le codeur que l'écrivain a pris.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codeur {
    Materiel,
    Logiciel,
}

impl Codeur {
    /// Nom au journal : `materiel`, `logiciel`.
    pub fn nom(self) -> &'static str {
        match self {
            Codeur::Materiel => "materiel",
            Codeur::Logiciel => "logiciel",
        }
    }
}

/// Ce que l'écrivain doit produire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parametres {
    /// Côtés de l'image, pairs (`taille_acceptee`).
    pub largeur: u32,
    pub hauteur: u32,
    /// Images par seconde (30, `application::film::CADENCE_VIDEO`).
    pub cadence: u32,
    /// Débit moyen visé (F15).
    pub debit_bit_s: u32,
    /// Une piste son (le PC a un micro, F17).
    pub son: bool,
    /// Essayer le codeur matériel d'abord (F16).
    pub materiel: bool,
}

/// L'écrivain d'un fichier MP4 en cours. Créé, servi et fini dans **un seul fil**
/// (`film-video`) : il n'est pas `Send`.
#[derive(Debug)]
pub struct Ecrivain {
    interne: plateforme::Ecrivain,
    _un_seul_fil: PhantomData<*const ()>,
}

/// Crée le fichier `chemin` et son écrivain (F15, F16). Erreur : garde fausse, côtés
/// refusés, codeur ou fichier refusés.
pub fn creer(chemin: &Path, p: &Parametres) -> Result<Ecrivain, String> {
    plateforme::creer(chemin, p).map(|interne| Ecrivain {
        interne,
        _un_seul_fil: PhantomData,
    })
}

impl Ecrivain {
    /// Le codeur pris.
    pub fn codeur(&self) -> Codeur {
        plateforme::codeur(&self.interne)
    }

    /// Convertit les pixels RGBA en NV12 (BT.709, plage réduite) dans un tampon réutilisé,
    /// puis les donne au codeur à `instant` du film (F15). Rend le temps passé.
    pub fn image(&mut self, pixels: &Pixels, instant: Duration) -> Result<Duration, String> {
        plateforme::image(&mut self.interne, pixels, instant)
    }

    /// La dernière image donnée, de nouveau, à `instant` (F13) : aucune conversion.
    pub fn meme_image(&mut self, instant: Duration) -> Result<Duration, String> {
        plateforme::meme_image(&mut self.interne, instant)
    }

    /// Du son du micro (PCM 48 kHz, mono, 16 bits) à `instant` de la piste.
    pub fn son(&mut self, pcm: &[i16], instant: Duration) -> Result<(), String> {
        plateforme::son(&mut self.interne, pcm, instant)
    }

    /// Ferme le fichier (`Finalize`) ; rend la durée prise.
    pub fn finir(self) -> Result<Duration, String> {
        plateforme::finir(self.interne)
    }

    /// Ferme sans finir (le fichier est à effacer).
    pub fn abandonner(self) {
        plateforme::abandonner(self.interne)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tailles_acceptees() {
        for (l, h) in [
            (600, 450),
            (1024, 768),
            (2560, 1440),
            (120, 120),
            (4096, 2304),
        ] {
            assert!(taille_acceptee(l, h), "{l}x{h}");
        }
        for (l, h) in [(0, 450), (601, 450), (600, 451), (4098, 2304), (4096, 2306)] {
            assert!(!taille_acceptee(l, h), "{l}x{h}");
        }
    }

    /// Hors Windows, et sous Windows tant que le chantier B n'est pas fait : rien
    /// n'existe, sans panique.
    #[test]
    fn absent_sans_panique() {
        if !bibliotheques() {
            assert!(!codeur_h264_present());
            let p = Parametres {
                largeur: 600,
                hauteur: 450,
                cadence: 30,
                debit_bit_s: 1_080_000,
                son: false,
                materiel: true,
            };
            assert!(creer(Path::new("film-essai.mp4"), &p).is_err());
        }
        assert_eq!(Codeur::Logiciel.nom(), "logiciel");
    }
}
