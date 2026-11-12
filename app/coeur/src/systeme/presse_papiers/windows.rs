//! Presse-papiers sous Windows : deux versions d'une image dans la même ouverture.
//!
//! Mémoire (documentation de `SetClipboardData`) : un bloc `GlobalAlloc(GMEM_MOVEABLE)`
//! accepté appartient ensuite au système, qui le libère ; un bloc refusé reste à nous et
//! est libéré ici. `CloseClipboard` est toujours appelé (garde `Drop`).

use std::time::Duration;

use ::windows::core::w;
use ::windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use ::windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use ::windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

use super::{ecrire_dib, taille_dib, ErreurPressePapiers};

pub const DISPONIBLE: bool = true;

/// `CF_DIB` : « un BITMAPINFO suivi des bits de l'image » (formats standard du
/// presse-papiers, valeur 8). Recopié ici pour ne pas tirer `Win32_System_Ole`.
pub(super) const CF_DIB: u32 = 8;

/// Un bloc de mémoire globale encore à nous : libéré par `Drop`, sauf s'il a été donné
/// au presse-papiers.
struct Bloc(Option<HGLOBAL>);

impl Bloc {
    /// Un bloc déplaçable de `taille` octets (non nulle), rempli par `remplir` pendant
    /// qu'il est verrouillé. `remplir` rend faux : refusé, le bloc est libéré.
    fn rempli(
        taille: usize,
        remplir: impl FnOnce(&mut [u8]) -> bool,
    ) -> Result<Bloc, ErreurPressePapiers> {
        if taille == 0 {
            return Err(ErreurPressePapiers::Refuse);
        }
        // SAFETY : allocation sans pointeur ; le bloc est rendu par `Drop`.
        let poignee = unsafe { GlobalAlloc(GMEM_MOVEABLE, taille) }
            .map_err(|_| ErreurPressePapiers::Refuse)?;
        let bloc = Bloc(Some(poignee));
        // SAFETY : bloc de `taille` octets au moins, verrouillé le temps du remplissage
        // puis déverrouillé ; la tranche ne sort pas de ce bloc.
        let rempli = unsafe {
            let destination = GlobalLock(poignee) as *mut u8;
            if destination.is_null() {
                return Err(ErreurPressePapiers::Refuse);
            }
            let rempli = remplir(std::slice::from_raw_parts_mut(destination, taille));
            // Rend une « erreur » quand le compteur de verrous retombe à zéro : normal.
            let _ = GlobalUnlock(poignee);
            rempli
        };
        if rempli {
            Ok(bloc)
        } else {
            Err(ErreurPressePapiers::Refuse)
        }
    }

    /// Donne le bloc au presse-papiers (ouvert et vidé par ce fil) sous `format`.
    /// Accepté : le système en est propriétaire et le bloc est oublié ici. Refusé : il
    /// reste à nous (libéré par `Drop`). Déjà donné : `Refuse`.
    fn donner(&mut self, format: u32) -> Result<(), ErreurPressePapiers> {
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

/// Ouvertures tentées par essai, et l'attente entre deux (comme `arboard`).
const OUVERTURES: u32 = 6;
const ENTRE_OUVERTURES: Duration = Duration::from_millis(5);

/// Le presse-papiers ouvert par ce fil ; fermé par `Drop`, même en cas d'erreur.
struct Ouvert;

impl Ouvert {
    /// Une ouverture, sans attente.
    fn une_fois() -> Result<Ouvert, ErreurPressePapiers> {
        // SAFETY : sans tampon. Sans fenêtre propriétaire, comme `arboard`.
        unsafe { OpenClipboard(None) }.map_err(|_| ErreurPressePapiers::Occupe)?;
        Ok(Ouvert)
    }

    /// `OUVERTURES` ouvertures au plus, séparées de `ENTRE_OUVERTURES`.
    fn ouvrir() -> Result<Ouvert, ErreurPressePapiers> {
        let mut restantes = OUVERTURES;
        loop {
            match Ouvert::une_fois() {
                Ok(o) => return Ok(o),
                Err(e) => {
                    restantes -= 1;
                    if restantes == 0 {
                        return Err(e);
                    }
                    std::thread::sleep(ENTRE_OUVERTURES);
                }
            }
        }
    }
}

impl Drop for Ouvert {
    fn drop(&mut self) {
        // SAFETY : ouvert par ce fil dans `une_fois`.
        let _ = unsafe { CloseClipboard() };
    }
}

/// Numéro du format « PNG » (le même pour toutes les apps de la session).
pub(super) fn format_png() -> Option<u32> {
    // SAFETY : chaîne constante terminée par un zéro.
    let format = unsafe { RegisterClipboardFormatW(w!("PNG")) };
    (format != 0).then_some(format)
}

pub struct Preparee {
    format_png: u32,
    png: Bloc,
    dib: Bloc,
}

pub fn preparer(
    png: &[u8],
    largeur: u32,
    hauteur: u32,
    rgba_sur_blanc: &[u8],
) -> Result<Preparee, ErreurPressePapiers> {
    let format_png = format_png().ok_or(ErreurPressePapiers::Refuse)?;
    let taille = taille_dib(largeur, hauteur, rgba_sur_blanc).ok_or(ErreurPressePapiers::Refuse)?;
    let png = Bloc::rempli(png.len(), |d| {
        d.copy_from_slice(png);
        true
    })?;
    // Le DIB est écrit directement dans son bloc, sans copie intermédiaire.
    let dib = Bloc::rempli(taille, |d| ecrire_dib(largeur, hauteur, rgba_sur_blanc, d))?;
    Ok(Preparee {
        format_png,
        png,
        dib,
    })
}

pub fn poser(image: &mut Preparee) -> Result<(), ErreurPressePapiers> {
    // Déjà posée ou refusée : il n'y a plus rien à donner.
    if image.png.0.is_none() || image.dib.0.is_none() {
        return Err(ErreurPressePapiers::Refuse);
    }
    let _ouvert = Ouvert::ouvrir()?;
    // SAFETY : presse-papiers ouvert par ce fil ; nous en devenons propriétaires.
    unsafe { EmptyClipboard() }.map_err(|_| ErreurPressePapiers::Refuse)?;
    // Le PNG d'abord : des apps prennent le premier format qu'elles savent lire.
    image.png.donner(image.format_png)?;
    if let Err(e) = image.dib.donner(CF_DIB) {
        // Pas de demi-image : on vide (le système libère le PNG qu'il possède) ; le bloc
        // du DIB, refusé, reste à nous et sera libéré avec l'image.
        // SAFETY : presse-papiers toujours ouvert par ce fil.
        let _ = unsafe { EmptyClipboard() };
        return Err(e);
    }
    Ok(())
}

/// Ouvre le presse-papiers en réessayant pendant 1 s (une autre app peut le tenir).
#[cfg(test)]
fn ouvrir_pour_lire() -> Option<Ouvert> {
    let debut = std::time::Instant::now();
    loop {
        match Ouvert::ouvrir() {
            Ok(o) => return Some(o),
            Err(_) if debut.elapsed() >= Duration::from_secs(1) => return None,
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

/// Lit un format du presse-papiers (ouvert par ce fil) : une copie de ses octets.
#[cfg(test)]
fn lire_format(format: u32) -> Option<Vec<u8>> {
    use ::windows::Win32::System::DataExchange::GetClipboardData;
    use ::windows::Win32::System::Memory::GlobalSize;
    // SAFETY : presse-papiers ouvert par ce fil ; le bloc appartient au système, on le
    // lit verrouillé, sans le libérer.
    unsafe {
        let poignee = GetClipboardData(format).ok()?;
        let bloc = HGLOBAL(poignee.0);
        let taille = GlobalSize(bloc);
        let source = GlobalLock(bloc) as *const u8;
        if source.is_null() {
            return None;
        }
        let copie = std::slice::from_raw_parts(source, taille).to_vec();
        let _ = GlobalUnlock(bloc);
        Some(copie)
    }
}

#[cfg(test)]
pub fn lire_dib() -> Option<Vec<u8>> {
    let _ouvert = ouvrir_pour_lire()?;
    lire_format(CF_DIB)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    use ::windows::Win32::Graphics::Gdi::{BITMAPINFOHEADER, BI_RGB};

    use crate::systeme::presse_papiers::{
        pixel_dib, ImagePreparee, TAILLE_ENTETE_DIB, VERROU_ESSAIS,
    };

    #[test]
    fn entete_dib_de_la_bonne_taille() {
        assert_eq!(std::mem::size_of::<BITMAPINFOHEADER>(), TAILLE_ENTETE_DIB);
        assert_eq!(BI_RGB.0, 0);
    }

    #[test]
    fn image_preparee_pose_le_png_et_le_dib_sur_blanc() {
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

        // Préparée une fois ; seuls les essais « occupé » recommencent.
        let mut image = ImagePreparee::preparer(&png, l, h, &sur_blanc).expect("preparee");
        let debut = Instant::now();
        let pose = loop {
            match image.poser() {
                Err(ErreurPressePapiers::Occupe) if debut.elapsed() < Duration::from_secs(1) => {
                    std::thread::sleep(Duration::from_millis(20))
                }
                autre => break autre,
            }
        };
        assert_eq!(pose, Ok(()));
        // Les blocs appartiennent au système : un second essai n'a plus rien à donner.
        assert_eq!(image.poser(), Err(ErreurPressePapiers::Refuse));
        drop(image);

        let _ouvert = ouvrir_pour_lire().expect("presse-papiers ouvert en 1 s");
        // PNG : les mêmes octets (`GlobalSize` peut arrondir la taille au-dessus).
        let lu_png = lire_format(format_png().expect("format PNG")).expect("format PNG");
        assert!(lu_png.len() >= png.len(), "{} octets", lu_png.len());
        assert_eq!(&lu_png[..png.len()], png.as_slice());

        // CF_DIB : en-tête, puis lignes de bas en haut en BGR.
        let dib = lire_format(CF_DIB).expect("CF_DIB");
        assert!(dib.len() >= TAILLE_ENTETE_DIB + (l * h * 4) as usize);
        // SAFETY : au moins 40 octets lus, lecture non alignée.
        let entete: BITMAPINFOHEADER =
            unsafe { std::ptr::read_unaligned(dib.as_ptr() as *const BITMAPINFOHEADER) };
        assert_eq!(entete.biSize as usize, TAILLE_ENTETE_DIB);
        assert_eq!((entete.biWidth, entete.biHeight), (l as i32, h as i32));
        assert_eq!((entete.biPlanes, entete.biBitCount), (1, 32));
        assert_eq!(entete.biCompression, BI_RGB.0);
        let pixel = |x: u32, y: u32| pixel_dib(&dib, x, y).expect("pixel");
        assert_eq!(pixel(0, 0), [255, 255, 255], "coin blanc");
        assert_eq!(pixel(1, 0), [255, 0, 0]);
        assert_eq!(pixel(2, 0), [0, 255, 0]);
        assert_eq!(pixel(0, 1), [0, 0, 255]);
        assert_eq!(pixel(1, 1), [10, 20, 30]);
        assert_eq!(pixel(2, 1), [200, 100, 50]);
        println!("Presse-papiers : PNG et CF_DIB poses ensemble et relus.");
    }

    #[test]
    fn preparer_refuse_des_pixels_incoherents() {
        let _seul = VERROU_ESSAIS.lock().unwrap_or_else(|e| e.into_inner());
        // Refusées dès la préparation, sans ouvrir le presse-papiers.
        assert_eq!(
            ImagePreparee::preparer(&[1, 2, 3], 2, 2, &[255; 15]).err(),
            Some(ErreurPressePapiers::Refuse)
        );
        assert_eq!(
            ImagePreparee::preparer(&[], 1, 1, &[255; 4]).err(),
            Some(ErreurPressePapiers::Refuse)
        );
    }
}
