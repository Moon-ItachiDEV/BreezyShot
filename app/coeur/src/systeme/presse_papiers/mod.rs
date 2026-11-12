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

/// Vrai si le presse-papiers natif existe sur ce système.
pub fn disponible() -> bool {
    plateforme::DISPONIBLE
}

/// Une image prête à poser : le format « PNG » inscrit, et les deux blocs de mémoire
/// remplis une fois pour toutes (le PNG, et le `CF_DIB` écrit directement dans son bloc).
/// Seul `poser` touche au presse-papiers : il peut être réessayé sans rien refaire tant
/// qu'il répond `Occupe` (rien n'a encore été donné au système). Les blocs non donnés
/// sont libérés à la destruction.
pub struct ImagePreparee {
    interne: plateforme::Preparee,
}

impl ImagePreparee {
    /// Prépare `png` (les octets d'un fichier PNG, avec alpha), pour le format « PNG »,
    /// et `rgba_sur_blanc` (RGBA, ligne par ligne de haut en bas, déjà composé sur fond
    /// blanc), pour `CF_DIB`. `Refuse` si les tailles sont incohérentes ou la mémoire
    /// manque ; `NonDisponible` hors Windows.
    pub fn preparer(
        png: &[u8],
        largeur: u32,
        hauteur: u32,
        rgba_sur_blanc: &[u8],
    ) -> Result<ImagePreparee, ErreurPressePapiers> {
        plateforme::preparer(png, largeur, hauteur, rgba_sur_blanc)
            .map(|interne| ImagePreparee { interne })
    }

    /// Un essai : ouvre le presse-papiers (6 ouvertures au plus, séparées de 5 ms, comme
    /// `arboard`), le vide et y pose les deux versions, le PNG d'abord. `Occupe` si
    /// aucune ouverture ne réussit : rien n'est donné, un nouvel essai est permis. Après
    /// une réussite ou un `Refuse`, les blocs ne sont plus là : un nouvel essai répond
    /// `Refuse`. N'importe quel fil.
    pub fn poser(&mut self) -> Result<(), ErreurPressePapiers> {
        plateforme::poser(&mut self.interne)
    }
}

/// Prépare puis pose, en un seul essai (voir `ImagePreparee`).
pub fn poser_image(
    png: &[u8],
    largeur: u32,
    hauteur: u32,
    rgba_sur_blanc: &[u8],
) -> Result<(), ErreurPressePapiers> {
    ImagePreparee::preparer(png, largeur, hauteur, rgba_sur_blanc)?.poser()
}

/// Taille de `BITMAPINFOHEADER`, en octets.
pub const TAILLE_ENTETE_DIB: usize = 40;

/// Taille du contenu d'un `CF_DIB` pour ces pixels ; `None` si la taille est nulle,
/// trop grande ou ne correspond pas aux pixels.
pub fn taille_dib(largeur: u32, hauteur: u32, rgba: &[u8]) -> Option<usize> {
    if largeur == 0 || hauteur == 0 || largeur > i32::MAX as u32 || hauteur > i32::MAX as u32 {
        return None;
    }
    let taille_pixels = (largeur as usize)
        .checked_mul(4)?
        .checked_mul(hauteur as usize)?;
    if taille_pixels != rgba.len() || taille_pixels > u32::MAX as usize {
        return None;
    }
    TAILLE_ENTETE_DIB.checked_add(taille_pixels)
}

/// Écrit dans `destination` le contenu d'un `CF_DIB` : un `BITMAPINFOHEADER` (32 bits,
/// `BI_RGB`, hauteur positive : lignes de bas en haut), puis les pixels en bleu, vert,
/// rouge et un quatrième octet sans usage pour `BI_RGB`, mis à 255 (image opaque) pour
/// les apps qui le liraient comme un alpha. Faux, sans rien écrire, si `destination` n'a
/// pas exactement `taille_dib` octets.
pub fn ecrire_dib(largeur: u32, hauteur: u32, rgba: &[u8], destination: &mut [u8]) -> bool {
    if taille_dib(largeur, hauteur, rgba) != Some(destination.len()) {
        return false;
    }
    let taille_pixels = rgba.len() as u32;
    let mut entete = Vec::with_capacity(TAILLE_ENTETE_DIB);
    entete.extend_from_slice(&(TAILLE_ENTETE_DIB as u32).to_le_bytes()); // biSize
    entete.extend_from_slice(&(largeur as i32).to_le_bytes()); // biWidth
    entete.extend_from_slice(&(hauteur as i32).to_le_bytes()); // biHeight > 0 : de bas en haut
    entete.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    entete.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    entete.extend_from_slice(&0u32.to_le_bytes()); // biCompression = BI_RGB
    entete.extend_from_slice(&taille_pixels.to_le_bytes()); // biSizeImage
    entete.extend_from_slice(&[0; 16]); // résolutions, biClrUsed, biClrImportant
    let (tete, pixels) = destination.split_at_mut(TAILLE_ENTETE_DIB);
    tete.copy_from_slice(&entete);
    let ligne = largeur as usize * 4;
    for (dest, source) in pixels
        .chunks_exact_mut(ligne)
        .zip(rgba.chunks_exact(ligne).rev())
    {
        for (d, p) in dest.chunks_exact_mut(4).zip(source.chunks_exact(4)) {
            d.copy_from_slice(&[p[2], p[1], p[0], 255]);
        }
    }
    true
}

/// Le contenu d'un `CF_DIB` dans un `Vec` (voir `ecrire_dib`).
pub fn contenu_dib(largeur: u32, hauteur: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut dib = vec![0; taille_dib(largeur, hauteur, rgba)?];
    ecrire_dib(largeur, hauteur, rgba, &mut dib).then_some(dib)
}

/// Pour les tests : la couleur (rouge, vert, bleu) du pixel `(x, y)`, compté du haut,
/// d'un `CF_DIB` 32 bits de bas en haut ; `None` si l'en-tête ne convient pas.
#[cfg(test)]
pub fn pixel_dib(dib: &[u8], x: u32, y: u32) -> Option<[u8; 3]> {
    let mot = |i: usize| Some(i32::from_le_bytes(dib.get(i..i + 4)?.try_into().ok()?));
    let (largeur, hauteur) = (mot(4)?, mot(8)?);
    let bits = u16::from_le_bytes(dib.get(14..16)?.try_into().ok()?);
    if mot(0)? != TAILLE_ENTETE_DIB as i32 || bits != 32 || mot(16)? != 0 {
        return None;
    }
    if largeur <= 0 || hauteur <= 0 || x >= largeur as u32 || y >= hauteur as u32 {
        return None;
    }
    let i = TAILLE_ENTETE_DIB + (((hauteur as u32 - 1 - y) * largeur as u32 + x) * 4) as usize;
    let p = dib.get(i..i + 3)?;
    Some([p[2], p[1], p[0]])
}

/// Pour les tests : le `CF_DIB` du presse-papiers, lu directement (1 s d'essais
/// d'ouverture au plus). `None` hors Windows ou s'il n'y en a pas.
#[cfg(test)]
pub fn lire_dib() -> Option<Vec<u8>> {
    plateforme::lire_dib()
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
        // Destination de la mauvaise taille : rien n'est écrit.
        let mut trop_court = [7u8; 43];
        assert!(!ecrire_dib(1, 1, &[1, 2, 3, 4], &mut trop_court));
        assert!(trop_court.iter().all(|o| *o == 7));
    }

    #[test]
    fn pixel_relu_dans_un_dib() {
        let rgba = [
            255, 0, 0, 0, 0, 255, 0, 17, //
            0, 0, 255, 255, 255, 255, 255, 128,
        ];
        let dib = contenu_dib(2, 2, &rgba).unwrap();
        assert_eq!(pixel_dib(&dib, 0, 0), Some([255, 0, 0]));
        assert_eq!(pixel_dib(&dib, 1, 0), Some([0, 255, 0]));
        assert_eq!(pixel_dib(&dib, 0, 1), Some([0, 0, 255]));
        assert_eq!(pixel_dib(&dib, 1, 1), Some([255, 255, 255]));
        assert_eq!(pixel_dib(&dib, 2, 0), None);
        assert_eq!(pixel_dib(&dib[..30], 0, 0), None);
    }

    /// Hors Windows : non disponible, sans rien toucher.
    #[test]
    fn non_disponible_ailleurs() {
        if !disponible() {
            assert_eq!(
                poser_image(&[1], 1, 1, &[255; 4]),
                Err(ErreurPressePapiers::NonDisponible)
            );
            assert!(lire_dib().is_none());
        }
    }
}
