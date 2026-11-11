//! Validation des pixels et encodage PNG (fonctions pures).

use std::borrow::Cow;

use crate::systeme::gel::Pixels;

/// Cause donnee quand les pixels recus sont incoherents.
pub(super) const CAUSE_PIXELS: &str = "La capture est vide ou abîmée.";

/// Verifie que les dimensions sont non nulles et que la taille du tampon
/// correspond (4 octets par pixel).
pub(super) fn valider(pixels: &Pixels) -> Result<(), String> {
    if pixels.largeur == 0 || pixels.hauteur == 0 {
        return Err(CAUSE_PIXELS.to_string());
    }
    let attendu = (pixels.largeur as u64)
        .checked_mul(pixels.hauteur as u64)
        .and_then(|n| n.checked_mul(4));
    match attendu {
        Some(n) if n == pixels.rgba.len() as u64 => Ok(()),
        _ => Err(CAUSE_PIXELS.to_string()),
    }
}

/// Les pixels avec alpha a 255 partout. Ne copie que si besoin.
pub(super) fn opaques(rgba: &[u8]) -> Cow<'_, [u8]> {
    if rgba.chunks_exact(4).all(|p| p[3] == 255) {
        Cow::Borrowed(rgba)
    } else {
        let mut copie = rgba.to_vec();
        for p in copie.chunks_exact_mut(4) {
            p[3] = 255;
        }
        Cow::Owned(copie)
    }
}

/// Encode en PNG RGBA 8 bits, sans metadonnees, compression rapide,
/// alpha force a 255.
pub fn encoder_png(pixels: &Pixels) -> Result<Vec<u8>, String> {
    valider(pixels)?;
    encoder_opaques(pixels.largeur, pixels.hauteur, &opaques(&pixels.rgba))
}

/// Encode des pixels deja valides et deja opaques.
pub(super) fn encoder_opaques(largeur: u32, hauteur: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut sortie = Vec::new();
    let mut encodeur = ::png::Encoder::new(&mut sortie, largeur, hauteur);
    encodeur.set_color(::png::ColorType::Rgba);
    encodeur.set_depth(::png::BitDepth::Eight);
    encodeur.set_compression(::png::Compression::Fast);

    let mut ecrivain = encodeur.write_header().map_err(|_| CAUSE_ENCODAGE)?;
    ecrivain
        .write_image_data(rgba)
        .map_err(|_| CAUSE_ENCODAGE)?;
    ecrivain.finish().map_err(|_| CAUSE_ENCODAGE)?;
    Ok(sortie)
}

/// Cause donnee si l'encodage echoue (jamais le message de la crate).
const CAUSE_ENCODAGE: &str = "L'image n'a pas pu être convertie en PNG.";
