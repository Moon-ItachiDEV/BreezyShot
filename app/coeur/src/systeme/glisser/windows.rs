//! Glisser-déposer sous Windows : `SHDoDragDrop` avec l'objet de données du Shell.
//!
//! Aucune interface COM n'est écrite ici : l'objet de données vient du Shell
//! (`SHCreateItemFromParsingName` puis `BindToHandler(BHID_DataObject)`), la source est
//! celle que `SHDoDragDrop` fournit quand on ne lui en donne pas (elle suit le bouton
//! principal et Échap), et l'image du glisser est celle du Shell. OLE est déjà initialisé sur
//! le fil principal (tao, à la création des fenêtres).

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use ::windows::core::PCWSTR;
use ::windows::Win32::Foundation::{HWND, POINT};
use ::windows::Win32::System::Com::{IBindCtx, IDataObject};
use ::windows::Win32::System::Ole::{IDropSource, DROPEFFECT_COPY, DROPEFFECT_NONE};
use ::windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
use ::windows::Win32::UI::Shell::{
    BHID_DataObject, IShellItem, SHCreateItemFromParsingName, SHDoDragDrop,
};
use ::windows::Win32::UI::WindowsAndMessaging::{
    GetAncestor, GetSystemMetrics, WindowFromPoint, GA_ROOT, SM_SWAPBUTTON,
};

use super::{BoutonPhysique, Depot};

pub fn glisser_fichier(fenetre: isize, chemin: &Path) -> Result<Depot, String> {
    let large: Vec<u16> = chemin
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY : chaîne terminée par un zéro, vivante pendant l'appel ; les interfaces
    // rendues sont des pointeurs comptés que windows-rs libère.
    let donnees: IDataObject = unsafe {
        let element: IShellItem =
            SHCreateItemFromParsingName(PCWSTR(large.as_ptr()), None::<&IBindCtx>)
                .map_err(|e| format!("element du Shell: {e}"))?;
        element
            .BindToHandler(None::<&IBindCtx>, &BHID_DataObject)
            .map_err(|e| format!("objet de donnees: {e}"))?
    };
    let h = HWND(fenetre as *mut core::ffi::c_void);
    // SAFETY : fil principal (STA, OLE initialisé) ; `donnees` vit pendant tout l'appel,
    // qui ne rend la main qu'à la fin du glisser.
    let effet = unsafe { SHDoDragDrop(Some(h), &donnees, None::<&IDropSource>, DROPEFFECT_COPY) }
        .map_err(|e| format!("SHDoDragDrop: {e}"))?;
    Ok(if effet == DROPEFFECT_NONE {
        Depot::Annule
    } else {
        Depot::Depose
    })
}

pub fn boutons_inverses() -> bool {
    // SAFETY : aucun tampon ; non nul veut dire « boutons inversés ».
    unsafe { GetSystemMetrics(SM_SWAPBUTTON) != 0 }
}

pub fn bouton_enfonce(bouton: BoutonPhysique) -> bool {
    // `GetAsyncKeyState` lit le bouton physique, sans tenir compte de l'inversion.
    let touche = match bouton {
        BoutonPhysique::Gauche => VK_LBUTTON,
        BoutonPhysique::Droit => VK_RBUTTON,
    };
    // SAFETY : aucun tampon ; le bit de poids fort dit « enfoncé maintenant ».
    let etat = unsafe { GetAsyncKeyState(i32::from(touche.0)) };
    etat < 0
}

pub fn fenetre_sous(point: (i32, i32)) -> Option<isize> {
    // SAFETY : aucun tampon ; un handle nul veut dire « rien ».
    unsafe {
        let h = WindowFromPoint(POINT {
            x: point.0,
            y: point.1,
        });
        if h.0.is_null() {
            return None;
        }
        let racine = GetAncestor(h, GA_ROOT);
        let r = if racine.0.is_null() { h } else { racine };
        Some(r.0 as isize)
    }
}
