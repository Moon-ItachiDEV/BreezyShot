//! Media Foundation sous Windows (chantier B) : la garde F31, la recherche du codeur F32,
//! l'écrivain (F15, F16) et le lecteur de l'exemple `lire_film`. Repris de l'exemple `mf`
//! de la sonde (`sonde/film/rust/examples/mf.rs`), prouvé sur la machine Windows de GitHub :
//! types d'entrée NV12 et de sortie H.264, paramètres du codeur, piste AAC, relecture par
//! `relire_avec` corrigé (géométrie relue à chaque changement de type, largeur de ligne
//! réelle, zone d'affichage).
//!
//! COM et Media Foundation sont ouverts et refermés par chaque `Ecrivain`, dans son fil
//! (`film-video`), et par chaque recherche ou lecture, dans le fil qui l'appelle. Aucun
//! appel ne part avant `bibliotheques()`. Les erreurs ne portent que l'étape et le code
//! HRESULT (jamais le message de Windows, qui peut citer un chemin).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use ::windows::core::{Interface, GUID, HSTRING};
use ::windows::Win32::Media::MediaFoundation::*;
use ::windows::Win32::System::Com::StructuredStorage::PropVariantClear;
use ::windows::Win32::System::Com::{
    CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_MULTITHREADED,
};
use ::windows::Win32::System::LibraryLoader::{LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32};

use super::{
    copier_zone, duree_son, sans_codeur_demande, taille_nv12, unites, vers_nv12, Codeur,
    Horodatage, Lecture, Parametres, Pixels, Zone, DEBIT_SON_BIT_S, TAUX_SON, VARIABLE_SANS_CODEUR,
};

/// Les DLL de Media Foundation que l'app appelle (chargées à la demande, `build.rs`).
const DLL: [&str; 3] = ["mfplat.dll", "mfreadwrite.dll", "mf.dll"];

/// Images écrites pendant lesquelles un refus du codeur matériel fait recommencer le
/// fichier avec le codeur logiciel (F16).
const IMAGES_A_L_ESSAI: u64 = 30;

/// Paramètres du codeur (S2 point 5, acceptés par le codeur logiciel) : mode « qualité »,
/// qualité 70, image clé toutes les 2 s.
const QUALITE: u32 = 70;
const SECONDES_PAR_IMAGE_CLE: u32 = 2;

const FERME: &str = "ecrivain ferme";

// ---------------------------------------------------------------------------------------
// Erreurs, COM, Media Foundation

/// Le code d'une erreur COM : `0xC00D36B4`.
fn code(e: &::windows::core::Error) -> String {
    format!("{:#010X}", e.code().0 as u32)
}

/// `etape` et le code : « SetInputMediaType 0xC00D36B4 ».
fn ctx<T>(etape: &str, r: ::windows::core::Result<T>) -> Result<T, String> {
    r.map_err(|e| format!("{etape} {}", code(&e)))
}

/// Le code à la fin d'une erreur rendue par `ctx`.
fn code_de(erreur: &str) -> &str {
    erreur.rsplit(' ').next().unwrap_or(erreur)
}

/// COM de ce fil, en appartement multiple ; refermé à la fin s'il a été ouvert ici (un
/// fil déjà en appartement simple le garde, et Media Foundation y marche aussi).
#[derive(Debug)]
struct Com {
    fermer: bool,
}

impl Com {
    fn ouvrir() -> Com {
        // SAFETY : initialisation de COM pour le fil appelant, refermée par `Drop`.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Com { fermer: hr.is_ok() }
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.fermer {
            // SAFETY : équilibre le `CoInitializeEx` réussi de ce fil.
            unsafe { CoUninitialize() };
        }
    }
}

/// Media Foundation démarré ; `MFShutdown` à la fin (compté par processus).
#[derive(Debug)]
struct Mf;

impl Mf {
    fn demarrer(drapeaux: u32) -> Result<Mf, String> {
        // SAFETY : appelé seulement après la garde F31 (DLL chargées).
        ctx("MFStartup", unsafe { MFStartup(MF_VERSION, drapeaux) })?;
        Ok(Mf)
    }
}

impl Drop for Mf {
    fn drop(&mut self) {
        // SAFETY : équilibre le `MFStartup` réussi.
        let _ = unsafe { MFShutdown() };
    }
}

fn attributs(n: u32) -> Result<IMFAttributes, String> {
    let mut a = None;
    // SAFETY : sortie Option, remplie par Media Foundation.
    ctx("MFCreateAttributes", unsafe {
        MFCreateAttributes(&mut a, n)
    })?;
    a.ok_or_else(|| "MFCreateAttributes vide".into())
}

fn paire(a: u32, b: u32) -> u64 {
    (u64::from(a) << 32) | u64::from(b)
}

// ---------------------------------------------------------------------------------------
// Garde (F31) et recherche du codeur (F32)

static GARDE: OnceLock<bool> = OnceLock::new();

pub fn bibliotheques() -> bool {
    *GARDE.get_or_init(|| {
        let variable = std::env::var_os(VARIABLE_SANS_CODEUR);
        let ok = garde(variable.as_deref(), &DLL);
        if !ok {
            log::info!("film: media foundation absente");
        }
        ok
    })
}

/// Vrai si la variable d'essai ne demande pas l'absence et si chaque DLL se charge depuis
/// le dossier système (la première qui manque arrête tout : aucune autre n'est essayée).
/// Les modules chargés le restent : les appels chargés à la demande les trouveront.
fn garde(variable: Option<&std::ffi::OsStr>, noms: &[&str]) -> bool {
    !sans_codeur_demande(variable)
        && noms.iter().all(|nom| {
            let n = HSTRING::from(*nom);
            // SAFETY : nom terminé par zéro, tenu pendant l'appel ; aucune fonction de la
            // DLL n'est appelée ici.
            unsafe { LoadLibraryExW(&n, None, LOAD_LIBRARY_SEARCH_SYSTEM32) }.is_ok()
        })
}

pub fn codeur_h264_present() -> bool {
    if !bibliotheques() {
        return false;
    }
    let _com = Com::ouvrir();
    let Ok(_mf) = Mf::demarrer(MFSTARTUP_LITE) else {
        return false;
    };
    codeurs_h264() > 0
}

/// Les codeurs H.264 de `MFTEnumEx`, comptés **sans en activer aucun** : chaque
/// `IMFActivate` est rendu, puis le tableau.
fn codeurs_h264() -> u32 {
    let sortie = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_H264,
    };
    let mut tableau: *mut Option<IMFActivate> = std::ptr::null_mut();
    let mut n = 0u32;
    // SAFETY : tableau alloué par MFTEnumEx ; chaque élément rendu (Release, jamais
    // ActivateObject), puis le tableau libéré par CoTaskMemFree.
    unsafe {
        if MFTEnumEx(
            MFT_CATEGORY_VIDEO_ENCODER,
            MFT_ENUM_FLAG_ALL,
            None,
            Some(&sortie),
            &mut tableau,
            &mut n,
        )
        .is_err()
            || tableau.is_null()
        {
            return 0;
        }
        for x in std::slice::from_raw_parts_mut(tableau, n as usize) {
            drop(x.take());
        }
        CoTaskMemFree(Some(tableau as *const core::ffi::c_void));
    }
    n
}

// ---------------------------------------------------------------------------------------
// Types de média

/// La sortie : H.264, profil « High » (`haut`), débit moyen, BT.709 en plage réduite.
fn type_sortie(p: &Parametres, haut: bool) -> Result<IMFMediaType, String> {
    // SAFETY : objet COM créé ici ; attributs posés par clés connues.
    unsafe {
        let t = ctx("MFCreateMediaType", MFCreateMediaType())?;
        ctx(
            "MF_MT_MAJOR_TYPE",
            t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video),
        )?;
        ctx(
            "MF_MT_SUBTYPE",
            t.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264),
        )?;
        ctx(
            "MF_MT_AVG_BITRATE",
            t.SetUINT32(&MF_MT_AVG_BITRATE, p.debit_bit_s),
        )?;
        if haut {
            ctx(
                "MF_MT_MPEG2_PROFILE",
                t.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_High.0 as u32),
            )?;
        }
        ctx(
            "MF_MT_VIDEO_PRIMARIES",
            t.SetUINT32(&MF_MT_VIDEO_PRIMARIES, MFVideoPrimaries_BT709.0 as u32),
        )?;
        ctx(
            "MF_MT_TRANSFER_FUNCTION",
            t.SetUINT32(&MF_MT_TRANSFER_FUNCTION, MFVideoTransFunc_709.0 as u32),
        )?;
        poser_image(&t, p)?;
        Ok(t)
    }
}

/// L'entrée : NV12 en mémoire centrale, de haut en bas, ligne de `largeur` octets.
fn type_entree(p: &Parametres) -> Result<IMFMediaType, String> {
    // SAFETY : idem.
    unsafe {
        let t = ctx("MFCreateMediaType", MFCreateMediaType())?;
        ctx(
            "MF_MT_MAJOR_TYPE",
            t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video),
        )?;
        ctx(
            "MF_MT_SUBTYPE",
            t.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12),
        )?;
        ctx(
            "MF_MT_DEFAULT_STRIDE",
            t.SetUINT32(&MF_MT_DEFAULT_STRIDE, p.largeur),
        )?;
        poser_image(&t, p)?;
        Ok(t)
    }
}

/// Ce que l'entrée et la sortie partagent : matrice, plage, balayage, taille, cadence.
///
/// # Safety
/// `t` est un type de média vivant.
unsafe fn poser_image(t: &IMFMediaType, p: &Parametres) -> Result<(), String> {
    // SAFETY : attributs posés par clés connues sur un type vivant.
    unsafe {
        ctx(
            "MF_MT_YUV_MATRIX",
            t.SetUINT32(&MF_MT_YUV_MATRIX, MFVideoTransferMatrix_BT709.0 as u32),
        )?;
        ctx(
            "MF_MT_VIDEO_NOMINAL_RANGE",
            t.SetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE, MFNominalRange_16_235.0 as u32),
        )?;
        ctx(
            "MF_MT_INTERLACE_MODE",
            t.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32),
        )?;
        ctx(
            "MF_MT_FRAME_SIZE",
            t.SetUINT64(&MF_MT_FRAME_SIZE, paire(p.largeur, p.hauteur)),
        )?;
        ctx(
            "MF_MT_FRAME_RATE",
            t.SetUINT64(&MF_MT_FRAME_RATE, paire(p.cadence, 1)),
        )?;
        ctx(
            "MF_MT_PIXEL_ASPECT_RATIO",
            t.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, paire(1, 1)),
        )
    }
}

/// Le son : AAC à 96 kbit/s en sortie (`aac`), PCM 48 kHz mono 16 bits en entrée.
fn type_son(aac: bool) -> Result<IMFMediaType, String> {
    // SAFETY : idem.
    unsafe {
        let t = ctx("MFCreateMediaType", MFCreateMediaType())?;
        ctx(
            "MF_MT_MAJOR_TYPE",
            t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio),
        )?;
        let sous_type = if aac {
            &MFAudioFormat_AAC
        } else {
            &MFAudioFormat_PCM
        };
        ctx("MF_MT_SUBTYPE", t.SetGUID(&MF_MT_SUBTYPE, sous_type))?;
        ctx(
            "MF_MT_AUDIO_BITS_PER_SAMPLE",
            t.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16),
        )?;
        ctx(
            "MF_MT_AUDIO_SAMPLES_PER_SECOND",
            t.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, TAUX_SON),
        )?;
        ctx(
            "MF_MT_AUDIO_NUM_CHANNELS",
            t.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 1),
        )?;
        let octets_par_s = if aac {
            DEBIT_SON_BIT_S / 8
        } else {
            ctx(
                "MF_MT_AUDIO_BLOCK_ALIGNMENT",
                t.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 2),
            )?;
            TAUX_SON * 2
        };
        ctx(
            "MF_MT_AUDIO_AVG_BYTES_PER_SECOND",
            t.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, octets_par_s),
        )?;
        Ok(t)
    }
}

/// Les paramètres du codeur passés à `SetInputMediaType` (clés `CODECAPI_*`).
fn parametres_codeur(p: &Parametres) -> Result<IMFAttributes, String> {
    let a = attributs(3)?;
    // SAFETY : attributs créés ici, clés connues.
    unsafe {
        ctx(
            "CODECAPI_AVEncCommonRateControlMode",
            a.SetUINT32(
                &CODECAPI_AVEncCommonRateControlMode,
                eAVEncCommonRateControlMode_Quality.0 as u32,
            ),
        )?;
        ctx(
            "CODECAPI_AVEncCommonQuality",
            a.SetUINT32(&CODECAPI_AVEncCommonQuality, QUALITE),
        )?;
        ctx(
            "CODECAPI_AVEncMPVGOPSize",
            a.SetUINT32(
                &CODECAPI_AVEncMPVGOPSize,
                p.cadence.saturating_mul(SECONDES_PAR_IMAGE_CLE),
            ),
        )?;
    }
    Ok(a)
}

// ---------------------------------------------------------------------------------------
// Pannes simulées du codeur matériel (tests du repli F16 sur une machine sans lui)

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Panne {
    /// `SetInputMediaType` refusé.
    Type,
    /// `BeginWriting` refusé.
    Demarrage,
    /// `WriteSample` refusé pour l'image écrite de ce rang.
    Image(u64),
}

#[cfg(test)]
thread_local! {
    static PANNE: std::cell::Cell<Option<Panne>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
fn panne() -> Option<Panne> {
    PANNE.with(|p| p.get())
}

#[cfg(test)]
fn panne_simulee(materiel: bool, cas: Panne, etape: &str) -> Result<(), String> {
    if materiel && panne() == Some(cas) {
        return Err(format!("{etape} 0x80004005"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------
// L'écrivain

/// Un `IMFSinkWriter` ouvert sur le fichier, `BeginWriting` fait.
struct Ouvert {
    sw: IMFSinkWriter,
    video: u32,
    audio: Option<u32>,
    codeur: Codeur,
}

/// Arrête le récepteur du fichier (le MP4) tout de suite : le fichier est relâché, les
/// écritures suivantes échouent. Sert l'abandon et les échecs.
fn arreter_recepteur(sw: &IMFSinkWriter) {
    let mut p: *mut core::ffi::c_void = std::ptr::null_mut();
    // SAFETY : interface demandée par son IID, reprise par `from_raw` si rendue.
    unsafe {
        if sw
            .GetServiceForStream(
                MF_SINK_WRITER_MEDIASINK.0,
                &GUID::zeroed(),
                &IMFMediaSink::IID,
                &mut p,
            )
            .is_ok()
            && !p.is_null()
        {
            let recepteur = IMFMediaSink::from_raw(p);
            let _ = recepteur.Shutdown();
        }
    }
}

/// Ouvre un écrivain sur `chemin` (le fichier est d'abord effacé), codeur matériel permis
/// ou non, profil « High » et paramètres du codeur (`complet`) ou débit moyen seul.
fn ouvrir(chemin: &Path, p: &Parametres, materiel: bool, complet: bool) -> Result<Ouvert, String> {
    let _ = std::fs::remove_file(chemin);
    let a = attributs(2)?;
    let url = HSTRING::from(chemin.as_os_str());
    // SAFETY : objets COM créés ici, chemin et types valides.
    let sw = unsafe {
        ctx(
            "MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS",
            a.SetUINT32(
                &MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS,
                u32::from(materiel),
            ),
        )?;
        ctx(
            "MF_TRANSCODE_CONTAINERTYPE",
            a.SetGUID(&MF_TRANSCODE_CONTAINERTYPE, &MFTranscodeContainerType_MPEG4),
        )?;
        ctx(
            "MFCreateSinkWriterFromURL",
            MFCreateSinkWriterFromURL(&url, None::<&IMFByteStream>, &a),
        )?
    };
    let pose = (|| -> Result<Ouvert, String> {
        // SAFETY : écrivain vivant, types créés ici.
        unsafe {
            let video = ctx("AddStream", sw.AddStream(&type_sortie(p, complet)?))?;
            let entree = type_entree(p)?;
            #[cfg(test)]
            panne_simulee(materiel, Panne::Type, "SetInputMediaType")?;
            if complet {
                let parametres = parametres_codeur(p)?;
                if sw.SetInputMediaType(video, &entree, &parametres).is_err() {
                    // Paramètres refusés par ce codeur : le débit moyen seul (F15).
                    ctx(
                        "SetInputMediaType",
                        sw.SetInputMediaType(video, &entree, None),
                    )?;
                }
            } else {
                ctx(
                    "SetInputMediaType",
                    sw.SetInputMediaType(video, &entree, None),
                )?;
            }
            let audio = if p.son {
                let s = ctx("AddStream (son)", sw.AddStream(&type_son(true)?))?;
                ctx(
                    "SetInputMediaType (son)",
                    sw.SetInputMediaType(s, &type_son(false)?, None),
                )?;
                Some(s)
            } else {
                None
            };
            #[cfg(test)]
            panne_simulee(materiel, Panne::Demarrage, "BeginWriting")?;
            ctx("BeginWriting", sw.BeginWriting())?;
            let codeur = codeur_pris(&sw, video, materiel);
            Ok(Ouvert {
                sw: sw.clone(),
                video,
                audio,
                codeur,
            })
        }
    })();
    if pose.is_err() {
        arreter_recepteur(&sw);
        drop(sw);
        let _ = std::fs::remove_file(chemin);
    }
    pose
}

/// Le codeur de la chaîne du flux vidéo : matériel s'il porte une adresse ou un vendeur
/// de matériel. Matériel demandé mais impossible à lire : matériel (le repli reste armé).
fn codeur_pris(sw: &IMFSinkWriter, video: u32, materiel: bool) -> Codeur {
    #[cfg(test)]
    if materiel && panne().is_some() {
        return Codeur::Materiel;
    }
    if !materiel {
        return Codeur::Logiciel;
    }
    let Ok(ex) = sw.cast::<IMFSinkWriterEx>() else {
        return Codeur::Materiel;
    };
    for i in 0..8 {
        let mut categorie = GUID::zeroed();
        let mut t: Option<IMFTransform> = None;
        // SAFETY : sorties valides ; la transformation n'est que lue.
        unsafe {
            if ex
                .GetTransformForStream(video, i, Some(&mut categorie), &mut t)
                .is_err()
            {
                break;
            }
            if categorie != MFT_CATEGORY_VIDEO_ENCODER {
                continue;
            }
            let Some(a) = t.and_then(|t| t.GetAttributes().ok()) else {
                return Codeur::Materiel;
            };
            let du_materiel = a.GetStringLength(&MFT_ENUM_HARDWARE_URL_Attribute).is_ok()
                || a.GetStringLength(&MFT_ENUM_HARDWARE_VENDOR_ID_Attribute)
                    .is_ok();
            return if du_materiel {
                Codeur::Materiel
            } else {
                Codeur::Logiciel
            };
        }
    }
    Codeur::Materiel
}

/// Ouvre avec le profil et les paramètres, puis, refusé, avec le débit moyen seul.
fn ouvrir_au_mieux(chemin: &Path, p: &Parametres, materiel: bool) -> Result<Ouvert, String> {
    ouvrir(chemin, p, materiel, true).or_else(|e| ouvrir(chemin, p, materiel, false).map_err(|_| e))
}

#[derive(Debug)]
pub struct Ecrivain {
    sw: Option<IMFSinkWriter>,
    video: u32,
    audio: Option<u32>,
    codeur: Codeur,
    p: Parametres,
    chemin: PathBuf,
    horodatage: Horodatage,
    /// La dernière image donnée : elle attend la suivante pour être écrite, et
    /// `meme_image` la redonne.
    derniere: Option<IMFMediaBuffer>,
    /// Images vidéo écrites dans ce fichier.
    ecrites: u64,
    /// Le repli sur le codeur logiciel a déjà eu lieu (une fois au plus).
    repli_fait: bool,
    // Dans cet ordre : Media Foundation est arrêté avant COM, après les objets ci-dessus.
    _mf: Mf,
    _com: Com,
}

pub fn creer(chemin: &Path, p: &Parametres) -> Result<Ecrivain, String> {
    if !bibliotheques() {
        return Err("media foundation absente".into());
    }
    let com = Com::ouvrir();
    let mf = Mf::demarrer(MFSTARTUP_FULL)?;
    let ouvert = if p.materiel {
        match ouvrir_au_mieux(chemin, p, true) {
            Ok(o) => o,
            Err(refus) => {
                // Écrit seulement si le logiciel prend le relais : un refus qui ne vient
                // pas du codeur matériel n'est pas mis à son compte.
                let o = ouvrir_au_mieux(chemin, p, false)?;
                log::info!(
                    "film codeur logiciel (materiel refuse: {})",
                    code_de(&refus)
                );
                o
            }
        }
    } else {
        ouvrir_au_mieux(chemin, p, false)?
    };
    Ok(Ecrivain {
        sw: Some(ouvert.sw),
        video: ouvert.video,
        audio: ouvert.audio,
        codeur: ouvert.codeur,
        p: *p,
        chemin: chemin.to_path_buf(),
        horodatage: Horodatage::nouveau(p.cadence),
        derniere: None,
        ecrites: 0,
        repli_fait: !p.materiel,
        _mf: mf,
        _com: com,
    })
}

impl Ecrivain {
    fn ecrivain(&self) -> Result<&IMFSinkWriter, String> {
        self.sw.as_ref().ok_or_else(|| FERME.to_string())
    }

    /// Un tampon de Media Foundation de `n` octets, rempli par `remplir`.
    fn tampon(
        n: usize,
        remplir: impl FnOnce(&mut [u8]) -> Result<(), String>,
    ) -> Result<IMFMediaBuffer, String> {
        let n32 = u32::try_from(n).map_err(|_| format!("tampon de {n} octets"))?;
        // SAFETY : tampon créé ici, verrouillé le temps du remplissage, sur sa longueur
        // maximale vérifiée.
        unsafe {
            let b = ctx("MFCreateMemoryBuffer", MFCreateMemoryBuffer(n32))?;
            let mut p: *mut u8 = std::ptr::null_mut();
            let mut max = 0u32;
            ctx("Lock", b.Lock(&mut p, Some(&mut max), None))?;
            let r = if p.is_null() || (max as usize) < n {
                Err(format!("Lock {max} octets"))
            } else {
                remplir(std::slice::from_raw_parts_mut(p, n))
            };
            let _ = b.Unlock();
            r?;
            ctx("SetCurrentLength", b.SetCurrentLength(n32))?;
            Ok(b)
        }
    }

    fn ecrire(
        &self,
        flux: u32,
        tampon: &IMFMediaBuffer,
        debut: i64,
        duree: i64,
    ) -> Result<(), String> {
        let sw = self.ecrivain()?;
        // SAFETY : échantillon créé ici autour d'un tampon complet ; flux ouvert.
        unsafe {
            let s = ctx("MFCreateSample", MFCreateSample())?;
            ctx("AddBuffer", s.AddBuffer(tampon))?;
            ctx("SetSampleTime", s.SetSampleTime(debut))?;
            ctx("SetSampleDuration", s.SetSampleDuration(duree))?;
            ctx("WriteSample", sw.WriteSample(flux, &s))
        }
    }

    /// Écrit une image vidéo ; un refus du codeur matériel dans les 30 premières images
    /// fait recommencer le fichier avec le codeur logiciel (F16).
    fn ecrire_image(
        &mut self,
        tampon: &IMFMediaBuffer,
        debut: i64,
        duree: i64,
    ) -> Result<(), String> {
        #[cfg(test)]
        let simulee = if self.codeur == Codeur::Materiel {
            panne_simulee(true, Panne::Image(self.ecrites), "WriteSample")
        } else {
            Ok(())
        };
        #[cfg(not(test))]
        let simulee = Ok(());
        match simulee.and_then(|()| self.ecrire(self.video, tampon, debut, duree)) {
            Ok(()) => {
                self.ecrites += 1;
                Ok(())
            }
            Err(e)
                if self.codeur == Codeur::Materiel
                    && !self.repli_fait
                    && self.ecrites < IMAGES_A_L_ESSAI =>
            {
                self.recommencer_en_logiciel()?;
                log::info!("film codeur logiciel (materiel refuse: {})", code_de(&e));
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Le fichier recommencé avec le codeur logiciel : l'image en attente couvre depuis
    /// 0, et la piste son est refaite en silence jusqu'où elle allait.
    fn recommencer_en_logiciel(&mut self) -> Result<(), String> {
        self.repli_fait = true;
        if let Some(sw) = self.sw.take() {
            arreter_recepteur(&sw);
        }
        let o = ouvrir_au_mieux(&self.chemin, &self.p, false)?;
        self.sw = Some(o.sw);
        self.video = o.video;
        self.audio = o.audio;
        self.codeur = o.codeur;
        self.ecrites = 0;
        self.horodatage.repartir();
        if let Some(flux) = self.audio {
            let total = (self.horodatage.son_fin.max(0) as u128 * u128::from(TAUX_SON) / 10_000_000)
                as usize;
            let morceau = TAUX_SON as usize / 10;
            let mut fait = 0usize;
            while fait < total {
                let n = morceau.min(total - fait);
                let debut = unites(duree_son(fait));
                let fin = unites(duree_son(fait + n));
                let t = Self::tampon(n * 2, |s| {
                    s.fill(0);
                    Ok(())
                })?;
                self.ecrire(flux, &t, debut, (fin - debut).max(1))?;
                fait += n;
            }
        }
        Ok(())
    }

    /// Donne `tampon` à `instant` : l'image en attente est écrite pour l'écart réel.
    fn donner(&mut self, tampon: IMFMediaBuffer, instant: Duration) -> Result<(), String> {
        let a_ecrire = self.horodatage.image(instant)?;
        let avant = self.derniere.replace(tampon);
        if let (Some((debut, duree)), Some(avant)) = (a_ecrire, avant) {
            self.ecrire_image(&avant, debut, duree)?;
        }
        Ok(())
    }

    /// La dernière image puis `Finalize`.
    fn fermer(&mut self) -> Result<(), String> {
        self.ecrivain()?;
        if let (Some((debut, duree)), Some(tampon)) = (self.horodatage.fin(), self.derniere.take())
        {
            self.ecrire(self.video, &tampon, debut, duree)?;
            self.ecrites += 1;
        }
        let sw = self.ecrivain()?;
        // SAFETY : écrivain commencé.
        ctx("Finalize", unsafe { sw.Finalize() })?;
        self.sw = None;
        Ok(())
    }
}

impl Drop for Ecrivain {
    fn drop(&mut self) {
        // Pas fini (abandon, erreur, panique) : le récepteur est arrêté, le fichier relâché.
        if let Some(sw) = self.sw.take() {
            arreter_recepteur(&sw);
        }
        self.derniere = None;
    }
}

pub fn codeur(e: &Ecrivain) -> Codeur {
    e.codeur
}

pub fn image(e: &mut Ecrivain, pixels: &Pixels, instant: Duration) -> Result<Duration, String> {
    let debut = Instant::now();
    e.ecrivain()?;
    let (l, h) = (e.p.largeur, e.p.hauteur);
    if (pixels.largeur, pixels.hauteur) != (l, h) {
        return Err(format!(
            "image de {}x{} pour {l}x{h}",
            pixels.largeur, pixels.hauteur
        ));
    }
    e.horodatage.peut(instant)?;
    let n = taille_nv12(l, h).ok_or("image trop grande")?;
    let tampon = Ecrivain::tampon(n, |s| vers_nv12(&pixels.rgba, l, h, s))?;
    e.donner(tampon, instant)?;
    Ok(debut.elapsed())
}

pub fn meme_image(e: &mut Ecrivain, instant: Duration) -> Result<Duration, String> {
    let debut = Instant::now();
    e.ecrivain()?;
    let tampon = e.derniere.clone().ok_or("aucune image a redonner")?;
    e.donner(tampon, instant)?;
    Ok(debut.elapsed())
}

pub fn son(e: &mut Ecrivain, pcm: &[i16], instant: Duration) -> Result<(), String> {
    let Some(flux) = e.audio else {
        return Ok(());
    };
    if pcm.is_empty() {
        return Ok(());
    }
    e.ecrivain()?;
    let (debut, duree) = e.horodatage.son(pcm.len(), instant)?;
    let tampon = Ecrivain::tampon(pcm.len() * 2, |s| {
        for (d, x) in s.chunks_exact_mut(2).zip(pcm) {
            d.copy_from_slice(&x.to_le_bytes());
        }
        Ok(())
    })?;
    e.ecrire(flux, &tampon, debut, duree)
}

pub fn finir(mut e: Ecrivain) -> Result<Duration, String> {
    let debut = Instant::now();
    // En erreur, `Drop` arrête le récepteur : le fichier est relâché.
    e.fermer()?;
    Ok(debut.elapsed())
}

pub fn abandonner(e: Ecrivain) {
    drop(e);
}

// ---------------------------------------------------------------------------------------
// Le lecteur (exemple `lire_film`, tests)

/// La géométrie du type courant de la sortie du lecteur : la taille du tampon, le pas
/// d'une ligne (`MF_MT_DEFAULT_STRIDE`, négatif de bas en haut) et la zone d'affichage
/// (`MF_MT_MINIMUM_DISPLAY_APERTURE`, sinon le tampon entier).
#[derive(Debug, Clone, Copy)]
struct Geometrie {
    largeur: u32,
    hauteur: u32,
    pas: Option<i32>,
    zone: Zone,
}

/// # Safety
/// `t` est un type de média vivant.
unsafe fn geometrie(t: &IMFMediaType) -> Result<Geometrie, String> {
    // SAFETY : lectures d'attributs d'un type vivant ; le tampon a la taille d'un
    // MFVideoArea, lu sans alignement.
    unsafe {
        let taille = ctx("MF_MT_FRAME_SIZE", t.GetUINT64(&MF_MT_FRAME_SIZE))?;
        let (largeur, hauteur) = ((taille >> 32) as u32, (taille & 0xFFFF_FFFF) as u32);
        let pas = t.GetUINT32(&MF_MT_DEFAULT_STRIDE).ok().map(|s| s as i32);
        let mut zone = Zone {
            x: 0,
            y: 0,
            largeur,
            hauteur,
        };
        let mut blob = [0u8; std::mem::size_of::<MFVideoArea>()];
        let mut n = 0u32;
        if t.GetBlob(&MF_MT_MINIMUM_DISPLAY_APERTURE, &mut blob, Some(&mut n))
            .is_ok()
            && n as usize >= blob.len()
        {
            let v: MFVideoArea = std::ptr::read_unaligned(blob.as_ptr() as *const MFVideoArea);
            let (x, y) = (v.OffsetX.value.max(0) as u32, v.OffsetY.value.max(0) as u32);
            let (l, h) = (v.Area.cx.max(0) as u32, v.Area.cy.max(0) as u32);
            if l > 0
                && h > 0
                && u64::from(x) + u64::from(l) <= u64::from(largeur)
                && u64::from(y) + u64::from(h) <= u64::from(hauteur)
            {
                zone = Zone {
                    x,
                    y,
                    largeur: l,
                    hauteur: h,
                };
            }
        }
        Ok(Geometrie {
            largeur,
            hauteur,
            pas,
            zone,
        })
    }
}

/// # Safety
/// `r` est un lecteur vivant et `flux` son flux vidéo.
unsafe fn sortie_rgb32(r: &IMFSourceReader, flux: u32) -> Result<(), String> {
    // SAFETY : type créé ici, donné au lecteur.
    unsafe {
        let t = ctx("MFCreateMediaType", MFCreateMediaType())?;
        ctx(
            "MF_MT_MAJOR_TYPE",
            t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video),
        )?;
        ctx(
            "MF_MT_SUBTYPE",
            t.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32),
        )?;
        ctx("SetCurrentMediaType", r.SetCurrentMediaType(flux, None, &t))
    }
}

/// Copie l'image d'un échantillon décodé (RGB32) dans `img`, selon `g`.
///
/// # Safety
/// `s` est un échantillon vivant rendu par le lecteur.
/// Rend la largeur de ligne lue (octets, négative de bas en haut).
unsafe fn copier_image(s: &IMFSample, g: &Geometrie, img: &mut Pixels) -> Result<isize, String> {
    // SAFETY : tampon verrouillé le temps de la copie ; la copie ne lit que dans la tranche
    // [début, début + longueur) que Windows rend, chaque ligne vérifiée.
    unsafe {
        let b = ctx("ConvertToContiguousBuffer", s.ConvertToContiguousBuffer())?;
        if let Ok(b2) = b.cast::<IMF2DBuffer2>() {
            let mut ligne0: *mut u8 = std::ptr::null_mut();
            let mut pas = 0i32;
            let mut debut: *mut u8 = std::ptr::null_mut();
            let mut longueur = 0u32;
            ctx(
                "Lock2DSize",
                b2.Lock2DSize(
                    MF2DBuffer_LockFlags_Read,
                    &mut ligne0,
                    &mut pas,
                    &mut debut,
                    &mut longueur,
                ),
            )?;
            let ok = !debut.is_null() && {
                let donnees = std::slice::from_raw_parts(debut as *const u8, longueur as usize);
                let decalage = (ligne0 as isize).wrapping_sub(debut as isize);
                copier_zone(donnees, decalage, pas as isize, g.zone, img)
            };
            let _ = b2.Unlock2D();
            return if ok {
                Ok(pas as isize)
            } else {
                Err(format!(
                    "image hors du tampon (pas {pas}, {longueur} octets)"
                ))
            };
        }
        let mut p: *mut u8 = std::ptr::null_mut();
        let mut longueur = 0u32;
        ctx("Lock", b.Lock(&mut p, None, Some(&mut longueur)))?;
        let pas = g.pas.unwrap_or((g.largeur as i32).saturating_mul(4)) as isize;
        // De bas en haut : la ligne 0 est la dernière du tampon.
        let ligne0 = if pas < 0 {
            (g.hauteur as isize - 1).saturating_mul(-pas)
        } else {
            0
        };
        let ok = !p.is_null() && {
            let donnees = std::slice::from_raw_parts(p as *const u8, longueur as usize);
            copier_zone(donnees, ligne0, pas, g.zone, img)
        };
        let _ = b.Unlock();
        if ok {
            Ok(pas)
        } else {
            Err(format!(
                "image hors du tampon (pas {pas}, {longueur} octets)"
            ))
        }
    }
}

pub fn lire(
    chemin: &Path,
    voir: &mut dyn FnMut(u64, Duration, &Pixels),
) -> Result<Lecture, String> {
    if !bibliotheques() {
        return Err("media foundation absente".into());
    }
    let _com = Com::ouvrir();
    let _mf = Mf::demarrer(MFSTARTUP_FULL)?;
    lire_ouvert(chemin, voir)
}

fn lire_ouvert(
    chemin: &Path,
    voir: &mut dyn FnMut(u64, Duration, &Pixels),
) -> Result<Lecture, String> {
    let fv = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
    let fa = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
    let a = attributs(1)?;
    let url = HSTRING::from(chemin.as_os_str());
    // SAFETY : objets COM créés ici ; sorties de ReadSample valides le temps de l'appel.
    unsafe {
        ctx(
            "MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING",
            a.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1),
        )?;
        let r = ctx(
            "MFCreateSourceReaderFromURL",
            MFCreateSourceReaderFromURL(&url, &a),
        )?;
        let son = r.GetNativeMediaType(fa, 0).is_ok();
        let h264 = r
            .GetNativeMediaType(fv, 0)
            .and_then(|t| t.GetGUID(&MF_MT_SUBTYPE))
            .is_ok_and(|g| g == MFVideoFormat_H264);
        let _ = r.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false);
        ctx("SetStreamSelection", r.SetStreamSelection(fv, true))?;
        sortie_rgb32(&r, fv)?;
        let mut g = geometrie(&ctx("GetCurrentMediaType", r.GetCurrentMediaType(fv))?)?;
        let duree = match r
            .GetPresentationAttribute(MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION)
        {
            Ok(mut pv) => {
                let d = pv.Anonymous.Anonymous.Anonymous.uhVal;
                let _ = PropVariantClear(&mut pv);
                d
            }
            Err(_) => 0,
        };
        let mut img = Pixels {
            largeur: 0,
            hauteur: 0,
            rgba: Vec::new(),
        };
        let mut images = 0u64;
        let mut dernier = None;
        let mut changements = 0u32;
        let mut vides = 0u32;
        let mut pas_ligne = 0isize;
        loop {
            let mut drapeaux = 0u32;
            let mut ts = 0i64;
            let mut s: Option<IMFSample> = None;
            ctx(
                "ReadSample",
                r.ReadSample(
                    fv,
                    0,
                    None,
                    Some(&mut drapeaux),
                    Some(&mut ts),
                    Some(&mut s),
                ),
            )
            .map_err(|e| format!("image {images}: {e}"))?;
            if drapeaux & MF_SOURCE_READERF_NATIVEMEDIATYPECHANGED.0 as u32 != 0 {
                // Le type d'origine a changé : la sortie RGB32 est redemandée.
                sortie_rgb32(&r, fv)?;
            }
            if drapeaux
                & (MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0
                    | MF_SOURCE_READERF_NATIVEMEDIATYPECHANGED.0) as u32
                != 0
            {
                g = geometrie(&ctx("GetCurrentMediaType", r.GetCurrentMediaType(fv))?)?;
                changements += 1;
            }
            if drapeaux & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                break;
            }
            let Some(s) = s else {
                vides += 1;
                if vides > 100_000 {
                    return Err(format!("image {images}: lecteur sans fin"));
                }
                continue;
            };
            vides = 0;
            pas_ligne =
                copier_image(&s, &g, &mut img).map_err(|e| format!("image {images}: {e}"))?;
            let instant = Duration::from_nanos((ts.max(0) as u64).saturating_mul(100));
            voir(images, instant, &img);
            dernier = Some(instant);
            images += 1;
        }
        Ok(Lecture {
            duree: Duration::from_nanos(duree.saturating_mul(100)),
            largeur: g.zone.largeur,
            hauteur: g.zone.hauteur,
            tampon_largeur: g.largeur,
            tampon_hauteur: g.hauteur,
            pas_ligne: pas_ligne as i64,
            zone_x: g.zone.x,
            zone_y: g.zone.y,
            images,
            dernier_instant: dernier,
            h264,
            son,
            changements_de_type: changements,
        })
    }
}

// ---------------------------------------------------------------------------------------
// Tests sur la machine Windows (GitHub). Ceux qui demandent un codeur se sautent en le
// disant s'il n'y en a pas ; la garde ne demande rien et ne se saute jamais. Les preuves
// qui demandent un processus neuf (modules chargés, poignées) tournent dans un enfant :
// l'exécutable des tests relancé sur un seul test.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systeme::mp4::{self as mp4, debit_vise};
    use ::windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use ::windows::Win32::System::Threading::{GetCurrentProcess, GetProcessHandleCount};
    use std::ffi::OsStr;
    use std::process::Command;

    const ENFANT: &str = "BREEZYSHOT_ESSAI_MP4_ENFANT";
    const FOND: [u8; 3] = [128, 128, 128];
    /// Les carrés des quatre coins : haut-gauche, haut-droit, bas-gauche, bas-droit.
    const COINS: [[u8; 3]; 4] = [[200, 60, 50], [50, 180, 80], [40, 70, 200], [230, 210, 40]];
    const COTE_COIN: u32 = 24;
    /// Écart permis par canal, lu au milieu d'un aplat (à 4 px au moins d'un bord : le
    /// 4:2:0 donne des écarts de dizaines sur les bords, D28).
    const ECART: i32 = 8;

    fn sans_codeur() -> bool {
        if mp4::codeur_h264_present() {
            return false;
        }
        eprintln!("saute : aucun codeur H.264 sur cette machine");
        true
    }

    fn params(largeur: u32, hauteur: u32, son: bool) -> Parametres {
        Parametres {
            largeur,
            hauteur,
            cadence: 30,
            debit_bit_s: debit_vise(largeur, hauteur),
            son,
            materiel: true,
        }
    }

    fn instant(n: u32) -> Duration {
        Duration::from_nanos(u64::from(n) * 1_000_000_000 / 30)
    }

    fn coins(l: u32, h: u32) -> [(u32, u32); 4] {
        let (d, b) = (l - COTE_COIN, h - COTE_COIN);
        [(0, 0), (d, 0), (0, b), (d, b)]
    }

    /// L'image `n` : fond gris, un carré de couleur connue à chaque coin, collé aux bords
    /// (donc aux derniers pixels quand les côtés ne sont pas des multiples de 16), et `n`
    /// écrit en 8 carrés noirs ou blancs de 16 × 16 en (32, 48).
    fn image_essai(l: u32, h: u32, n: u32) -> Pixels {
        let mut rgba = Vec::with_capacity((l * h * 4) as usize);
        for _ in 0..l * h {
            rgba.extend_from_slice(&[FOND[0], FOND[1], FOND[2], 255]);
        }
        let mut carre = |x0: u32, y0: u32, cote: u32, c: [u8; 3]| {
            for y in y0..y0 + cote {
                for x in x0..x0 + cote {
                    let i = ((y * l + x) * 4) as usize;
                    rgba[i..i + 3].copy_from_slice(&c);
                }
            }
        };
        for (i, &(x, y)) in coins(l, h).iter().enumerate() {
            carre(x, y, COTE_COIN, COINS[i]);
        }
        for k in 0..8 {
            let c = if (n >> k) & 1 == 1 { [255; 3] } else { [0; 3] };
            carre(32 + 16 * k, 48, 16, c);
        }
        Pixels {
            largeur: l,
            hauteur: h,
            rgba,
        }
    }

    fn px(img: &Pixels, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * img.largeur + x) * 4) as usize;
        [img.rgba[i], img.rgba[i + 1], img.rgba[i + 2]]
    }

    fn ecart(a: [u8; 3], b: [u8; 3]) -> i32 {
        (0..3)
            .map(|k| (i32::from(a[k]) - i32::from(b[k])).abs())
            .max()
            .unwrap_or(0)
    }

    fn numero(img: &Pixels) -> u32 {
        (0..8)
            .filter(|&k| px(img, 40 + 16 * k, 56)[1] > 128)
            .map(|k| 1 << k)
            .sum()
    }

    /// Les couleurs des coins au milieu de leur carré, à ± 8, l'image à l'endroit ; et
    /// chaque carré à sa place : 5 px en dedans de son bord intérieur il est là, 5 px en
    /// dehors c'est le fond.
    fn verifier_image(img: &Pixels, l: u32, h: u32) -> Result<(), String> {
        if (img.largeur, img.hauteur) != (l, h) {
            return Err(format!("image de {}x{}", img.largeur, img.hauteur));
        }
        let m = COTE_COIN / 2;
        for (i, &(x, y)) in coins(l, h).iter().enumerate() {
            let lu = px(img, x + m, y + m);
            if ecart(lu, COINS[i]) > ECART {
                return Err(format!("coin {i} en {},{} : {lu:?}", x + m, y + m));
            }
            // Le bord intérieur (vers le centre de l'image) en x puis en y.
            let (dedans_x, dehors_x) = if x == 0 {
                (COTE_COIN - 5, COTE_COIN + 5)
            } else {
                (x + 5, x - 5)
            };
            let (dedans_y, dehors_y) = if y == 0 {
                (COTE_COIN - 5, COTE_COIN + 5)
            } else {
                (y + 5, y - 5)
            };
            for (qx, qy, attendu, autre) in [
                (dedans_x, y + m, COINS[i], FOND),
                (dehors_x, y + m, FOND, COINS[i]),
                (x + m, dedans_y, COINS[i], FOND),
                (x + m, dehors_y, FOND, COINS[i]),
            ] {
                let lu = px(img, qx, qy);
                if ecart(lu, attendu) >= ecart(lu, autre) {
                    return Err(format!("coin {i} mal placé en {qx},{qy} : {lu:?}"));
                }
            }
        }
        Ok(())
    }

    struct Relu {
        lecture: Lecture,
        numeros: Vec<u32>,
        instants: Vec<Duration>,
        fautes: Vec<String>,
    }

    fn relire(chemin: &Path, l: u32, h: u32) -> Relu {
        let mut numeros = Vec::new();
        let mut instants = Vec::new();
        let mut fautes = Vec::new();
        let lecture = mp4::lire(chemin, |i, t, img| {
            numeros.push(numero(img));
            instants.push(t);
            if let Err(e) = verifier_image(img, l, h) {
                if fautes.len() < 5 {
                    fautes.push(format!("image {i} : {e}"));
                }
            }
        })
        .unwrap();
        eprintln!(
            "relu {}x{} : tampon {}x{}, ligne {} octets, zone {},{}, {} changements de type, \
             {} images, {:?}",
            lecture.largeur,
            lecture.hauteur,
            lecture.tampon_largeur,
            lecture.tampon_hauteur,
            lecture.pas_ligne,
            lecture.zone_x,
            lecture.zone_y,
            lecture.changements_de_type,
            lecture.images,
            lecture.duree
        );
        Relu {
            lecture,
            numeros,
            instants,
            fautes,
        }
    }

    fn mediane(mut v: Vec<Duration>) -> Duration {
        v.sort();
        v.get(v.len() / 2).copied().unwrap_or_default()
    }

    fn proche(a: Duration, b: Duration, marge_ms: u64) -> bool {
        a.abs_diff(b) <= Duration::from_millis(marge_ms)
    }

    /// Un film de 3 s (90 images faites ici) écrit puis relu : durée, nombre d'images,
    /// ordre, image à l'endroit, couleurs à ± 8, motif à sa place jusqu'aux derniers
    /// pixels. Le temps de la conversion et de l'écriture est noté (version de test).
    fn film_relu(l: u32, h: u32) {
        if sans_codeur() {
            return;
        }
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("film.mp4");
        let mut e = mp4::creer(&f, &params(l, h, false)).unwrap();
        eprintln!("{l}x{h} : codeur {}", e.codeur().nom());
        let mut ecriture = Vec::new();
        let mut conversion = Vec::new();
        let mut nv12 = vec![0u8; taille_nv12(l, h).unwrap()];
        for n in 0..90 {
            let img = image_essai(l, h, n);
            let c = Instant::now();
            vers_nv12(&img.rgba, l, h, &mut nv12).unwrap();
            conversion.push(c.elapsed());
            ecriture.push(e.image(&img, instant(n)).unwrap());
        }
        let fin = e.finir().unwrap();
        eprintln!(
            "{l}x{h} : conversion NV12 médiane {:?}, image (conversion et écriture) médiane \
             {:?}, finir {fin:?}, {} octets",
            mediane(conversion),
            mediane(ecriture),
            std::fs::metadata(&f).map(|m| m.len()).unwrap_or(0)
        );
        let r = relire(&f, l, h);
        assert_eq!((r.lecture.largeur, r.lecture.hauteur), (l, h));
        assert!(r.lecture.h264);
        assert!(!r.lecture.son);
        assert!(
            proche(r.lecture.duree, Duration::from_secs(3), 100),
            "{:?}",
            r.lecture.duree
        );
        assert!(
            (88..=92).contains(&r.lecture.images),
            "{}",
            r.lecture.images
        );
        assert!(r.fautes.is_empty(), "{:#?}", r.fautes);
        assert_eq!(r.numeros.first(), Some(&0));
        assert_eq!(r.numeros.last(), Some(&89));
        assert!(r.numeros.windows(2).all(|w| w[0] < w[1]), "{:?}", r.numeros);
        let dernier = r.lecture.dernier_instant.unwrap();
        assert!(proche(dernier, instant(89), 40), "{dernier:?}");
    }

    #[test]
    fn film_relu_1024x768() {
        film_relu(1024, 768);
    }

    /// Codé en 608 × 464 puis rogné (D28) : le lecteur suit la géométrie annoncée.
    #[test]
    fn film_relu_600x450() {
        film_relu(600, 450);
    }

    #[test]
    fn film_relu_602x452() {
        film_relu(602, 452);
    }

    /// `meme_image` redonne la dernière image, sans conversion, à l'instant demandé.
    #[test]
    fn film_meme_image() {
        if sans_codeur() {
            return;
        }
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("meme.mp4");
        let mut e = mp4::creer(&f, &params(600, 450, false)).unwrap();
        for n in 0..45 {
            if (15..30).contains(&n) {
                e.meme_image(instant(n)).unwrap();
            } else {
                e.image(&image_essai(600, 450, n), instant(n)).unwrap();
            }
        }
        e.finir().unwrap();
        let r = relire(&f, 600, 450);
        assert!(r.fautes.is_empty(), "{:#?}", r.fautes);
        assert!(
            (44..=46).contains(&r.lecture.images),
            "{}",
            r.lecture.images
        );
        let attendus: Vec<u32> = (0..45)
            .map(|n| if (15..30).contains(&n) { 14 } else { n })
            .collect();
        assert_eq!(r.numeros, attendus);
    }

    /// Des pas sautés (une seconde sans image) : le fichier garde les temps réels.
    #[test]
    fn film_garde_les_temps_reels() {
        if sans_codeur() {
            return;
        }
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("saut.mp4");
        let mut e = mp4::creer(&f, &params(600, 450, false)).unwrap();
        let pas: Vec<u32> = (0..30).chain(60..90).collect();
        for &n in &pas {
            e.image(&image_essai(600, 450, n), instant(n)).unwrap();
        }
        e.finir().unwrap();
        let r = relire(&f, 600, 450);
        assert!(r.fautes.is_empty(), "{:#?}", r.fautes);
        assert!(
            proche(r.lecture.duree, Duration::from_secs(3), 100),
            "{:?}",
            r.lecture.duree
        );
        assert_eq!(r.numeros, pas);
        for (n, t) in r.numeros.iter().zip(&r.instants) {
            assert!(proche(*t, instant(*n), 40), "image {n} à {t:?}");
        }
    }

    /// F16 : le codeur matériel (simulé) refuse le type, le démarrage ou une des 30
    /// premières images : le fichier est recommencé avec le codeur logiciel, et se relit.
    #[test]
    fn film_repli_logiciel() {
        if sans_codeur() {
            return;
        }
        for (cas, premier) in [
            (Panne::Type, 0),
            (Panne::Demarrage, 0),
            (Panne::Image(0), 1),
            (Panne::Image(5), 6),
            (Panne::Image(29), 30),
        ] {
            PANNE.with(|p| p.set(Some(cas)));
            let d = tempfile::tempdir().unwrap();
            let f = d.path().join("repli.mp4");
            let mut e = mp4::creer(&f, &params(600, 450, false)).unwrap();
            for n in 0..90 {
                e.image(&image_essai(600, 450, n), instant(n)).unwrap();
            }
            assert_eq!(e.codeur(), Codeur::Logiciel, "{cas:?}");
            e.finir().unwrap();
            PANNE.with(|p| p.set(None));
            let r = relire(&f, 600, 450);
            assert!(r.fautes.is_empty(), "{cas:?} {:#?}", r.fautes);
            assert!(
                proche(r.lecture.duree, Duration::from_secs(3), 100),
                "{cas:?}"
            );
            assert_eq!(r.numeros.first(), Some(&premier), "{cas:?}");
            assert_eq!(r.numeros.last(), Some(&89), "{cas:?}");
            assert!(
                r.instants
                    .first()
                    .is_some_and(|t| proche(*t, Duration::ZERO, 20)),
                "{cas:?} {:?}",
                r.instants.first()
            );
            assert!(r.numeros.windows(2).all(|w| w[0] < w[1]), "{cas:?}");
        }
        // Après 30 images écrites, un refus n'est plus rattrapé : l'erreur remonte.
        PANNE.with(|p| p.set(Some(Panne::Image(30))));
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("trop-tard.mp4");
        let mut e = mp4::creer(&f, &params(600, 450, false)).unwrap();
        let erreur = (0..40).find_map(|n| e.image(&image_essai(600, 450, n), instant(n)).err());
        PANNE.with(|p| p.set(None));
        assert!(erreur.is_some_and(|x| x.contains("WriteSample")));
        assert_eq!(e.codeur(), Codeur::Materiel);
        e.abandonner();
        assert!(std::fs::remove_file(&f).is_ok() || !f.exists());
    }

    /// Ce que l'écrivain refuse : une image d'une autre taille, un instant qui recule,
    /// `meme_image` sans image ; `finir` sans image rend une erreur et relâche le fichier.
    #[test]
    fn ecrivain_refuse_sans_paniquer() {
        if sans_codeur() {
            return;
        }
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("refus.mp4");
        let mut e = mp4::creer(&f, &params(600, 450, false)).unwrap();
        assert!(e.meme_image(instant(0)).is_err());
        assert!(e.image(&image_essai(602, 452, 0), instant(0)).is_err());
        e.image(&image_essai(600, 450, 0), instant(5)).unwrap();
        assert!(e.image(&image_essai(600, 450, 1), instant(5)).is_err());
        assert!(e.image(&image_essai(600, 450, 1), instant(4)).is_err());
        assert!(e.meme_image(instant(3)).is_err());
        assert!(
            e.son(&[0; 100], Duration::ZERO).is_ok(),
            "sans piste : rien"
        );
        e.image(&image_essai(600, 450, 1), instant(6)).unwrap();
        e.finir().unwrap();
        let g = d.path().join("vide.mp4");
        let e = mp4::creer(&g, &params(600, 450, false)).unwrap();
        assert!(e.finir().is_err());
        assert!(std::fs::remove_file(&g).is_ok() || !g.exists());
    }

    const TAUX: usize = TAUX_SON as usize;
    const SILENCE: (f64, f64) = (1.5, 2.5);

    fn sinus(debut: usize, n: usize) -> Vec<i16> {
        (debut..debut + n)
            .map(|i| {
                let t = i as f64 / TAUX as f64;
                if (SILENCE.0..SILENCE.1).contains(&t) {
                    0
                } else {
                    (0.5 * 32767.0 * (2.0 * std::f64::consts::PI * 440.0 * t).sin()) as i16
                }
            })
            .collect()
    }

    /// La piste son relue en PCM 16 bits : (premier horodatage, fin, échantillons).
    fn lire_son(chemin: &Path) -> (i64, i64, Vec<i16>) {
        let _com = Com::ouvrir();
        let _mf = Mf::demarrer(MFSTARTUP_FULL).unwrap();
        let fa = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
        let url = HSTRING::from(chemin.as_os_str());
        // SAFETY : objets COM créés ici ; tampons verrouillés le temps de la copie.
        unsafe {
            let r = MFCreateSourceReaderFromURL(&url, None::<&IMFAttributes>).unwrap();
            r.SetCurrentMediaType(fa, None, &type_son(false).unwrap())
                .unwrap();
            let _ = r.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false);
            r.SetStreamSelection(fa, true).unwrap();
            let mut pcm = Vec::new();
            let mut premier = None;
            let mut fin = 0i64;
            loop {
                let mut drapeaux = 0u32;
                let mut ts = 0i64;
                let mut s: Option<IMFSample> = None;
                r.ReadSample(
                    fa,
                    0,
                    None,
                    Some(&mut drapeaux),
                    Some(&mut ts),
                    Some(&mut s),
                )
                .unwrap();
                if drapeaux & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                    break;
                }
                let Some(s) = s else { continue };
                premier.get_or_insert(ts);
                fin = fin.max(ts + s.GetSampleDuration().unwrap_or(0));
                let b = s.ConvertToContiguousBuffer().unwrap();
                let mut p: *mut u8 = std::ptr::null_mut();
                let mut n = 0u32;
                b.Lock(&mut p, None, Some(&mut n)).unwrap();
                let octets = std::slice::from_raw_parts(p as *const u8, n as usize);
                pcm.extend(
                    octets
                        .chunks_exact(2)
                        .map(|c| i16::from_le_bytes([c[0], c[1]])),
                );
                let _ = b.Unlock();
            }
            (premier.unwrap_or(0), fin, pcm)
        }
    }

    /// Une vidéo avec sa piste son (une sinusoïde de 440 Hz et une seconde de silence),
    /// relue : la piste est là, sa fréquence, son silence et sa durée sont retrouvés.
    #[test]
    fn film_son_relu() {
        if sans_codeur() {
            return;
        }
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("son.mp4");
        let mut e = mp4::creer(&f, &params(600, 450, true)).unwrap();
        let par_image = TAUX / 30;
        for n in 0..120u32 {
            e.image(&image_essai(600, 450, n), instant(n)).unwrap();
            let debut = n as usize * par_image;
            e.son(&sinus(debut, par_image), duree_son(debut)).unwrap();
        }
        e.finir().unwrap();
        let r = relire(&f, 600, 450);
        assert!(r.lecture.son);
        assert!(r.fautes.is_empty(), "{:#?}", r.fautes);
        let (premier, fin, pcm) = lire_son(&f);
        let fin_video = 4.0;
        let fin_son = fin as f64 / 1e7;
        eprintln!(
            "son : premier {:.1} ms, fin {fin_son:.3} s, {} échantillons",
            premier as f64 / 1e4,
            pcm.len()
        );
        assert!((fin_son - fin_video).abs() < 0.1, "fin du son {fin_son}");
        let decalage = (premier.max(0) as f64 / 1e7 * TAUX as f64) as usize;
        let morceau = |a: f64, b: f64| -> &[i16] {
            let i = ((a * TAUX as f64) as usize)
                .saturating_sub(decalage)
                .min(pcm.len());
            let j = ((b * TAUX as f64) as usize)
                .saturating_sub(decalage)
                .min(pcm.len());
            &pcm[i..j]
        };
        let m = morceau(0.3, 1.2);
        let montants = m.windows(2).filter(|w| w[0] < 0 && w[1] >= 0).count();
        let frequence = montants as f64 / (m.len().max(1) as f64 / TAUX as f64);
        assert!((frequence - 440.0).abs() < 10.0, "{frequence} Hz");
        let fenetre = TAUX / 100;
        let (mut debut_s, mut fin_s) = (None, None);
        for (w, bloc) in pcm.chunks(fenetre).enumerate() {
            let efficace = (bloc.iter().map(|&x| f64::from(x).powi(2)).sum::<f64>()
                / bloc.len().max(1) as f64)
                .sqrt();
            let t = (w * fenetre) as f64 / TAUX as f64 + premier as f64 / 1e7;
            if efficace < 327.0 && t > 1.0 && t < 3.5 {
                debut_s.get_or_insert(t);
                fin_s = Some(t + 0.01);
            }
        }
        let (a, b) = (debut_s.unwrap(), fin_s.unwrap());
        assert!(
            (a - SILENCE.0).abs() < 0.1 && (b - SILENCE.1).abs() < 0.1,
            "silence {a} à {b}"
        );
    }

    fn charge(nom: &str) -> bool {
        // SAFETY : lecture de la liste des modules, rien n'est chargé.
        unsafe { GetModuleHandleW(&HSTRING::from(nom)) }.is_ok()
    }

    /// F31, la garde sur une DLL inventée : faux, sans plantage, et les suivantes ne sont
    /// pas essayées ; la variable d'essai la rend fausse sans rien charger. Ne demande aucun
    /// codeur : jamais sauté.
    #[test]
    fn garde_sur_une_dll_inventee() {
        assert!(!garde(None, &["bzs-absente.dll"]));
        assert!(!charge("bzs-absente.dll"));
        assert!(!garde(None, &["bzs-absente.dll", "bzs-absente-2.dll"]));
        assert!(!charge("bzs-absente-2.dll"));
        assert!(!garde(None, &["kernel32.dll", "bzs-absente.dll"]));
        assert!(garde(None, &["kernel32.dll"]));
        assert!(!garde(Some(OsStr::new("1")), &["kernel32.dll"]));
        assert!(garde(Some(OsStr::new("")), &["kernel32.dll"]));
        // Le dossier système seulement : un chemin vers ailleurs n'est pas cherché.
        assert!(!garde(None, &["bzs\\absente.dll"]));
    }

    fn est_enfant(nom: &str) -> bool {
        std::env::var(ENFANT).as_deref() == Ok(nom)
    }

    /// Relance l'exécutable des tests sur le seul test `nom`, dans un processus neuf.
    fn dans_un_enfant(nom: &str, variables: &[(&str, &str)]) {
        let exe = std::env::current_exe().unwrap();
        let test = format!("systeme::mp4::plateforme::tests::{nom}");
        let mut c = Command::new(exe);
        c.args([test.as_str(), "--exact", "--nocapture", "--test-threads=1"])
            .env(ENFANT, nom)
            .env_remove(VARIABLE_SANS_CODEUR);
        for (k, v) in variables {
            c.env(k, v);
        }
        let s = c.output().unwrap();
        let texte = format!(
            "{}{}",
            String::from_utf8_lossy(&s.stdout),
            String::from_utf8_lossy(&s.stderr)
        );
        eprintln!("{texte}");
        assert!(s.status.success(), "enfant {nom} : {:?}", s.status);
        assert!(
            texte.contains(&format!("{nom}: ok")),
            "enfant {nom} pas joué"
        );
    }

    /// F31 et F32 dans un processus neuf : aucune DLL de Media Foundation avant la garde ;
    /// après la recherche du codeur, ni le codeur H.264 de Windows ni aucun autre n'est
    /// entré dans le processus (rien d'activé).
    #[test]
    fn recherche_sans_activation() {
        dans_un_enfant("enfant_recherche", &[]);
    }

    #[test]
    fn enfant_recherche() {
        if !est_enfant("enfant_recherche") {
            return;
        }
        for dll in DLL {
            assert!(!charge(dll), "{dll} chargée avant la garde");
        }
        println!(
            "avant la garde : dxgi {}, d3d11 {}",
            charge("dxgi.dll"),
            charge("d3d11.dll")
        );
        assert!(bibliotheques());
        for dll in DLL {
            assert!(charge(dll), "{dll} absente après la garde");
        }
        let debut = Instant::now();
        let present = codeur_h264_present();
        println!("recherche : codeur {present} en {:?}", debut.elapsed());
        for codeur in ["mfh264enc.dll", "msmpeg2enc.dll"] {
            assert!(!charge(codeur), "{codeur} entré : un codeur a été activé");
        }
        if present {
            // Témoin : un vrai écrivain, lui, fait entrer le codeur.
            let d = tempfile::tempdir().unwrap();
            let e = mp4::creer(&d.path().join("t.mp4"), &params(600, 450, false)).unwrap();
            println!(
                "après un écrivain : mfh264enc {}, msmpeg2enc {}",
                charge("mfh264enc.dll"),
                charge("msmpeg2enc.dll")
            );
            e.abandonner();
        }
        println!("enfant_recherche: ok");
    }

    /// Cas J de l'essai 15 en Rust : avec `BREEZYSHOT_ESSAI_SANS_CODEUR`, rien n'est
    /// chargé ni appelé, et tout dit « absent » sans panique.
    #[test]
    fn sans_codeur_par_la_variable() {
        dans_un_enfant("enfant_sans_codeur", &[(VARIABLE_SANS_CODEUR, "1")]);
    }

    #[test]
    fn enfant_sans_codeur() {
        if !est_enfant("enfant_sans_codeur") {
            return;
        }
        assert!(!bibliotheques());
        assert!(!codeur_h264_present());
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("j.mp4");
        assert!(mp4::creer(&f, &params(600, 450, false)).is_err());
        assert!(mp4::lire(&f, |_, _, _| {}).is_err());
        for dll in DLL {
            assert!(!charge(dll), "{dll} chargée malgré la variable");
        }
        println!("enfant_sans_codeur: ok");
    }

    /// `abandonner` et `finir` sans fuite, dans un processus neuf : le fichier s'efface
    /// tout de suite après, et les poignées du processus ne montent pas.
    #[test]
    fn fermer_sans_fuite() {
        dans_un_enfant("enfant_fermeture", &[]);
    }

    fn poignees() -> u32 {
        let mut n = 0u32;
        // SAFETY : sortie valide, pseudo-poignée du processus.
        unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut n) }.unwrap();
        n
    }

    /// Efface `f` ; il doit l'être en 1 s au plus. Rend le temps pris.
    fn effacer(f: &Path) -> Duration {
        let debut = Instant::now();
        loop {
            match std::fs::remove_file(f) {
                Ok(()) => return debut.elapsed(),
                Err(e) => {
                    assert!(
                        debut.elapsed() < Duration::from_secs(1),
                        "fichier tenu : {e}"
                    );
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }

    #[test]
    fn enfant_fermeture() {
        if !est_enfant("enfant_fermeture") {
            return;
        }
        if !codeur_h264_present() {
            println!("saute : aucun codeur H.264 sur cette machine");
            println!("enfant_fermeture: ok");
            return;
        }
        // Media Foundation gardé ouvert : ses fils communs ne comptent pas dans l'écart.
        let _com = Com::ouvrir();
        let _mf = Mf::demarrer(MFSTARTUP_FULL).unwrap();
        let d = tempfile::tempdir().unwrap();
        let mut pire = Duration::ZERO;
        let mut cycle = |i: u32| {
            for finir in [false, true] {
                let f = d.path().join(format!("f{i}-{finir}.mp4"));
                let mut e = mp4::creer(&f, &params(600, 450, false)).unwrap();
                for n in 0..10 {
                    e.image(&image_essai(600, 450, n), instant(n)).unwrap();
                }
                if finir {
                    e.finir().unwrap();
                } else {
                    e.abandonner();
                }
                pire = pire.max(effacer(&f));
            }
        };
        for i in 0..3 {
            cycle(i);
        }
        std::thread::sleep(Duration::from_millis(300));
        let avant = poignees();
        for i in 3..13 {
            cycle(i);
        }
        std::thread::sleep(Duration::from_millis(500));
        let apres = poignees();
        println!("poignées {avant} puis {apres} après dix films ; effacé en {pire:?} au pire");
        assert!(apres <= avant + 5, "poignées {avant} puis {apres}");
        println!("enfant_fermeture: ok");
    }
}
