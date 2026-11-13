//! `Windows.Media.Ocr`, toujours hors du fil principal (fil `texte`, fil `texte-langues`).
//!
//! Chaque appel entre dans l'appartement à plusieurs fils de WinRT (`RoInitialize`) et en
//! sort à la fin. Le moteur est créé à chaque lecture puis relâché (R15). La lecture
//! (`RecognizeAsync`) est attendue dans le fil appelant, au plus `ATTENTE_MAX` : rien
//! n'attend sur le fil principal, et un moteur qui ne rend pas ne garde pas le fil.
//!
//! `TryCreateFrom…` rend `null` (sans erreur) quand aucune langue ne convient : la crate
//! `windows` le change en une erreur de code 0, que `est_nul` reconnaît.

use std::cell::Cell;
use std::sync::mpsc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use ::windows::core::{Error, HRESULT};
use ::windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use ::windows::Media::Ocr::{OcrEngine, OcrResult};
use ::windows::Security::Cryptography::CryptographicBuffer;
use ::windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use ::windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};

use super::{Disponibilite, ErreurMoteur, LigneBrute, MotBrut, Reconnu, ATTENTE_MAX};

pub const FIN_DE_LIGNE: &str = "\r\n";

/// `OcrEngine::MaxImageDimension`, lu une fois (10000 sur la machine d'essai).
static DIMENSION_MAX: OnceLock<u32> = OnceLock::new();

thread_local! {
    /// Durée de création du dernier moteur de ce fil.
    static DERNIERE_CREATION: Cell<Option<Duration>> = const { Cell::new(None) };
}

/// L'entrée de ce fil dans WinRT, défaite à la destruction.
struct Appartement {
    /// Faux si le fil était déjà dans un appartement à un fil (`RPC_E_CHANGED_MODE`) : on
    /// s'en sert tel quel (les objets de la lecture sont agiles) et on ne défait rien.
    a_defaire: bool,
}

impl Appartement {
    fn entrer() -> Result<Appartement, Error> {
        // SAFETY : initialisation WinRT de ce fil ; `Drop` l'équilibre si elle a réussi.
        match unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
            Ok(()) => Ok(Appartement { a_defaire: true }),
            Err(e) if e.code() == RPC_E_CHANGED_MODE => Ok(Appartement { a_defaire: false }),
            Err(e) => Err(e),
        }
    }

    /// Une lecture abandonnée tourne encore dans le moteur : on ne sort pas de
    /// l'appartement, pour ne pas le défaire sous elle (le fil finit ensuite).
    fn laisser(mut self) {
        self.a_defaire = false;
    }
}

impl Drop for Appartement {
    fn drop(&mut self) {
        if self.a_defaire {
            // SAFETY : équilibre le `RoInitialize` réussi de ce fil ; tous les objets WinRT
            // de l'appel sont détruits avant (la garde est créée en premier).
            unsafe { RoUninitialize() };
        }
    }
}

/// Vrai si l'erreur est un `null` rendu par WinRT (code 0), pas une vraie erreur.
fn est_nul(e: &Error) -> bool {
    e.code() == HRESULT(0)
}

/// Cause courte : l'étape et le code HRESULT, jamais de texte.
fn moteur(etape: &str, e: &Error) -> ErreurMoteur {
    ErreurMoteur::Moteur(format!("{etape} {:#010x}", e.code().0 as u32))
}

pub fn disponibilite() -> Disponibilite {
    let Ok(_appartement) = Appartement::entrer() else {
        return Disponibilite::NonPrisEnCharge;
    };
    match langues() {
        Ok(l) if l.is_empty() => Disponibilite::Aucune,
        Ok(l) => Disponibilite::Langues(l),
        // Classe absente (édition de Windows sans la lecture) : pas de moteur.
        Err(_) => Disponibilite::NonPrisEnCharge,
    }
}

/// Les étiquettes BCP-47 des langues de lecture installées.
fn langues() -> Result<Vec<String>, Error> {
    let liste = OcrEngine::AvailableRecognizerLanguages()?;
    let mut etiquettes = Vec::new();
    for i in 0..liste.Size()? {
        etiquettes.push(liste.GetAt(i)?.LanguageTag()?.to_string_lossy());
    }
    Ok(etiquettes)
}

pub fn dimension_max() -> u32 {
    if let Some(d) = DIMENSION_MAX.get() {
        return *d;
    }
    let Ok(_appartement) = Appartement::entrer() else {
        return 0;
    };
    match OcrEngine::MaxImageDimension() {
        // Gardée seulement si elle a pu être lue : un échec sera relu la fois suivante.
        Ok(d) => *DIMENSION_MAX.get_or_init(|| d),
        Err(_) => 0,
    }
}

pub fn derniere_creation() -> Option<Duration> {
    DERNIERE_CREATION.with(Cell::get)
}

pub fn reconnaitre(bgra: &[u8], largeur: u32, hauteur: u32) -> Result<Reconnu, ErreurMoteur> {
    // Créée en premier, détruite en dernier : après tous les objets WinRT ci-dessous.
    let appartement = Appartement::entrer().map_err(|e| moteur("init", &e))?;
    let (Ok(l), Ok(h)) = (i32::try_from(largeur), i32::try_from(hauteur)) else {
        return Err(ErreurMoteur::Moteur("taille".into()));
    };

    let debut = Instant::now();
    let moteur_ocr = creer_moteur()?;
    DERNIERE_CREATION.with(|c| c.set(Some(debut.elapsed())));
    let langue = moteur_ocr
        .RecognizerLanguage()
        .and_then(|l| l.LanguageTag())
        .map(|t| t.to_string_lossy())
        .unwrap_or_default();

    // Le tampon est une copie des octets : `bgra` n'est plus lu après cette ligne.
    let tampon =
        CryptographicBuffer::CreateFromByteArray(bgra).map_err(|e| moteur("tampon", &e))?;
    let image = SoftwareBitmap::CreateCopyFromBuffer(&tampon, BitmapPixelFormat::Bgra8, l, h)
        .map_err(|e| moteur("image", &e))?;
    let operation = moteur_ocr
        .RecognizeAsync(&image)
        .map_err(|e| moteur("lecture", &e))?;

    // Attente sans `join` (qui n'a pas de borne) : la fin est envoyée par le rappel de
    // l'opération (appelé tout de suite si elle est déjà finie), reçue ici avec une borne.
    let (envoi, reception) = mpsc::sync_channel::<Result<OcrResult, Error>>(1);
    operation
        .when(move |r| {
            // Reçu par personne si la lecture a été abandonnée : sans importance.
            let _ = envoi.send(r);
        })
        .map_err(|e| moteur("lecture", &e))?;
    let resultat = match reception.recv_timeout(ATTENTE_MAX) {
        Ok(r) => r.map_err(|e| moteur("lecture", &e))?,
        Err(_) => {
            let _ = operation.Cancel();
            appartement.laisser();
            return Err(ErreurMoteur::Moteur("echeance".into()));
        }
    };
    let lignes = lignes(&resultat).map_err(|e| moteur("resultat", &e))?;
    Ok(Reconnu { langue, lignes })
}

/// Le moteur des langues de l'utilisateur ; s'il n'y en a pas (`null`), celui de la
/// première langue de lecture installée ; sinon `AucuneLangue`.
fn creer_moteur() -> Result<OcrEngine, ErreurMoteur> {
    match OcrEngine::TryCreateFromUserProfileLanguages() {
        Ok(m) => return Ok(m),
        Err(e) if !est_nul(&e) => return Err(moteur("creation", &e)),
        Err(_) => {}
    }
    let liste = OcrEngine::AvailableRecognizerLanguages().map_err(|e| moteur("langues", &e))?;
    if liste.Size().map_err(|e| moteur("langues", &e))? == 0 {
        return Err(ErreurMoteur::AucuneLangue);
    }
    let premiere = liste.GetAt(0).map_err(|e| moteur("langues", &e))?;
    match OcrEngine::TryCreateFromLanguage(&premiere) {
        Ok(m) => Ok(m),
        Err(e) if est_nul(&e) => Err(ErreurMoteur::AucuneLangue),
        Err(e) => Err(moteur("creation", &e)),
    }
}

/// Les lignes et leurs mots, dans l'ordre de lecture du moteur.
fn lignes(resultat: &OcrResult) -> Result<Vec<LigneBrute>, Error> {
    let lignes = resultat.Lines()?;
    let mut rendu = Vec::new();
    for i in 0..lignes.Size()? {
        let ligne = lignes.GetAt(i)?;
        let mots_ocr = ligne.Words()?;
        let mut mots = Vec::new();
        for j in 0..mots_ocr.Size()? {
            let mot = mots_ocr.GetAt(j)?;
            let boite = mot.BoundingRect()?;
            mots.push(MotBrut {
                texte: mot.Text()?.to_string_lossy(),
                x: boite.X,
                y: boite.Y,
                largeur: boite.Width,
                hauteur: boite.Height,
            });
        }
        rendu.push(LigneBrute {
            texte: ligne.Text()?.to_string_lossy(),
            mots,
        });
    }
    Ok(rendu)
}
