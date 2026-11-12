//! Le presse-papiers natif, pour une image avec transparence.
//!
//! Façade commune : `windows.rs` pose deux versions de l'image dans la même ouverture du
//! presse-papiers (le format « PNG », avec son alpha, puis `CF_DIB` sur fond blanc) ;
//! `autre.rs` répond « non disponible » ailleurs, où `sortie` passe par `arboard`.
//! Windows fabrique seul les formats dérivés (`CF_BITMAP`, `CF_DIBV5`).

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Pourquoi l'image n'a pas été posée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErreurPressePapiers {
    /// Une autre app tient le presse-papiers ouvert : un nouvel essai peut réussir.
    Occupe,
    /// Pas de presse-papiers natif sur ce système.
    NonDisponible,
    /// Le système a refusé l'image (mémoire, format, données incohérentes).
    Refuse,
}

/// Vrai si `poser_image` existe sur ce système.
pub fn disponible() -> bool {
    plateforme::DISPONIBLE
}

/// Pose l'image dans le presse-papiers, en remplaçant son contenu : `png` (les octets
/// d'un fichier PNG, avec alpha) sous le format « PNG », et `rgba_sur_blanc` (RGBA,
/// ligne par ligne de haut en bas, déjà composé sur fond blanc) sous `CF_DIB`. Un seul
/// essai, sans attente : `Occupe` si le presse-papiers ne s'ouvre pas. N'importe quel fil.
pub fn poser_image(
    png: &[u8],
    largeur: u32,
    hauteur: u32,
    rgba_sur_blanc: &[u8],
) -> Result<(), ErreurPressePapiers> {
    plateforme::poser_image(png, largeur, hauteur, rgba_sur_blanc)
}

/// Taille de `BITMAPINFOHEADER`, en octets.
pub const TAILLE_ENTETE_DIB: usize = 40;

/// Le contenu d'un `CF_DIB` : un `BITMAPINFOHEADER` (32 bits, `BI_RGB`, hauteur positive :
/// lignes de bas en haut), puis les pixels en bleu, vert, rouge et un quatrième octet
/// sans usage pour `BI_RGB`, mis à 255 (image opaque) pour les apps qui le liraient
/// comme un alpha. `None` si la taille est nulle, trop grande ou ne correspond pas aux
/// pixels.
pub fn contenu_dib(largeur: u32, hauteur: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    if largeur == 0 || hauteur == 0 || largeur > i32::MAX as u32 || hauteur > i32::MAX as u32 {
        return None;
    }
    let ligne = (largeur as usize).checked_mul(4)?;
    let taille_pixels = ligne.checked_mul(hauteur as usize)?;
    if taille_pixels != rgba.len() || taille_pixels > u32::MAX as usize {
        return None;
    }
    let mut dib = Vec::with_capacity(TAILLE_ENTETE_DIB.checked_add(taille_pixels)?);
    dib.extend_from_slice(&(TAILLE_ENTETE_DIB as u32).to_le_bytes()); // biSize
    dib.extend_from_slice(&(largeur as i32).to_le_bytes()); // biWidth
    dib.extend_from_slice(&(hauteur as i32).to_le_bytes()); // biHeight > 0 : de bas en haut
    dib.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    dib.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    dib.extend_from_slice(&0u32.to_le_bytes()); // biCompression = BI_RGB
    dib.extend_from_slice(&(taille_pixels as u32).to_le_bytes()); // biSizeImage
    dib.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
    dib.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
    dib.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
    dib.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant
    for ligne_rgba in rgba.chunks_exact(ligne).rev() {
        for p in ligne_rgba.chunks_exact(4) {
            dib.extend_from_slice(&[p[2], p[1], p[0], 255]);
        }
    }
    Some(dib)
}

/// Pour les tests : un seul test à la fois touche au vrai presse-papiers, dans tout le
/// programme de test (ceux de `sortie` compris).
#[cfg(test)]
pub static VERROU_ESSAIS: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dib_de_bas_en_haut_en_bgr() {
        // 2 × 2 : rouge, vert / bleu, blanc ; alphas ignorés.
        let rgba = [
            255, 0, 0, 0, 0, 255, 0, 17, //
            0, 0, 255, 255, 255, 255, 255, 128,
        ];
        let dib = contenu_dib(2, 2, &rgba).unwrap();
        assert_eq!(dib.len(), TAILLE_ENTETE_DIB + 16);
        let mot = |i: usize| u32::from_le_bytes(dib[i..i + 4].try_into().unwrap());
        let demi = |i: usize| u16::from_le_bytes(dib[i..i + 2].try_into().unwrap());
        assert_eq!(mot(0), 40);
        assert_eq!(mot(4), 2);
        assert_eq!(mot(8), 2, "hauteur positive : de bas en haut");
        assert_eq!(demi(12), 1);
        assert_eq!(demi(14), 32);
        assert_eq!(mot(16), 0, "BI_RGB");
        assert_eq!(mot(20), 16);
        assert!(dib[24..40].iter().all(|o| *o == 0));
        // La ligne du bas d'abord.
        assert_eq!(
            &dib[40..],
            &[
                255, 0, 0, 255, 255, 255, 255, 255, //
                0, 0, 255, 255, 0, 255, 0, 255,
            ]
        );
    }

    #[test]
    fn dib_refuse_les_tailles_incoherentes() {
        assert_eq!(contenu_dib(0, 1, &[]), None);
        assert_eq!(contenu_dib(1, 0, &[]), None);
        assert_eq!(contenu_dib(2, 2, &[0; 15]), None);
        assert_eq!(contenu_dib(2, 2, &[0; 17]), None);
        assert_eq!(contenu_dib(u32::MAX, 1, &[0; 4]), None);
        assert_eq!(contenu_dib(1, u32::MAX, &[0; 4]), None);
        assert!(contenu_dib(1, 1, &[1, 2, 3, 4]).is_some());
    }

    /// Hors Windows : non disponible, sans rien toucher.
    #[test]
    fn non_disponible_ailleurs() {
        if !disponible() {
            assert_eq!(
                poser_image(&[1], 1, 1, &[255; 4]),
                Err(ErreurPressePapiers::NonDisponible)
            );
        }
    }
}
