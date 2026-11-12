//! Gestes de l'installation propres à Windows. Seule écriture du registre de
//! l'installateur : la clé de désinstallation de l'utilisateur (HKCU), voir `mod.rs`.

use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use ::windows::core::{Interface, HSTRING, PCWSTR, PWSTR};
use ::windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, HANDLE,
    HWND, POINT, RECT,
};
use ::windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use ::windows::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetVolumePathNameW, MoveFileExW, MOVEFILE_REPLACE_EXISTING,
    MOVEFILE_WRITE_THROUGH,
};
use ::windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, IPersistFile,
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, STGM_READ,
};
use ::windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteKeyW, RegDeleteValueW, RegGetValueW, RegOpenKeyExW,
    RegQueryInfoKeyW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE,
    KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ, RRF_RT_REG_SZ,
};
use ::windows::Win32::System::Threading::{CreateMutexW, OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE};
use ::windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use ::windows::Win32::UI::Shell::{
    FOLDERID_LocalAppData, FOLDERID_Programs, FOLDERID_UserProgramFiles, IShellLinkW,
    SHGetKnownFolderPath, ShellLink, KF_FLAG_DONT_VERIFY,
};
use ::windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, FindWindowExW, FindWindowW, GetCursorPos, GetWindowRect, IsIconic,
    IsWindowVisible, MessageBoxW, SetForegroundWindow, SetWindowPos, ShowWindow, ASFW_ANY,
    MB_ICONERROR, MB_OK, MB_SETFOREGROUND, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SW_RESTORE,
};
use webview2_com::Microsoft::Web::WebView2::Win32::GetAvailableCoreWebView2BrowserVersionString;

use super::{DossiersConnus, Valeur};
use crate::systeme::Rect;

/// Un mutex nommé, fermé à la destruction.
pub struct Poignee(HANDLE);

impl Drop for Poignee {
    fn drop(&mut self) {
        // SAFETY : poignée rendue par CreateMutexW, fermée une seule fois.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

// SAFETY : une poignée de mutex peut être fermée depuis n'importe quel fil.
unsafe impl Send for Poignee {}
unsafe impl Sync for Poignee {}

fn large(texte: &str) -> Vec<u16> {
    texte.encode_utf16().chain(std::iter::once(0)).collect()
}

fn large_chemin(chemin: &Path) -> Vec<u16> {
    chemin
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Partage refusé, verrou, accès refusé (un exe en cours ne se remplace pas) : à retenter.
pub fn est_partage_refuse(e: &io::Error) -> bool {
    matches!(e.raw_os_error(), Some(5) | Some(32) | Some(33))
}

fn dossier_connu(id: &::windows::core::GUID) -> Option<PathBuf> {
    // SAFETY : la chaîne rendue est libérée par CoTaskMemFree après copie.
    unsafe {
        let p = SHGetKnownFolderPath(id, KF_FLAG_DONT_VERIFY, None).ok()?;
        let texte = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const core::ffi::c_void));
        texte.filter(|t| !t.is_empty()).map(PathBuf::from)
    }
}

pub fn dossiers_connus() -> DossiersConnus {
    let local = dossier_connu(&FOLDERID_LocalAppData);
    DossiersConnus {
        programmes: dossier_connu(&FOLDERID_UserProgramFiles)
            .or_else(|| local.as_ref().map(|l| l.join("Programs"))),
        menu: dossier_connu(&FOLDERID_Programs),
        local,
        temporaire: std::env::temp_dir(),
    }
}

pub fn espace_libre(dossier: &Path) -> Option<u64> {
    let chemin = large_chemin(dossier);
    let mut libre = 0u64;
    // SAFETY : chaîne terminée par zéro, sortie dans une variable locale.
    unsafe { GetDiskFreeSpaceExW(PCWSTR(chemin.as_ptr()), Some(&mut libre), None, None) }
        .ok()
        .map(|()| libre)
}

pub fn lettre_volume(dossier: &Path) -> Option<String> {
    let existant = dossier.ancestors().find(|d| d.exists())?;
    let chemin = large_chemin(existant);
    let mut tampon = [0u16; 512];
    // SAFETY : tampon local dont la taille est passée par la tranche.
    unsafe { GetVolumePathNameW(PCWSTR(chemin.as_ptr()), &mut tampon) }.ok()?;
    let fin = tampon.iter().position(|c| *c == 0).unwrap_or(tampon.len());
    let volume = String::from_utf16_lossy(&tampon[..fin]);
    let mut c = volume.chars();
    match (c.next(), c.next()) {
        (Some(l), Some(':')) if l.is_ascii_alphabetic() => {
            Some(format!("{}:", l.to_ascii_uppercase()))
        }
        _ => None,
    }
}

pub fn remplacer(provisoire: &Path, cible: &Path) -> io::Result<()> {
    let a = large_chemin(provisoire);
    let b = large_chemin(cible);
    // SAFETY : deux chaînes terminées par zéro.
    unsafe {
        MoveFileExW(
            PCWSTR(a.as_ptr()),
            PCWSTR(b.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|e| io::Error::from_raw_os_error(e.code().0 & 0xFFFF))
}

/// COM pour le fil courant, rendu à la destruction si c'est nous qui l'avons ouvert.
struct Com(bool);

impl Com {
    fn ouvrir() -> Com {
        // SAFETY : appel simple ; S_FALSE (déjà ouvert) compte aussi comme réussite à rendre.
        let r = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        Com(r.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY : équilibre le CoInitializeEx réussi de ce fil.
            unsafe { CoUninitialize() };
        }
    }
}

/// HRESULT d'une erreur COM, en hexadécimal (jamais son message, qui peut citer un chemin).
fn hresult(appel: &str, e: ::windows::core::Error) -> String {
    format!("{appel} hresult {:#010x}", e.code().0 as u32)
}

pub fn creer_raccourci(lien: &Path, cible: &Path) -> Result<bool, String> {
    if let Some(parent) = lien.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("dossier {}", super::diagnostic(&e)))?;
    }
    let _com = Com::ouvrir();
    // SAFETY : objets COM créés et utilisés sur ce fil ; chaînes HSTRING tenues pendant
    // les appels.
    unsafe {
        let lien_shell: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| hresult("ShellLink", e))?;
        lien_shell
            .SetPath(&HSTRING::from(cible.as_os_str()))
            .map_err(|e| hresult("SetPath", e))?;
        if let Some(dossier) = cible.parent() {
            lien_shell
                .SetWorkingDirectory(&HSTRING::from(dossier.as_os_str()))
                .map_err(|e| hresult("SetWorkingDirectory", e))?;
        }
        lien_shell
            .SetDescription(&HSTRING::from("BreezyShot"))
            .map_err(|e| hresult("SetDescription", e))?;
        let fichier: IPersistFile = lien_shell.cast().map_err(|e| hresult("IPersistFile", e))?;
        fichier
            .Save(&HSTRING::from(lien.as_os_str()), true)
            .map_err(|e| hresult("Save", e))?;
    }
    Ok(true)
}

pub fn cible_raccourci(lien: &Path) -> Option<PathBuf> {
    if !lien.is_file() {
        return None;
    }
    let _com = Com::ouvrir();
    // SAFETY : comme `creer_raccourci` ; tampon local dont la taille est passée.
    unsafe {
        let lien_shell: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let fichier: IPersistFile = lien_shell.cast().ok()?;
        fichier
            .Load(&HSTRING::from(lien.as_os_str()), STGM_READ)
            .ok()?;
        let mut tampon = [0u16; 1024];
        lien_shell
            .GetPath(&mut tampon, std::ptr::null_mut(), 0)
            .ok()?;
        let fin = tampon.iter().position(|c| *c == 0).unwrap_or(tampon.len());
        Some(PathBuf::from(String::from_utf16_lossy(&tampon[..fin])))
    }
}

/// Clé HKCU ouverte, fermée à la destruction.
struct Cle(HKEY);

impl Drop for Cle {
    fn drop(&mut self) {
        // SAFETY : clé ouverte par RegCreateKeyExW ou RegOpenKeyExW.
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

pub fn ecrire_cle(cle: &str, valeurs: &[(&str, Valeur)]) -> Result<bool, String> {
    let nom = large(cle);
    let mut h = HKEY::default();
    // SAFETY : chaîne terminée par zéro, sortie dans une variable locale.
    let r = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(nom.as_ptr()),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut h,
            None,
        )
    };
    if r != ERROR_SUCCESS {
        return Err(format!("creation de la cle, code {}", r.0));
    }
    let h = Cle(h);
    for (nom_valeur, valeur) in valeurs {
        let n = large(nom_valeur);
        let r = match valeur {
            Valeur::Texte(t) => {
                let octets: Vec<u8> = large(t).iter().flat_map(|c| c.to_le_bytes()).collect();
                // SAFETY : clé ouverte en écriture, données locales.
                unsafe { RegSetValueExW(h.0, PCWSTR(n.as_ptr()), None, REG_SZ, Some(&octets)) }
            }
            Valeur::Nombre(v) => {
                let octets = v.to_le_bytes();
                // SAFETY : clé ouverte en écriture, quatre octets.
                unsafe { RegSetValueExW(h.0, PCWSTR(n.as_ptr()), None, REG_DWORD, Some(&octets)) }
            }
        };
        if r != ERROR_SUCCESS {
            return Err(format!("valeur {nom_valeur}, code {}", r.0));
        }
    }
    Ok(true)
}

pub fn lire_texte_cle(cle: &str, nom: &str) -> Option<String> {
    let c = large(cle);
    let n = large(nom);
    let mut tampon = vec![0u16; 2048];
    let mut taille = (tampon.len() * 2) as u32;
    // SAFETY : tampon local dont la taille en octets est passée à l'API.
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(c.as_ptr()),
            PCWSTR(n.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(tampon.as_mut_ptr() as *mut core::ffi::c_void),
            Some(&mut taille),
        )
    };
    if r != ERROR_SUCCESS {
        return None;
    }
    let fin = tampon.iter().position(|c| *c == 0).unwrap_or(tampon.len());
    Some(String::from_utf16_lossy(&tampon[..fin]))
}

pub fn retirer_valeurs_et_cle(cle: &str, noms: &[&str]) -> Result<bool, String> {
    let c = large(cle);
    let mut h = HKEY::default();
    // SAFETY : chaîne terminée par zéro, sortie dans une variable locale.
    let r = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(c.as_ptr()),
            None,
            KEY_SET_VALUE | KEY_QUERY_VALUE,
            &mut h,
        )
    };
    if r == ERROR_FILE_NOT_FOUND {
        return Ok(false);
    }
    if r != ERROR_SUCCESS {
        return Err(format!("ouverture de la cle, code {}", r.0));
    }
    let h = Cle(h);
    for nom in noms {
        let n = large(nom);
        // SAFETY : clé ouverte en écriture ; une valeur absente n'est pas une erreur.
        let _ = unsafe { RegDeleteValueW(h.0, PCWSTR(n.as_ptr())) };
    }
    let (mut sous_cles, mut valeurs) = (0u32, 0u32);
    // SAFETY : sorties dans des variables locales.
    let r = unsafe {
        RegQueryInfoKeyW(
            h.0,
            None,
            None,
            None,
            Some(&mut sous_cles),
            None,
            None,
            Some(&mut valeurs),
            None,
            None,
            None,
            None,
        )
    };
    drop(h);
    if r != ERROR_SUCCESS {
        return Err(format!("lecture de la cle, code {}", r.0));
    }
    if sous_cles > 0 || valeurs > 0 {
        return Err("la cle garde des valeurs qui ne sont pas a nous".into());
    }
    // SAFETY : chaîne terminée par zéro ; la clé est vide.
    let r = unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, PCWSTR(c.as_ptr())) };
    if r != ERROR_SUCCESS {
        return Err(format!("suppression de la cle, code {}", r.0));
    }
    Ok(true)
}

pub fn verrou_existe(nom: &str) -> bool {
    let n = large(nom);
    // SAFETY : chaîne terminée par zéro ; la poignée ouverte est refermée aussitôt.
    match unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, PCWSTR(n.as_ptr())) } {
        Ok(h) => {
            // SAFETY : poignée rendue juste au-dessus.
            unsafe {
                let _ = CloseHandle(h);
            }
            true
        }
        Err(_) => false,
    }
}

pub fn prendre_verrou(nom: &str) -> Option<Poignee> {
    let n = large(nom);
    // SAFETY : chaîne terminée par zéro ; GetLastError lu juste après l'appel.
    unsafe {
        let h = CreateMutexW(None, true, PCWSTR(n.as_ptr())).ok()?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(h);
            return None;
        }
        Some(Poignee(h))
    }
}

pub fn permettre_premier_plan() {
    // SAFETY : appel simple ; un refus ne change rien d'autre.
    let _ = unsafe { AllowSetForegroundWindow(ASFW_ANY) };
}

pub fn version_webview2() -> Option<String> {
    let mut version = PWSTR::null();
    // SAFETY : la chaîne rendue est libérée par CoTaskMemFree après copie.
    unsafe {
        GetAvailableCoreWebView2BrowserVersionString(PCWSTR::null(), &mut version).ok()?;
        if version.is_null() {
            return None;
        }
        let texte = version.to_string().ok();
        CoTaskMemFree(Some(version.0 as *const core::ffi::c_void));
        texte.filter(|t| !t.is_empty())
    }
}

fn rect_de(r: RECT) -> Rect {
    Rect {
        x: r.left,
        y: r.top,
        largeur: r.right - r.left,
        hauteur: r.bottom - r.top,
    }
}

pub fn zone_de_notification() -> Option<Rect> {
    // SAFETY : recherche de fenêtres par classe, rectangle lu dans une variable locale.
    unsafe {
        let barre = FindWindowW(&HSTRING::from("Shell_TrayWnd"), PCWSTR::null()).ok()?;
        let zone = FindWindowExW(
            Some(barre),
            None,
            &HSTRING::from("TrayNotifyWnd"),
            PCWSTR::null(),
        )
        .ok()?;
        let mut r = RECT::default();
        GetWindowRect(zone, &mut r).ok()?;
        let rect = rect_de(r);
        (rect.largeur > 0 && rect.hauteur > 0).then_some(rect)
    }
}

pub fn zone_utile_sous_souris() -> Option<(Rect, f64)> {
    // SAFETY : sorties dans des variables locales, taille de MONITORINFO renseignée.
    unsafe {
        let mut p = POINT::default();
        GetCursorPos(&mut p).ok()?;
        let ecran = MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(ecran, &mut info).as_bool() {
            return None;
        }
        let (mut dx, mut dy) = (96u32, 96u32);
        let echelle = match GetDpiForMonitor(ecran, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) {
            Ok(()) => dx as f64 / 96.0,
            Err(_) => 1.0,
        };
        Some((rect_de(info.rcWork), echelle))
    }
}

pub fn message_bloquant(titre: &str, texte: &str) {
    // SAFETY : chaînes HSTRING tenues pendant l'appel.
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(texte),
            &HSTRING::from(titre),
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND,
        );
    }
}

pub fn deplacer_fenetre(identifiant: isize, x: i32, y: i32) -> bool {
    if identifiant == 0 {
        return false;
    }
    let h = HWND(identifiant as *mut core::ffi::c_void);
    // SAFETY : une poignée périmée fait seulement échouer l'appel.
    unsafe {
        SetWindowPos(
            h,
            None,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        )
    }
    .is_ok()
}

pub fn rect_fenetre(identifiant: isize) -> Option<Rect> {
    if identifiant == 0 {
        return None;
    }
    let h = HWND(identifiant as *mut core::ffi::c_void);
    let mut r = RECT::default();
    // SAFETY : rectangle lu dans une variable locale ; une poignée périmée fait échouer.
    unsafe { GetWindowRect(h, &mut r) }.ok()?;
    Some(rect_de(r))
}

pub fn ramener_fenetre(titre: &str) -> bool {
    // SAFETY : recherche par titre ; une fenêtre disparue entre-temps fait échouer les appels.
    unsafe {
        let Ok(h) = FindWindowW(PCWSTR::null(), &HSTRING::from(titre)) else {
            return false;
        };
        if !IsWindowVisible(h).as_bool() {
            return false;
        }
        if IsIconic(h).as_bool() {
            let _ = ShowWindow(h, SW_RESTORE);
        }
        let _ = SetForegroundWindow(h);
        true
    }
}
