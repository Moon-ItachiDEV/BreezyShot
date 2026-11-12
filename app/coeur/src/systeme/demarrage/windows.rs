//! Windows : la valeur `BreezyShot` de `HKCU\…\Run` (ou de la clé d'essai).

use ::windows::core::{HSTRING, PCWSTR};
use ::windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use ::windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteKeyValueW, RegGetValueW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RRF_NOEXPAND,
    RRF_RT_REG_BINARY, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
};

use super::{etat_depuis, EtatDemarrage};

/// Lit une valeur (texte ou binaire) en octets bruts ; `None` si la clé ou la valeur
/// manque. Au plus 4 Kio : la valeur `Run` d'un chemin Windows tient bien en dessous.
fn lire_octets(cle: &str, nom: &str, texte: bool) -> Option<Vec<u8>> {
    let cle = HSTRING::from(cle);
    let nom = HSTRING::from(nom);
    let mut tampon = vec![0u8; 4096];
    let mut taille = tampon.len() as u32;
    let genre = if texte {
        RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND
    } else {
        RRF_RT_REG_BINARY
    };
    // SAFETY : tampon local dont la taille en octets est passée à l'API.
    let resultat = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(cle.as_ptr()),
            PCWSTR(nom.as_ptr()),
            genre,
            None,
            Some(tampon.as_mut_ptr() as *mut core::ffi::c_void),
            Some(&mut taille),
        )
    };
    if resultat != ERROR_SUCCESS {
        return None;
    }
    tampon.truncate(taille as usize);
    Some(tampon)
}

pub fn lire(run: &str, approuve: &str, nom: &str) -> EtatDemarrage {
    let texte = lire_octets(run, nom, true).map(|o| {
        let mots: Vec<u16> = o
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&c| c != 0)
            .collect();
        String::from_utf16_lossy(&mots)
    });
    let approuve = lire_octets(approuve, nom, false);
    etat_depuis(texte.as_deref(), approuve.as_deref())
}

pub fn ecrire(cle: &str, nom: &str, texte: &str) -> Result<(), String> {
    let chemin = HSTRING::from(cle);
    let nom = HSTRING::from(nom);
    let mut ouverte = HKEY::default();
    // SAFETY : chaînes terminées par zéro tenues par HSTRING pendant l'appel.
    let resultat = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(chemin.as_ptr()),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut ouverte,
            None,
        )
    };
    if resultat != ERROR_SUCCESS {
        return Err(format!("ouverture de la cle: {}", resultat.0));
    }
    let mut mots: Vec<u16> = texte.encode_utf16().collect();
    mots.push(0);
    // SAFETY : octets du texte UTF-16 terminé par zéro, vivant pendant l'appel.
    let octets = unsafe { std::slice::from_raw_parts(mots.as_ptr() as *const u8, mots.len() * 2) };
    // SAFETY : clé ouverte ci-dessus, fermée juste après.
    let resultat = unsafe {
        let r = RegSetValueExW(ouverte, PCWSTR(nom.as_ptr()), None, REG_SZ, Some(octets));
        let _ = RegCloseKey(ouverte);
        r
    };
    if resultat != ERROR_SUCCESS {
        return Err(format!("ecriture de la valeur: {}", resultat.0));
    }
    Ok(())
}

pub fn supprimer(cle: &str, nom: &str) -> Result<(), String> {
    let chemin = HSTRING::from(cle);
    let nom = HSTRING::from(nom);
    // SAFETY : chaînes terminées par zéro tenues par HSTRING pendant l'appel.
    let resultat = unsafe {
        RegDeleteKeyValueW(
            HKEY_CURRENT_USER,
            PCWSTR(chemin.as_ptr()),
            PCWSTR(nom.as_ptr()),
        )
    };
    if resultat == ERROR_SUCCESS || resultat == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        Err(format!("suppression de la valeur: {}", resultat.0))
    }
}
