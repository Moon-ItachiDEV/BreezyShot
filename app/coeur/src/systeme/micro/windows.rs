//! WASAPI sous Windows (chantier C, F17, F18) : le point de capture par défaut du rôle
//! `eConsole`, en mode partagé, ouvert dans le fil `film-son`.
//!
//! Windows convertit lui-même en PCM 48 kHz mono 16 bits (`AUTOCONVERTPCM` et
//! `SRC_DEFAULT_QUALITY`). S'il refuse ce format (Windows ancien, pilote rétif), le flux
//! est ouvert dans le format de mélange de l'appareil et converti ici (`Conversion`, pure).
//!
//! Le fil relève le tampon de l'appareil toutes les 10 ms (tampon de 200 ms : une relève
//! en retard ne perd rien) et donne au rappel ce qu'il a lu, hors du tampon de l'appareil.
//! Un appareil débranché ou repris par Windows arrête la capture : `perdu()` devient vrai,
//! et une ligne `film: micro (<code>)` est écrite.
//!
//! COM : initialisé dans le fil `film-son` (MTA) et rendu à sa fin. `present()` sert depuis
//! n'importe quel fil : si COM y est déjà initialisé autrement (le fil principal de l'app,
//! STA), il s'en sert sans rien changer.
//!
//! Aucune DLL n'est chargée avant le premier appel : WASAPI passe par COM
//! (`MMDeviceEnumerator`), qui charge `mmdevapi.dll` à la demande.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use ::windows::Win32::Foundation::E_INVALIDARG;
use ::windows::Win32::Media::Audio::{
    eCapture, eConsole, IAudioCaptureClient, IAudioClient, IMMDevice, IMMDeviceEnumerator,
    MMDeviceEnumerator, AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_E_UNSUPPORTED_FORMAT,
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
    AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, DEVICE_STATE_ACTIVE, WAVEFORMATEX, WAVE_FORMAT_PCM,
};
use ::windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_MULTITHREADED,
};

use super::{absence_ordinaire, code_hex, erreur_de, lire_format, Conversion, Erreur, FREQUENCE};

/// Le rappel du son, du fil `film-son`.
pub type Rappel = Box<dyn FnMut(&[i16]) + Send + 'static>;

/// Tampon partagé demandé à l'appareil (unités de 100 ns) : 200 ms.
const DUREE_TAMPON: i64 = 2_000_000;
/// Période de relève du tampon.
const RELEVE: Duration = Duration::from_millis(10);
/// Au plus ce temps pour ouvrir l'appareil ; au-delà, `ouvrir` rend une erreur (le fil
/// s'arrête de lui-même dès qu'il a fini).
const DELAI_OUVERTURE: Duration = Duration::from_secs(3);

/// COM initialisé pour ce fil (MTA), rendu à la destruction si c'est cet appel qui l'a
/// initialisé. Déjà initialisé autrement (STA) : utilisable tel quel, rien à rendre.
struct Com {
    a_rendre: bool,
}

impl Com {
    fn initialiser() -> Com {
        // SAFETY : sans réserve ; S_OK et S_FALSE se rendent par `CoUninitialize`,
        // RPC_E_CHANGED_MODE (COM déjà en STA dans ce fil) ne se rend pas.
        let resultat = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Com {
            a_rendre: resultat.is_ok(),
        }
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.a_rendre {
            // SAFETY : rend l'initialisation faite par `initialiser` dans ce même fil ; les
            // objets COM de ce fil ont été rendus avant (déclarés après ce gardien).
            unsafe { CoUninitialize() };
        }
    }
}

/// Une mémoire allouée par COM (`GetMixFormat`), rendue par `CoTaskMemFree`.
struct MemoireCom(*mut WAVEFORMATEX);

impl Drop for MemoireCom {
    fn drop(&mut self) {
        // SAFETY : pointeur rendu par COM, rendu une seule fois.
        unsafe { CoTaskMemFree(Some(self.0 as *const core::ffi::c_void)) };
    }
}

/// Le point de capture par défaut du rôle `eConsole` (« Son › Entrée »).
fn point_par_defaut() -> ::windows::core::Result<IMMDevice> {
    // SAFETY : COM initialisé dans ce fil par l'appelant ; objets rendus par leur compte
    // de références.
    unsafe {
        let enumerateur: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        enumerateur.GetDefaultAudioEndpoint(eCapture, eConsole)
    }
}

pub fn present() -> bool {
    let _com = Com::initialiser();
    let Ok(appareil) = point_par_defaut() else {
        return false;
    };
    // SAFETY : appareil vivant ; lecture de son état.
    unsafe { appareil.GetState() }.is_ok_and(|etat| etat == DEVICE_STATE_ACTIVE)
}

/// Le sens d'un échec à l'ouverture, écrit au journal sauf l'absence ordinaire.
fn echec(e: &::windows::core::Error) -> Erreur {
    let code = e.code().0;
    if !absence_ordinaire(code) {
        log::warn!("film: micro ({})", code_hex(code));
    }
    erreur_de(code)
}

/// Le flux ouvert et démarré.
struct Flux {
    capture: IAudioCaptureClient,
    client: IAudioClient,
    /// `None` : Windows convertit, les trames sont déjà en 48 kHz mono 16 bits.
    conversion: Option<Conversion>,
    /// Octets d'une trame dans le tampon de l'appareil.
    bloc: usize,
}

/// Active un client audio sur le point de capture par défaut.
fn activer() -> Result<IAudioClient, Erreur> {
    let appareil = point_par_defaut().map_err(|e| echec(&e))?;
    // SAFETY : appareil vivant ; le client est rendu par son compte de références.
    unsafe { appareil.Activate(CLSCTX_ALL, None) }.map_err(|e| echec(&e))
}

/// Ouvre et démarre le flux : en 48 kHz mono 16 bits converti par Windows, ou, s'il refuse,
/// dans le format de mélange de l'appareil, converti ici.
fn ouvrir_flux() -> Result<Flux, Erreur> {
    let client = activer()?;
    let voulu = WAVEFORMATEX {
        wFormatTag: WAVE_FORMAT_PCM as u16,
        nChannels: 1,
        nSamplesPerSec: FREQUENCE,
        nAvgBytesPerSec: FREQUENCE * 2,
        nBlockAlign: 2,
        wBitsPerSample: 16,
        cbSize: 0,
    };
    // SAFETY : format décrit en entier ; mode partagé ; sans session nommée.
    let direct = unsafe {
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
            DUREE_TAMPON,
            0,
            &voulu,
            None,
        )
    };
    let (client, conversion, bloc) = match direct {
        Ok(()) => (client, None, 2),
        Err(e) if e.code() == AUDCLNT_E_UNSUPPORTED_FORMAT || e.code() == E_INVALIDARG => {
            // Un client dont `Initialize` a échoué ne se réessaie pas : un neuf.
            drop(client);
            let client = activer()?;
            // SAFETY : client vivant ; le format est alloué par COM et rendu par
            // `MemoireCom` ; ses octets sont lus sur la taille qu'il annonce (18 + cbSize).
            let format = unsafe {
                let melange = MemoireCom(client.GetMixFormat().map_err(|e| echec(&e))?);
                if melange.0.is_null() {
                    return Err(Erreur::Autre("format de melange absent".into()));
                }
                let entete = std::slice::from_raw_parts(melange.0 as *const u8, 18);
                let suite = u16::from_le_bytes([entete[16], entete[17]]) as usize;
                let octets = std::slice::from_raw_parts(melange.0 as *const u8, 18 + suite);
                let format = lire_format(octets).map_err(|e| {
                    log::warn!("film: micro (format: {e})");
                    Erreur::Autre(format!("format: {e}"))
                })?;
                client
                    .Initialize(
                        AUDCLNT_SHAREMODE_SHARED,
                        0,
                        DUREE_TAMPON,
                        0,
                        melange.0,
                        None,
                    )
                    .map_err(|e| echec(&e))?;
                format
            };
            (client, Some(Conversion::nouvelle(format)), format.bloc)
        }
        Err(e) => return Err(echec(&e)),
    };
    // SAFETY : client initialisé ; le service de capture est rendu par son compte.
    let capture: IAudioCaptureClient = unsafe { client.GetService() }.map_err(|e| echec(&e))?;
    // SAFETY : client initialisé.
    unsafe { client.Start() }.map_err(|e| echec(&e))?;
    Ok(Flux {
        capture,
        client,
        conversion,
        bloc,
    })
}

/// Relève le tampon de l'appareil jusqu'à l'arrêt ; rend le code d'un échec (appareil
/// perdu).
fn relever(flux: &mut Flux, rappel: &mut Rappel, arret: &AtomicBool) -> Result<(), String> {
    let code = |e: ::windows::core::Error| code_hex(e.code().0);
    let mut pcm: Vec<i16> = Vec::with_capacity(FREQUENCE as usize / 10);
    while !arret.load(Ordering::SeqCst) {
        std::thread::sleep(RELEVE);
        loop {
            // SAFETY : flux démarré, dans le fil qui l'a ouvert.
            let paquet = unsafe { flux.capture.GetNextPacketSize() }.map_err(code)?;
            if paquet == 0 {
                break;
            }
            let mut donnees: *mut u8 = std::ptr::null_mut();
            let mut trames = 0u32;
            let mut drapeaux = 0u32;
            // SAFETY : sorties valides ; le tampon est rendu par `ReleaseBuffer` plus bas.
            unsafe {
                flux.capture
                    .GetBuffer(&mut donnees, &mut trames, &mut drapeaux, None, None)
            }
            .map_err(code)?;
            if trames == 0 {
                break;
            }
            let n = trames as usize;
            let silence = drapeaux & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || donnees.is_null();
            let octets: &[u8] = if silence {
                &[]
            } else {
                // SAFETY : `GetBuffer` rend `trames` trames de `bloc` octets, valides
                // jusqu'à `ReleaseBuffer`.
                unsafe { std::slice::from_raw_parts(donnees, n * flux.bloc) }
            };
            match &mut flux.conversion {
                Some(conversion) => conversion.convertir(octets, n, silence, &mut pcm),
                None if silence => pcm.resize(pcm.len() + n, 0),
                None => pcm.extend(
                    octets
                        .chunks_exact(2)
                        .map(|b| i16::from_le_bytes([b[0], b[1]])),
                ),
            }
            // SAFETY : rend les trames lues par `GetBuffer`.
            unsafe { flux.capture.ReleaseBuffer(trames) }.map_err(code)?;
            if !pcm.is_empty() {
                rappel(&pcm);
                pcm.clear();
            }
        }
    }
    Ok(())
}

/// Le fil `film-son` : ouvre, dit le résultat sur `pret`, relève jusqu'à l'arrêt.
fn capturer(
    mut rappel: Rappel,
    pret: mpsc::SyncSender<Result<(), Erreur>>,
    arret: Arc<AtomicBool>,
    perdu: Arc<AtomicBool>,
) {
    let _com = Com::initialiser();
    let mut flux = match ouvrir_flux() {
        Ok(flux) => flux,
        Err(e) => {
            let _ = pret.send(Err(e));
            return;
        }
    };
    // Personne n'attend plus (ouverture trop longue) : on s'arrête.
    if pret.send(Ok(())).is_err() {
        // SAFETY : flux démarré par ce fil.
        let _ = unsafe { flux.client.Stop() };
        return;
    }
    let releve = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        relever(&mut flux, &mut rappel, &arret)
    }));
    match releve {
        Ok(Ok(())) => {}
        Ok(Err(code)) => {
            perdu.store(true, Ordering::SeqCst);
            log::warn!("film: micro ({code})");
        }
        Err(_) => {
            perdu.store(true, Ordering::SeqCst);
            log::warn!("film: micro (panique)");
        }
    }
    // SAFETY : flux démarré par ce fil ; sans effet s'il est déjà arrêté.
    let _ = unsafe { flux.client.Stop() };
}

/// Le micro ouvert : le fil `film-son` et ses drapeaux.
pub struct Micro {
    arret: Arc<AtomicBool>,
    perdu: Arc<AtomicBool>,
    fil: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for Micro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Micro")
            .field("perdu", &perdu(self))
            .finish()
    }
}

impl Drop for Micro {
    fn drop(&mut self) {
        self.arret.store(true, Ordering::SeqCst);
        if let Some(fil) = self.fil.take() {
            // Le fil voit l'arrêt à sa prochaine relève (10 ms au plus, plus le rappel).
            let _ = fil.join();
        }
    }
}

pub fn ouvrir(rappel: Rappel) -> Result<Micro, Erreur> {
    let arret = Arc::new(AtomicBool::new(false));
    let perdu = Arc::new(AtomicBool::new(false));
    let (pret, reponse) = mpsc::sync_channel(1);
    let (a, p) = (Arc::clone(&arret), Arc::clone(&perdu));
    let fil = std::thread::Builder::new()
        .name("film-son".into())
        .spawn(move || capturer(rappel, pret, a, p))
        .map_err(|e| Erreur::Autre(format!("fil: {e}")))?;
    match reponse.recv_timeout(DELAI_OUVERTURE) {
        Ok(Ok(())) => Ok(Micro {
            arret,
            perdu,
            fil: Some(fil),
        }),
        Ok(Err(e)) => {
            let _ = fil.join();
            Err(e)
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // Le fil n'est pas attendu : il s'arrête de lui-même quand l'ouverture finit.
            arret.store(true, Ordering::SeqCst);
            log::warn!("film: micro (ouverture trop longue)");
            Err(Erreur::Autre("ouverture trop longue".into()))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = fil.join();
            log::warn!("film: micro (fil arrete)");
            Err(Erreur::Autre("fil arrete".into()))
        }
    }
}

pub fn perdu(m: &Micro) -> bool {
    // Le fil ne s'arrête de lui-même que sur un échec (ou une panique) : un fil fini
    // pendant que le micro est ouvert, c'est un micro perdu.
    m.perdu.load(Ordering::SeqCst) || m.fil.as_ref().is_some_and(|f| f.is_finished())
}

#[cfg(test)]
mod tests {
    //! Tests Windows. La machine GitHub n'a aucun micro (S3) : on y prouve que rien ne
    //! panique et que l'absence est dite ; le rythme du PCM ne se joue qu'avec un micro.

    use super::*;
    use crate::systeme::micro as facade;
    use std::sync::Mutex;
    use std::time::Instant;

    use ::windows::Win32::System::Com::COINIT_APARTMENTTHREADED;

    /// `present` ne panique pas, depuis un fil sans COM et depuis un fil déjà en STA (le
    /// fil principal de l'app), et répond pareil ; durée notée.
    #[test]
    fn present_sans_panique() {
        let debut = Instant::now();
        let ici = facade::present();
        let duree = debut.elapsed();
        let sta = std::thread::spawn(|| {
            // SAFETY : COM en STA pour ce fil, rendu à la fin.
            let r = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            let present = facade::present();
            if r.is_ok() {
                // SAFETY : rend l'initialisation de ce fil.
                unsafe { CoUninitialize() };
            }
            present
        })
        .join()
        .unwrap();
        eprintln!(
            "micro: present {ici} en {:.1} ms ; depuis un fil STA : {sta}",
            duree.as_secs_f64() * 1000.0
        );
        assert_eq!(ici, sta);
        // Deux fois de suite : même réponse (rien n'est gardé ouvert).
        assert_eq!(facade::present(), ici);
    }

    /// Sans micro : `ouvrir` rend `Absent`, sans fil laissé ni rappel appelé.
    #[test]
    fn ouvrir_sans_micro_rend_absent() {
        if facade::present() {
            eprintln!("micro: un micro est la : cas sans micro non joue");
            return;
        }
        let appele = Arc::new(AtomicBool::new(false));
        let a = Arc::clone(&appele);
        let e = facade::ouvrir(move |_| a.store(true, Ordering::SeqCst)).unwrap_err();
        assert_eq!(e, Erreur::Absent);
        assert!(!appele.load(Ordering::SeqCst));
    }

    /// Avec un micro (jamais sur GitHub) : le rappel reçoit du PCM au rythme de 48 kHz,
    /// depuis le fil `film-son` ; la destruction arrête le fil et rend l'appareil.
    #[test]
    fn avec_micro_pcm_au_bon_rythme() {
        if !facade::present() {
            eprintln!("micro: aucun micro : rythme non joue");
            return;
        }
        let recus = Arc::new(Mutex::new((0usize, 0usize, Vec::<String>::new())));
        let r = Arc::clone(&recus);
        let debut = Instant::now();
        let micro = match facade::ouvrir(move |pcm| {
            let mut g = r.lock().unwrap_or_else(|e| e.into_inner());
            g.0 += pcm.len();
            g.1 += 1;
            let nom = std::thread::current().name().unwrap_or("?").to_string();
            if !g.2.contains(&nom) {
                g.2.push(nom);
            }
        }) {
            Ok(m) => m,
            Err(Erreur::Refuse) => {
                eprintln!("micro: refuse par Windows (confidentialite ou exclusif) : non joue");
                return;
            }
            Err(e) => panic!("ouvrir : {e:?}"),
        };
        let ouverture = debut.elapsed();
        std::thread::sleep(Duration::from_millis(200));
        let depart = {
            let g = recus.lock().unwrap();
            g.0
        };
        let t0 = Instant::now();
        std::thread::sleep(Duration::from_secs(2));
        let (total, appels, fils) = {
            let g = recus.lock().unwrap();
            (g.0, g.1, g.2.clone())
        };
        let duree = t0.elapsed().as_secs_f64();
        let rythme = (total - depart) as f64 / duree;
        eprintln!(
            "micro: ouvert en {:.0} ms ; {rythme:.0} echantillons par seconde, {appels} \
             rappels, fils {fils:?}, perdu {}",
            ouverture.as_secs_f64() * 1000.0,
            micro.perdu()
        );
        assert!(!micro.perdu());
        assert_eq!(fils, vec!["film-son".to_string()]);
        assert!(
            (0.8 * 48_000.0..=1.2 * 48_000.0).contains(&rythme),
            "rythme {rythme:.0} par seconde"
        );
        let arret = Instant::now();
        drop(micro);
        let apres = recus.lock().unwrap().1;
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(
            recus.lock().unwrap().1,
            apres,
            "rappel apres la destruction"
        );
        eprintln!(
            "micro: ferme en {:.1} ms",
            arret.elapsed().as_secs_f64() * 1000.0
        );
    }
}
