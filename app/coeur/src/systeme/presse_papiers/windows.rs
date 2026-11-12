//! Presse-papiers sous Windows : deux versions d'une image dans la même ouverture.
//!
//! Mémoire (documentation de `SetClipboardData`) : un bloc `GlobalAlloc(GMEM_MOVEABLE)`
//! accepté appartient ensuite au système, qui le libère ; un bloc refusé reste à nous et
//! est libéré ici. `CloseClipboard` est toujours appelé (garde `Drop`).

use ::windows::core::w;
use ::windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use ::windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use ::windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

use super::{contenu_dib, ErreurPressePapiers};

pub const DISPONIBLE: bool = true;

/// `CF_DIB` : « un BITMAPINFO suivi des bits de l'image » (formats standard du
/// presse-papiers, valeur 8). Recopié ici pour ne pas tirer `Win32_System_Ole`.
pub(super) const CF_DIB: u32 = 8;

/// Un bloc de mémoire globale encore à nous : libéré par `Drop`, sauf s'il a été donné
/// au presse-papiers.
struct Bloc(Option<HGLOBAL>);

impl Bloc {
    /// Un bloc déplaçable qui contient une copie de `octets` (non vide).
    fn copie_de(octets: &[u8]) -> Result<Bloc, ErreurPressePapiers> {
        if octets.is_empty() {
            return Err(ErreurPressePapiers::Refuse);
        }
        // SAFETY : allocation sans pointeur ; le bloc est rendu par `Drop`.
        let bloc = Bloc(Some(
            unsafe { GlobalAlloc(GMEM_MOVEABLE, octets.len()) }
                .map_err(|_| ErreurPressePapiers::Refuse)?,
        ));
        let poignee = bloc.0.ok_or(ErreurPressePapiers::Refuse)?;
        // SAFETY : bloc de `octets.len()` octets au moins, verrouillé le temps de la
        // copie puis déverrouillé.
        unsafe {
            let destination = GlobalLock(poignee) as *mut u8;
            if destination.is_null() {
                return Err(ErreurPressePapiers::Refuse);
            }
            std::ptr::copy_nonoverlapping(octets.as_ptr(), destination, octets.len());
            // Rend une « erreur » quand le compteur de verrous retombe à zéro : normal.
            let _ = GlobalUnlock(poignee);
        }
        Ok(bloc)
    }

    /// Donne le bloc au presse-papiers (ouvert et vidé par ce fil) sous `format`.
    /// Accepté : le système en est propriétaire. Refusé : `Drop` le libère.
    fn donner(mut self, format: u32) -> Result<(), ErreurPressePapiers> {
        let poignee = self.0.ok_or(ErreurPressePapiers::Refuse)?;
        // SAFETY : presse-papiers ouvert par ce fil ; bloc déplaçable, déverrouillé.
        match unsafe { SetClipboardData(format, Some(HANDLE(poignee.0))) } {
            Ok(_) => {
                self.0 = None;
                Ok(())
            }
            Err(_) => Err(ErreurPressePapiers::Refuse),
        }
    }
}

impl Drop for Bloc {
    fn drop(&mut self) {
        if let Some(poignee) = self.0.take() {
            // SAFETY : bloc à nous, jamais donné, libéré une seule fois.
            let _ = unsafe { GlobalFree(Some(poignee)) };
        }
    }
}

/// Le presse-papiers ouvert par ce fil ; fermé par `Drop`, même en cas d'erreur.
struct Ouvert;

impl Ouvert {
    fn ouvrir() -> Result<Ouvert, ErreurPressePapiers> {
        // SAFETY : sans tampon. Sans fenêtre propriétaire, comme `arboard`.
        unsafe { OpenClipboard(None) }.map_err(|_| ErreurPressePapiers::Occupe)?;
        Ok(Ouvert)
    }
}

impl Drop for Ouvert {
    fn drop(&mut self) {
        // SAFETY : ouvert par ce fil dans `ouvrir`.
        let _ = unsafe { CloseClipboard() };
    }
}

/// Numéro du format « PNG » (le même pour toutes les apps de la session).
pub(super) fn format_png() -> Option<u32> {
    // SAFETY : chaîne constante terminée par un zéro.
    let format = unsafe { RegisterClipboardFormatW(w!("PNG")) };
    (format != 0).then_some(format)
}

pub fn poser_image(
    png: &[u8],
    largeur: u32,
    hauteur: u32,
    rgba_sur_blanc: &[u8],
) -> Result<(), ErreurPressePapiers> {
    let format_png = format_png().ok_or(ErreurPressePapiers::Refuse)?;
    let dib = contenu_dib(largeur, hauteur, rgba_sur_blanc).ok_or(ErreurPressePapiers::Refuse)?;
    // Les blocs sont prêts avant l'ouverture : le presse-papiers reste ouvert le moins
    // longtemps possible.
    let bloc_png = Bloc::copie_de(png)?;
    let bloc_dib = Bloc::copie_de(&dib)?;
    drop(dib);

    let _ouvert = Ouvert::ouvrir()?;
    // SAFETY : presse-papiers ouvert par ce fil ; nous en devenons propriétaires.
    unsafe { EmptyClipboard() }.map_err(|_| ErreurPressePapiers::Refuse)?;
    // Le PNG d'abord : des apps prennent le premier format qu'elles savent lire.
    bloc_png.donner(format_png)?;
    if let Err(e) = bloc_dib.donner(CF_DIB) {
        // Pas de demi-image : on vide (le système libère le PNG qu'il possède).
        // SAFETY : presse-papiers toujours ouvert par ce fil.
        let _ = unsafe { EmptyClipboard() };
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    use ::windows::Win32::Graphics::Gdi::{BITMAPINFOHEADER, BI_RGB};
    use ::windows::Win32::System::DataExchange::GetClipboardData;
    use ::windows::Win32::System::Memory::GlobalSize;

    use crate::systeme::presse_papiers::{TAILLE_ENTETE_DIB, VERROU_ESSAIS};

    #[test]
    fn entete_dib_de_la_bonne_taille() {
        assert_eq!(std::mem::size_of::<BITMAPINFOHEADER>(), TAILLE_ENTETE_DIB);
        assert_eq!(BI_RGB.0, 0);
    }

    /// Lit un format du presse-papiers (ouvert par ce fil) : une copie de ses octets.
    fn lire_format(format: u32) -> Vec<u8> {
        // SAFETY : presse-papiers ouvert par ce fil ; le bloc appartient au système, on
        // le lit verrouillé, sans le libérer.
        unsafe {
            let poignee = GetClipboardData(format).expect("format present");
            let bloc = HGLOBAL(poignee.0);
            let taille = GlobalSize(bloc);
            let source = GlobalLock(bloc) as *const u8;
            assert!(!source.is_null(), "bloc illisible");
            let copie = std::slice::from_raw_parts(source, taille).to_vec();
            let _ = GlobalUnlock(bloc);
            copie
        }
    }

    /// Ouvre le presse-papiers en réessayant pendant 1 s (une autre app peut le tenir).
    fn ouvrir_pour_lire() -> Ouvert {
        let debut = Instant::now();
        loop {
            match Ouvert::ouvrir() {
                Ok(o) => return o,
                Err(e) if debut.elapsed() >= Duration::from_secs(1) => {
                    panic!("presse-papiers jamais ouvert en 1 s : {e:?}")
                }
                Err(_) => std::thread::sleep(Duration::from_millis(20)),
            }
        }
    }

    #[test]
    fn poser_image_pose_le_png_et_le_dib_sur_blanc() {
        let _seul = VERROU_ESSAIS.lock().unwrap_or_else(|e| e.into_inner());
        // 3 × 2 vu comme déjà composé sur blanc : coin haut gauche blanc (alpha 0 avant),
        // le reste en couleurs connues.
        let (l, h) = (3u32, 2u32);
        let sur_blanc: Vec<u8> = [
            [255, 255, 255],
            [255, 0, 0],
            [0, 255, 0],
            [0, 0, 255],
            [10, 20, 30],
            [200, 100, 50],
        ]
        .iter()
        .flat_map(|[r, v, b]| [*r, *v, *b, 255])
        .collect();
        // Des octets de « fichier » reconnaissables (le presse-papiers ne les décode pas).
        let png: Vec<u8> = (0..1000u32).map(|i| (i * 7 % 251) as u8).collect();

        let debut = Instant::now();
        let pose = loop {
            match poser_image(&png, l, h, &sur_blanc) {
                Err(ErreurPressePapiers::Occupe) if debut.elapsed() < Duration::from_secs(1) => {
                    std::thread::sleep(Duration::from_millis(20))
                }
                autre => break autre,
            }
        };
        assert_eq!(pose, Ok(()));

        let _ouvert = ouvrir_pour_lire();
        // PNG : les mêmes octets (`GlobalSize` peut arrondir la taille au-dessus).
        let lu_png = lire_format(format_png().expect("format PNG"));
        assert!(lu_png.len() >= png.len(), "{} octets", lu_png.len());
        assert_eq!(&lu_png[..png.len()], png.as_slice());

        // CF_DIB : en-tête, puis lignes de bas en haut en BGR.
        let dib = lire_format(CF_DIB);
        assert!(dib.len() >= TAILLE_ENTETE_DIB + (l * h * 4) as usize);
        // SAFETY : au moins 40 octets lus, lecture non alignée.
        let entete: BITMAPINFOHEADER =
            unsafe { std::ptr::read_unaligned(dib.as_ptr() as *const BITMAPINFOHEADER) };
        assert_eq!(entete.biSize as usize, TAILLE_ENTETE_DIB);
        assert_eq!((entete.biWidth, entete.biHeight), (l as i32, h as i32));
        assert_eq!((entete.biPlanes, entete.biBitCount), (1, 32));
        assert_eq!(entete.biCompression, BI_RGB.0);
        let pixel = |x: u32, y: u32| {
            // Ligne `y` (comptée du haut) rangée à la place `h - 1 - y`.
            let i = TAILLE_ENTETE_DIB + (((h - 1 - y) * l + x) * 4) as usize;
            [dib[i + 2], dib[i + 1], dib[i]]
        };
        assert_eq!(pixel(0, 0), [255, 255, 255], "coin blanc");
        assert_eq!(pixel(1, 0), [255, 0, 0]);
        assert_eq!(pixel(2, 0), [0, 255, 0]);
        assert_eq!(pixel(0, 1), [0, 0, 255]);
        assert_eq!(pixel(1, 1), [10, 20, 30]);
        assert_eq!(pixel(2, 1), [200, 100, 50]);
        println!("Presse-papiers : PNG et CF_DIB poses ensemble et relus.");
    }

    #[test]
    fn poser_image_refuse_des_pixels_incoherents() {
        let _seul = VERROU_ESSAIS.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(
            poser_image(&[1, 2, 3], 2, 2, &[255; 15]),
            Err(ErreurPressePapiers::Refuse)
        );
        assert_eq!(
            poser_image(&[], 1, 1, &[255; 4]),
            Err(ErreurPressePapiers::Refuse)
        );
    }
}
