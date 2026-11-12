//! Windows : `IFileOpenDialog` avec `FOS_PICKFOLDERS` et `IFileSaveDialog`, dans un
//! appartement COM à un fil (le fil de l'appelant, qui n'est pas le fil principal).

use std::path::{Path, PathBuf};

use ::windows::core::{HSTRING, PCWSTR};
use ::windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
use ::windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
};
use ::windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use ::windows::Win32::UI::Shell::{
    FileOpenDialog, FileSaveDialog, IFileOpenDialog, IFileSaveDialog, IShellItem,
    SHCreateItemFromParsingName, FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST,
    FOS_PICKFOLDERS, FOS_STRICTFILETYPES, SIGDN_FILESYSPATH,
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

/// Le dialogue « Enregistrer sous » (`IFileSaveDialog`, même appartement COM à un fil) :
/// filtre « Image PNG » seul, extension `png` par défaut et imposée, remplacement confirmé
/// par le système, chemins du disque seulement, modal à `proprietaire`. Annulé (Échap,
/// Annuler, croix) : `Ok(None)`. Les causes rendues ne contiennent jamais de chemin.
pub fn choisir_fichier_png(
    proprietaire: Option<isize>,
    depart: Option<&Path>,
    nom: &str,
) -> Result<Option<PathBuf>, String> {
    // SAFETY : initialisation COM de ce fil ; la garde la défait sur tous les retours.
    let _appartement = unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE)
            .ok()
            .map_err(|e| format!("com: {:#010x}", e.code().0))?;
        Appartement
    };
    let filtre_nom = super::large(super::FILTRE_PNG.0);
    let filtre_motif = super::large(super::FILTRE_PNG.1);
    let extension = super::large(super::EXTENSION_PNG);
    let filtres = [COMDLG_FILTERSPEC {
        pszName: PCWSTR(filtre_nom.as_ptr()),
        pszSpec: PCWSTR(filtre_motif.as_ptr()),
    }];
    // SAFETY : objets COM créés, utilisés et relâchés sur ce seul fil, avant la garde
    // (déclarée plus haut, donc défaite après eux) ; les tableaux UTF-16 vivent jusqu'à la
    // fin de la fonction ; le texte rendu par GetDisplayName est libéré par CoTaskMemFree.
    let choisi = unsafe {
        let dialogue: IFileSaveDialog =
            CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| format!("dialogue: {:#010x}", e.code().0))?;
        let options = dialogue
            .GetOptions()
            .map_err(|e| format!("options: {:#010x}", e.code().0))?;
        dialogue
            .SetOptions(options | FOS_OVERWRITEPROMPT | FOS_FORCEFILESYSTEM | FOS_STRICTFILETYPES)
            .map_err(|e| format!("options: {:#010x}", e.code().0))?;
        dialogue
            .SetFileTypes(&filtres)
            .map_err(|e| format!("filtre: {:#010x}", e.code().0))?;
        dialogue
            .SetFileTypeIndex(1)
            .map_err(|e| format!("filtre: {:#010x}", e.code().0))?;
        dialogue
            .SetDefaultExtension(PCWSTR(extension.as_ptr()))
            .map_err(|e| format!("extension: {:#010x}", e.code().0))?;
        if let Some(nom) = super::nom_propose(nom) {
            let texte = super::large(&nom);
            // Un nom refusé laisse le champ vide : l'utilisateur en tape un.
            let _ = dialogue.SetFileName(PCWSTR(texte.as_ptr()));
        }
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
            return Err(format!("dialogue: {:#010x}", e.code().0));
        }
        let element = dialogue
            .GetResult()
            .map_err(|e| format!("resultat: {:#010x}", e.code().0))?;
        let texte = element
            .GetDisplayName(SIGDN_FILESYSPATH)
            .map_err(|e| format!("chemin: {:#010x}", e.code().0))?;
        let chemin = super::chemin_depuis_large(texte.as_wide());
        CoTaskMemFree(Some(texte.0 as *const core::ffi::c_void));
        chemin?
    };
    super::chemin_png(choisi, |c| c.symlink_metadata().is_ok()).map(Some)
}
