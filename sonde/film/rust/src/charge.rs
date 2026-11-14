//! S2 points 8 et 12 : chargement à la demande des DLL de Media Foundation, de DXGI et de
//! Direct3D, garde par `LoadLibraryExW`, bibliothèque absente, modules au repos. Le même
//! code sert à l'exemple `charge` (lié avec /DELAYLOAD, et une DLL inventée
//! `bzs-absente.dll`) et au binaire `charge-temoin` (lié sans /DELAYLOAD, sans la DLL
//! inventée), le témoin de la mesure. Windows seulement.
//!
//!   charge --etat E --signal S            les phases ci-dessous
//!   charge --absente garde|sans-garde     8c (exemple `charge` seulement)
//!
//! 1. « avant » : au lancement, la liste des modules (EnumProcessModules) et la mémoire
//!    privée sont écrites dans E ; attente de « go » dans S (60 s au plus) : la sonde
//!    regarde alors `Get-Process -Module`.
//! 2. « recherche » : la garde de F31 (`LoadLibraryExW(nom, LOAD_LIBRARY_SEARCH_SYSTEM32)`
//!    pour mfplat, mfreadwrite et mf), puis la recherche du codeur comme F32 (`MFStartup`,
//!    `MFTEnumEx` sans `ActivateObject`, `MFShutdown`) ; modules et mémoire ; attente de
//!    « go ».
//! 3. « apres » : `MFStartup`, `MFCreateSinkWriterFromURL`, `CreateDXGIFactory1`,
//!    `D3D11CreateDevice`, `MFShutdown` ; modules ; attente de « quitter ».
//!
//! Toute erreur est écrite (« erreur … » avec le HRESULT), jamais tue.

use crate::{ecrire_etat, hresult, ressources};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{FreeLibrary, GetLastError, HMODULE};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, COINIT_MULTITHREADED};
use windows::Win32::System::LibraryLoader::{LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32};
use windows::Win32::System::ProcessStatus::{EnumProcessModules, GetModuleBaseNameW};
use windows::Win32::System::Threading::GetCurrentProcess;

/// DLL suivies (S2 points 8 et 12).
pub const SUIVIES: [&str; 8] = [
    "mfplat.dll",
    "mfreadwrite.dll",
    "mf.dll",
    "dxgi.dll",
    "d3d11.dll",
    "mmdevapi.dll",
    "audioses.dll",
    "msmpeg2enc.dll",
];

fn hr(e: &windows::core::Error) -> String {
    hresult(e.code().0, &e.message())
}

/// Noms (en minuscules) des modules chargés dans le processus.
pub fn modules() -> Vec<String> {
    let mut h = vec![HMODULE::default(); 2048];
    let mut besoin = 0u32;
    // SAFETY : tableau de la taille annoncée ; pseudo-handle du processus courant.
    unsafe {
        let p = GetCurrentProcess();
        if EnumProcessModules(
            p,
            h.as_mut_ptr(),
            (h.len() * std::mem::size_of::<HMODULE>()) as u32,
            &mut besoin,
        )
        .is_err()
        {
            return vec!["(EnumProcessModules a échoué)".into()];
        }
        let n = (besoin as usize / std::mem::size_of::<HMODULE>()).min(h.len());
        let mut v: Vec<String> = h[..n]
            .iter()
            .map(|m| {
                let mut nom = [0u16; 260];
                let l = GetModuleBaseNameW(p, Some(*m), &mut nom) as usize;
                String::from_utf16_lossy(&nom[..l]).to_lowercase()
            })
            .collect();
        v.sort();
        v
    }
}

fn suivies(m: &[String]) -> String {
    let v: Vec<&str> = SUIVIES
        .iter()
        .copied()
        .filter(|s| m.iter().any(|x| x == s))
        .collect();
    if v.is_empty() {
        "aucune".into()
    } else {
        v.join(",")
    }
}

fn attendre(signal: &Path, mot: &str) -> bool {
    let limite = Instant::now() + Duration::from_secs(60);
    while Instant::now() < limite {
        if std::fs::read_to_string(signal).is_ok_and(|t| t.trim() == mot) {
            let _ = std::fs::remove_file(signal);
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

/// La garde de F31 : `LoadLibraryExW(nom, LOAD_LIBRARY_SEARCH_SYSTEM32)`.
pub fn garde(nom: &str) -> Result<HMODULE, String> {
    // SAFETY : nom terminé par un zéro (HSTRING) ; aucun fichier passé.
    unsafe {
        LoadLibraryExW(
            PCWSTR(HSTRING::from(nom).as_ptr()),
            None,
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        )
    }
    .map_err(|e| format!("{} (GetLastError {})", hr(&e), unsafe { GetLastError().0 }))
}

/// MFStartup, MFTEnumEx des codeurs H.264 (sans activer), le nombre trouvé.
fn chercher_codeur(l: &mut Vec<String>, cle: &str) {
    // SAFETY : COM du fil puis Media Foundation ; tableau de MFTEnumEx libéré sans activer.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        match MFStartup(MF_VERSION, MFSTARTUP_FULL) {
            Ok(()) => l.push(format!("{cle}_mfstartup 1")),
            Err(e) => l.push(format!("erreur {cle} MFStartup: {}", hr(&e))),
        }
        let sortie = MFT_REGISTER_TYPE_INFO {
            guidMajorType: MFMediaType_Video,
            guidSubtype: MFVideoFormat_H264,
        };
        let mut p: *mut Option<IMFActivate> = std::ptr::null_mut();
        let mut n = 0u32;
        match MFTEnumEx(
            MFT_CATEGORY_VIDEO_ENCODER,
            MFT_ENUM_FLAG_SYNCMFT
                | MFT_ENUM_FLAG_ASYNCMFT
                | MFT_ENUM_FLAG_HARDWARE
                | MFT_ENUM_FLAG_SORTANDFILTER,
            None,
            Some(&sortie),
            &mut p,
            &mut n,
        ) {
            Ok(()) => {
                l.push(format!("{cle}_codeurs_h264 {n}"));
                if !p.is_null() {
                    for x in std::slice::from_raw_parts_mut(p, n as usize) {
                        drop(x.take());
                    }
                    CoTaskMemFree(Some(p as *const core::ffi::c_void));
                }
            }
            Err(e) => l.push(format!("erreur {cle} MFTEnumEx: {}", hr(&e))),
        }
    }
}

fn phase_recherche(l: &mut Vec<String>) {
    let t = Instant::now();
    for nom in ["mfplat.dll", "mfreadwrite.dll", "mf.dll"] {
        match garde(nom) {
            // Gardée chargée, comme le ferait l'app jusqu'à sa sortie.
            Ok(_) => l.push(format!("garde_{nom} 1")),
            Err(e) => l.push(format!("garde_{nom} 0 ; {e}")),
        }
    }
    chercher_codeur(l, "recherche");
    // SAFETY : rend ce que MFStartup a pris.
    let _ = unsafe { MFShutdown() };
    l.push(format!(
        "recherche_ms {:.1}",
        t.elapsed().as_secs_f64() * 1000.0
    ));
}

fn phase_apres(l: &mut Vec<String>) {
    let t = Instant::now();
    chercher_codeur(l, "apres");
    let f: PathBuf = std::env::temp_dir().join(format!("charge-{}.mp4", std::process::id()));
    // SAFETY : chemin terminé par un zéro ; objets COM comptés.
    unsafe {
        match MFCreateSinkWriterFromURL(
            &HSTRING::from(f.as_os_str()),
            None::<&IMFByteStream>,
            None::<&IMFAttributes>,
        ) {
            Ok(w) => {
                l.push("sinkwriter 1".into());
                drop(w);
            }
            Err(e) => l.push(format!("erreur MFCreateSinkWriterFromURL: {}", hr(&e))),
        }
        let _ = std::fs::remove_file(&f);
        match CreateDXGIFactory1::<IDXGIFactory1>() {
            Ok(_) => l.push("dxgi 1".into()),
            Err(e) => l.push(format!("erreur CreateDXGIFactory1: {}", hr(&e))),
        }
        let mut d = None;
        let mut r = Err(windows::core::Error::empty());
        for genre in [D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP] {
            r = D3D11CreateDevice(
                None,
                genre,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d),
                None,
                None,
            );
            if r.is_ok() {
                l.push(format!("d3d11 1 (type {})", genre.0));
                break;
            }
        }
        if let Err(e) = r {
            l.push(format!("erreur D3D11CreateDevice: {}", hr(&e)));
        }
        drop(d);
        let _ = MFShutdown();
    }
    l.push(format!(
        "appels_ms {:.1}",
        t.elapsed().as_secs_f64() * 1000.0
    ));
}

fn noter_modules(l: &mut Vec<String>, phase: &str) {
    let m = modules();
    l.push(format!("suivies_{phase} {}", suivies(&m)));
    l.push(format!("nombre_modules_{phase} {}", m.len()));
    l.push(format!("modules_{phase} {}", m.join(",")));
    l.push(format!("prive_{phase} {}", ressources().0));
}

/// `absente` : l'appel de la fonction de `bzs-absente.dll` (exemple `charge`) ; `None`
/// pour le témoin lié sans /DELAYLOAD.
pub fn principal(a: &HashMap<String, String>, absente: Option<fn() -> u32>) {
    if let Some(m) = a.get("absente") {
        let Some(appel) = absente else {
            println!("absente non liée (binaire témoin)");
            println!("ok 0");
            return;
        };
        match m.as_str() {
            "garde" => match garde("bzs-absente.dll") {
                Ok(h) => {
                    println!("absente_garde chargée (inattendu)");
                    println!("absente_appel {}", appel());
                    // SAFETY : module chargé juste avant.
                    let _ = unsafe { FreeLibrary(h) };
                    println!("ok 1");
                }
                Err(e) => {
                    println!("absente_garde refusée ; {e}");
                    println!("absente_appel non fait");
                    println!("ok 1");
                }
            },
            _ => {
                println!("absente_appel sans garde : appel de BzsAbsente");
                use std::io::Write;
                let _ = std::io::stdout().flush();
                let r = appel();
                println!("absente_rendu {r} (inattendu : la DLL n'existe pas)");
                println!("ok 1");
            }
        }
        return;
    }
    let (Some(etat), Some(signal)) = (a.get("etat"), a.get("signal")) else {
        println!("ok 0");
        println!("erreur --etat et --signal sont demandés");
        return;
    };
    let (etat, signal) = (Path::new(etat), Path::new(signal));
    let mut l = vec![
        format!("pid {}", std::process::id()),
        format!("temoin {}", absente.is_none() as u8),
    ];
    noter_modules(&mut l, "avant");
    l.push("etape avant".into());
    ecrire_etat(etat, &l);
    if !attendre(signal, "go") {
        l.push("erreur pas de « go » en 60 s".into());
        ecrire_etat(etat, &l);
        return;
    }
    l.retain(|x| !x.starts_with("etape "));
    phase_recherche(&mut l);
    noter_modules(&mut l, "recherche");
    l.push("etape recherche".into());
    ecrire_etat(etat, &l);
    if !attendre(signal, "go") {
        l.push("erreur pas de second « go » en 60 s".into());
        ecrire_etat(etat, &l);
        return;
    }
    l.retain(|x| !x.starts_with("etape "));
    phase_apres(&mut l);
    noter_modules(&mut l, "apres");
    l.push("etape apres".into());
    ecrire_etat(etat, &l);
    let _ = attendre(signal, "quitter");
}
