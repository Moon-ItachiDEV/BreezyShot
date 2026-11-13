//! `Windows.Media.Ocr`, toujours hors du fil principal (fil `texte`, fil `texte-langues`).
//!
//! L'appartement à plusieurs fils (MTA) est tenu pour tout le processus par un seul
//! `CoIncrementMTAUsage`, jamais rendu : la crate `windows` garde ses fabriques WinRT
//! (`OcrEngine`…) dans des statiques, et un MTA défait entre deux lectures les laisserait
//! pointer dans un appartement mort. Les fils qui lisent n'initialisent rien eux-mêmes : un
//! fil sans appartement est dans le MTA implicite ; un fil déjà à un fil (STA) le reste, les
//! objets de la lecture sont agiles.
//!
//! Le moteur est créé à chaque lecture puis relâché (R15). La lecture (`RecognizeAsync`)
//! est attendue dans le fil appelant, au plus `ATTENTE_MAX` : rien n'attend sur le fil
//! principal, et un moteur qui ne rend pas ne garde pas le fil.
//!
//! `TryCreateFrom…` rend `null` (sans erreur) quand aucune langue ne convient : la crate
//! `windows` le change en une erreur de code 0, que `est_nul` reconnaît.

use std::sync::mpsc;
use std::sync::OnceLock;
use std::time::Instant;

use ::windows::core::{Error, HRESULT};
use ::windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use ::windows::Media::Ocr::{OcrEngine, OcrResult};
use ::windows::Security::Cryptography::CryptographicBuffer;
use ::windows::Win32::System::Com::CoIncrementMTAUsage;

use super::{
    Disponibilite, ErreurMoteur, LigneBrute, MotBrut, Place, Reconnu, ATTENTE_MAX,
    DERNIERE_CREATION,
};

pub const FIN_DE_LIGNE: &str = "\r\n";

/// `OcrEngine::MaxImageDimension`, lu une fois (10000 sur la machine d'essai).
static DIMENSION_MAX: OnceLock<u32> = OnceLock::new();

/// Le MTA tenu pour tout le processus : le code de `CoIncrementMTAUsage`, demandé une fois.
static MTA: OnceLock<Result<(), HRESULT>> = OnceLock::new();

/// Tient le MTA (une fois par processus, jamais rendu).
fn tenir_mta() -> Result<(), Error> {
    let tenu = MTA.get_or_init(|| {
        // SAFETY : sans argument ; le témoin rendu n'est jamais passé à
        // `CoDecrementMTAUsage`, le MTA vit jusqu'à la fin du processus.
        unsafe { CoIncrementMTAUsage() }
            .map(|_temoin| ())
            .map_err(|e| e.code())
    });
    (*tenu).map_err(Error::from_hresult)
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
    if tenir_mta().is_err() {
        return Disponibilite::NonPrisEnCharge;
    }
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

pub fn dimension() -> Result<u32, ErreurMoteur> {
    if let Some(d) = DIMENSION_MAX.get() {
        return Ok(*d);
    }
    tenir_mta().map_err(|e| moteur("init", &e))?;
    // Gardée seulement si elle a pu être lue : un échec sera relu la fois suivante.
    match OcrEngine::MaxImageDimension() {
        Ok(d) => Ok(*DIMENSION_MAX.get_or_init(|| d)),
        Err(e) => Err(moteur("dimension", &e)),
    }
}

/// `place` suit la lecture jusqu'au rappel du moteur (rendue alors, même après l'échéance).
pub fn reconnaitre(
    bgra: &[u8],
    largeur: u32,
    hauteur: u32,
    place: Place<'static>,
) -> Result<Reconnu, ErreurMoteur> {
    tenir_mta().map_err(|e| moteur("init", &e))?;
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
            // Le moteur a fini : sa place est rendue avant que le fil qui attend reparte.
            drop(place);
            // Reçu par personne si la lecture a été abandonnée : sans importance.
            let _ = envoi.send(r);
        })
        .map_err(|e| moteur("lecture", &e))?;
    let resultat = match reception.recv_timeout(ATTENTE_MAX) {
        Ok(r) => r.map_err(|e| moteur("lecture", &e))?,
        Err(_) => {
            let _ = operation.Cancel();
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
