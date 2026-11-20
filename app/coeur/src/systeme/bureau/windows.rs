//! Masquage temporaire des icônes du bureau sous Windows (`Progman` / `WorkerW` →
//! `SHELLDLL_DefView` → `SysListView32`). N'utilise que `user32.dll` (déjà chargée).

use core::ffi::c_void;

use ::windows::core::{w, BOOL, PCWSTR};
use ::windows::Win32::Foundation::{HWND, LPARAM};
use ::windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, FindWindowW, IsWindow, IsWindowVisible, ShowWindow,
    ShowWindowAsync, SW_HIDE, SW_SHOWNOACTIVATE,
};

unsafe extern "system" fn chercher_defview(hwnd: HWND, donnee: LPARAM) -> BOOL {
    // SAFETY : `donnee` est l'adresse de `Option<HWND>` dans `trouver_defview`, vivante
    // pendant `EnumWindows`.
    let trouve = unsafe { &mut *(donnee.0 as *mut Option<HWND>) };
    if let Ok(defview) =
        unsafe { FindWindowExW(Some(hwnd), None, w!("SHELLDLL_DefView"), PCWSTR::null()) }
    {
        if !defview.0.is_null() {
            *trouve = Some(defview);
            return BOOL(0);
        }
    }
    BOOL(1)
}

fn trouver_defview() -> Option<HWND> {
    // SAFETY : recherche de fenêtres par noms de classes constants.
    unsafe {
        if let Ok(progman) = FindWindowW(w!("Progman"), PCWSTR::null()) {
            if !progman.0.is_null() {
                if let Ok(defview) =
                    FindWindowExW(Some(progman), None, w!("SHELLDLL_DefView"), PCWSTR::null())
                {
                    if !defview.0.is_null() {
                        return Some(defview);
                    }
                }
            }
        }
        let mut trouve: Option<HWND> = None;
        let _ = EnumWindows(
            Some(chercher_defview),
            LPARAM(&mut trouve as *mut Option<HWND> as isize),
        );
        trouve
    }
}

fn trouver_liste_icones() -> Option<HWND> {
    let defview = trouver_defview()?;
    // SAFETY : `defview` est une poignée de fenêtre valide ; recherche de son enfant
    // `SysListView32` par classe constante.
    let liste =
        unsafe { FindWindowExW(Some(defview), None, w!("SysListView32"), PCWSTR::null()) }.ok()?;
    (!liste.0.is_null()).then_some(liste)
}

pub(super) fn masquer() -> Option<isize> {
    let liste = trouver_liste_icones()?;
    // SAFETY : `liste` est la poignée de `SysListView32`. Si l'utilisateur cache déjà ses
    // icônes du bureau, on ne touche à rien.
    unsafe {
        if !IsWindowVisible(liste).as_bool() {
            return None;
        }
        let _ = ShowWindow(liste, SW_HIDE);
    }
    Some(liste.0 as isize)
}

pub(super) fn restaurer(fenetre: isize) {
    if fenetre == 0 {
        return;
    }
    let h = HWND(fenetre as *mut c_void);
    // SAFETY : poignée vérifiée par `IsWindow` ; `ShowWindowAsync(SW_SHOWNOACTIVATE)`
    // évite de voler le focus et de bloquer en `Drop` si Explorer est occupé.
    unsafe {
        if IsWindow(Some(h)).as_bool() {
            let _ = ShowWindowAsync(h, SW_SHOWNOACTIVATE);
        }
    }
}
