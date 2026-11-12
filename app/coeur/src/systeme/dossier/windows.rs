//! Windows : `IFileOpenDialog` avec `FOS_PICKFOLDERS`, dans un appartement COM à un fil
//! (le fil de l'appelant, qui n'est pas le fil principal).

use std::path::PathBuf;

use ::windows::core::{HSTRING, PCWSTR};
use ::windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
use ::windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
};
use ::windows::Win32::UI::Shell::{
    FileOpenDialog, IFileOpenDialog, IShellItem, SHCreateItemFromParsingName, FOS_FORCEFILESYSTEM,
    FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
};

/// Désinitialise COM à la sortie, quoi qu'il arrive.
struct Appartement;

impl Drop for Appartement {
    fn drop(&mut self) {
        // SAFETY : appelé une fois, après un CoInitializeEx réussi sur ce fil.
        unsafe { CoUninitialize() };
    }
}

pub fn choisir_dossier(
    proprietaire: Option<isize>,
    depart: Option<PathBuf>,
) -> Result<Option<PathBuf>, String> {
    // SAFETY : initialisation COM de ce fil ; la garde la défait.
    let _appartement = unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE)
            .ok()
            .map_err(|e| format!("com: {e}"))?;
        Appartement
    };
    // SAFETY : objets COM créés et utilisés sur ce seul fil ; chaînes vivantes pendant
    // les appels ; le texte rendu par GetDisplayName est libéré par CoTaskMemFree.
    unsafe {
        let dialogue: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| format!("dialogue: {e}"))?;
        let options = dialogue.GetOptions().map_err(|e| format!("options: {e}"))?;
        dialogue
            .SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST)
            .map_err(|e| format!("options: {e}"))?;
        if let Some(d) = depart.filter(|d| d.is_dir()) {
            let texte = HSTRING::from(d.as_os_str());
            if let Ok(element) =
                SHCreateItemFromParsingName::<_, _, IShellItem>(PCWSTR(texte.as_ptr()), None)
            {
                let _ = dialogue.SetFolder(&element);
            }
        }
        let fenetre = proprietaire.map(|h| HWND(h as *mut core::ffi::c_void));
        if let Err(e) = dialogue.Show(fenetre) {
            if e.code() == ERROR_CANCELLED.to_hresult() {
                return Ok(None);
            }
            return Err(format!("dialogue: {e}"));
        }
        let element = dialogue.GetResult().map_err(|e| format!("resultat: {e}"))?;
        let texte = element
            .GetDisplayName(SIGDN_FILESYSPATH)
            .map_err(|e| format!("chemin: {e}"))?;
        let chemin = texte.to_string();
        CoTaskMemFree(Some(texte.0 as *const core::ffi::c_void));
        let chemin = chemin.map_err(|e| format!("chemin: {e}"))?;
        Ok(Some(PathBuf::from(chemin)))
    }
}
