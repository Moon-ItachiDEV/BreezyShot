//! Le fichier « .breezy » (0.7.6, Pro) : une capture et ses annotations, gardées séparément
//! pour qu'on puisse rouvrir l'éditeur plus tard et tout modifier. Ce module est pur
//! (octets contre octets, aucun chemin, aucun journal) ; l'écriture sur le disque est dans
//! `fichier.rs` (`enregistrer_breezy_a`) et la lecture dans `lire_breezy_a`.
//!
//! Format (version 1) : une archive ZIP, entrées « stockées » (sans compression : le PNG
//! l'est déjà, le JSON est petit), sans ZIP64, sans chiffrement, écrite et lue ici, à la main :
//!
//! - `breezy.json` : le manifeste `{ "format": "breezy", "version": 1, "app": "0.7.6",
//!   "largeur": L, "hauteur": H, "transparence": bool, "cree": "AAAA-MM-JJTHH:MM:SS±HH:MM" }` ;
//! - `image.png` : les pixels d'origine (RGBA 8 bits, alpha gardé), sans annotation ;
//! - `annotations.json` : le document de l'éditeur (`interface/editeur/document.js`), opaque
//!   pour le cœur : seule la page le lit et le valide.
//!
//! Un fichier est une entrée non fiable : taille du fichier, de chaque entrée et de l'image
//! bornées avant toute allocation, empreinte CRC-32 vérifiée, noms en double refusés,
//! champs inconnus du manifeste ignorés, version plus récente refusée avec sa propre
//! cause. Les causes rendues ne contiennent jamais de chemin ni de contenu.

use std::io::Cursor;

use crate::systeme::gel::Pixels;

/// La version du format que ce code écrit, et la plus récente qu'il sait lire.
pub const VERSION_FORMAT: u32 = 1;
/// L'extension du fichier, sans point.
pub const EXTENSION: &str = "breezy";

/// Un fichier plus gros que cela n'est jamais lu.
pub const FICHIER_OCTETS_MAX: u64 = 256 << 20;
/// Les annotations (JSON) au plus.
pub const ANNOTATIONS_OCTETS_MAX: usize = 16 << 20;
/// Le manifeste au plus.
const MANIFESTE_OCTETS_MAX: usize = 16 << 10;
/// L'image PNG au plus (octets de l'entrée).
const IMAGE_OCTETS_MAX: usize = 200 << 20;
/// Côté et pixels maximaux d'une image (mêmes bornes que `application::editeur`, qu'un test
/// de l'Application garde égales).
pub const COTE_MAX: u32 = 16_384;
pub const PIXELS_MAX: u64 = 25_000_000;
/// Entrées lues au plus dans l'archive (il y en a trois).
const ENTREES_MAX: usize = 64;

const NOM_MANIFESTE: &str = "breezy.json";
const NOM_IMAGE: &str = "image.png";
const NOM_ANNOTATIONS: &str = "annotations.json";

/// Ce que contient un fichier .breezy lu et vérifié.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContenuBreezy {
    pub pixels: Pixels,
    pub transparence: bool,
    /// Le JSON des annotations, tel qu'il est dans le fichier (UTF-8 valide, objet JSON
    /// vérifié syntaxiquement seulement ; la page le valide).
    pub annotations: String,
}

/// Pourquoi un fichier n'a pas pu être lu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErreurBreezy {
    /// Pas une archive BreezyShot, tronquée, abîmée ou hors bornes.
    Abime,
    /// Fait par une version plus récente de BreezyShot.
    PlusRecent,
    /// Plus gros que ce que l'éditeur accepte.
    TropGros,
}

impl ErreurBreezy {
    /// Le code du journal (jamais de chemin).
    pub fn code(self) -> &'static str {
        match self {
            ErreurBreezy::Abime => "abime",
            ErreurBreezy::PlusRecent => "plus_recent",
            ErreurBreezy::TropGros => "trop_gros",
        }
    }
}

// ---------- écriture ----------

/// Le fichier .breezy de `pixels` (annotation absente) et `annotations` (JSON de la page).
/// `version_app` et `cree` vont au manifeste (informations, jamais lues pour décider).
pub fn ecrire(
    pixels: &Pixels,
    transparence: bool,
    annotations: &[u8],
    version_app: &str,
    cree: &str,
) -> Result<Vec<u8>, String> {
    super::png::valider(pixels)?;
    if !taille_permise(pixels.largeur, pixels.hauteur) {
        return Err(CAUSE_TROP_GROS.into());
    }
    if annotations.len() > ANNOTATIONS_OCTETS_MAX {
        return Err(CAUSE_TROP_GROS.into());
    }
    let png = if transparence {
        super::png::encoder_rgba(pixels.largeur, pixels.hauteur, &pixels.rgba)?
    } else {
        super::png::encoder_rgba(
            pixels.largeur,
            pixels.hauteur,
            &super::png::opaques(&pixels.rgba),
        )?
    };
    let manifeste = serde_json::json!({
        "format": "breezy",
        "version": VERSION_FORMAT,
        "app": version_app,
        "largeur": pixels.largeur,
        "hauteur": pixels.hauteur,
        "transparence": transparence,
        "cree": cree,
    })
    .to_string();
    zip_ecrire(&[
        (NOM_MANIFESTE, manifeste.as_bytes()),
        (NOM_IMAGE, &png),
        (NOM_ANNOTATIONS, annotations),
    ])
}

const CAUSE_TROP_GROS: &str = "La capture est trop grande pour un fichier .breezy.";

pub fn taille_permise(largeur: u32, hauteur: u32) -> bool {
    largeur > 0
        && hauteur > 0
        && largeur <= COTE_MAX
        && hauteur <= COTE_MAX
        && u64::from(largeur) * u64::from(hauteur) <= PIXELS_MAX
}

// ---------- lecture ----------

/// Lit et vérifie un fichier .breezy. Jamais de panique, quelle que soit l'entrée.
pub fn lire(octets: &[u8]) -> Result<ContenuBreezy, ErreurBreezy> {
    if octets.len() as u64 > FICHIER_OCTETS_MAX {
        return Err(ErreurBreezy::TropGros);
    }
    let entrees = zip_lire(octets)?;
    let trouve = |nom: &str| {
        let mut ici = entrees.iter().filter(|(n, _)| n == nom);
        let premiere = ici.next().map(|(_, d)| *d);
        // Un nom en double : l'archive est ambiguë, on la refuse.
        (premiere, ici.next().is_some())
    };
    let (manifeste, double_m) = trouve(NOM_MANIFESTE);
    let (image, double_i) = trouve(NOM_IMAGE);
    let (annotations, double_a) = trouve(NOM_ANNOTATIONS);
    if double_m || double_i || double_a {
        return Err(ErreurBreezy::Abime);
    }
    let manifeste = manifeste.ok_or(ErreurBreezy::Abime)?;
    let image = image.ok_or(ErreurBreezy::Abime)?;
    let annotations = annotations.ok_or(ErreurBreezy::Abime)?;
    if manifeste.len() > MANIFESTE_OCTETS_MAX
        || image.len() > IMAGE_OCTETS_MAX
        || annotations.len() > ANNOTATIONS_OCTETS_MAX
    {
        return Err(ErreurBreezy::TropGros);
    }

    let manifeste: serde_json::Value =
        serde_json::from_slice(manifeste).map_err(|_| ErreurBreezy::Abime)?;
    if manifeste.get("format").and_then(|v| v.as_str()) != Some("breezy") {
        return Err(ErreurBreezy::Abime);
    }
    let version = manifeste
        .get("version")
        .and_then(|v| v.as_u64())
        .ok_or(ErreurBreezy::Abime)?;
    if version == 0 {
        return Err(ErreurBreezy::Abime);
    }
    if version > u64::from(VERSION_FORMAT) {
        return Err(ErreurBreezy::PlusRecent);
    }

    let pixels = decoder_rgba8(Cursor::new(image))?;
    // La taille du manifeste, si elle y est, doit être celle de l'image : l'image fait foi.
    let dit = |nom: &str| manifeste.get(nom).and_then(|v| v.as_u64());
    if dit("largeur").is_some_and(|l| l != u64::from(pixels.largeur))
        || dit("hauteur").is_some_and(|h| h != u64::from(pixels.hauteur))
    {
        return Err(ErreurBreezy::Abime);
    }
    let transparence = manifeste
        .get("transparence")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
        && pixels.rgba.chunks_exact(4).any(|p| p[3] < 255);

    let annotations = std::str::from_utf8(annotations)
        .map_err(|_| ErreurBreezy::Abime)?
        .to_string();
    // Un objet JSON : sinon la page n'aurait rien à lire. Le contenu, lui, est jugé par la page.
    match serde_json::from_str::<serde_json::Value>(&annotations) {
        Ok(serde_json::Value::Object(_)) => {}
        _ => return Err(ErreurBreezy::Abime),
    }
    Ok(ContenuBreezy {
        pixels,
        transparence,
        annotations,
    })
}

/// Décode un PNG RGBA 8 bits (ce que `ecrire` produit) ; l'en-tête est jugé avant toute
/// allocation. Tout autre PNG, tronqué ou abîmé : `Abime` ; trop grand : `TropGros`.
fn decoder_rgba8(lecteur: Cursor<&[u8]>) -> Result<Pixels, ErreurBreezy> {
    let limites = ::png::Limits {
        bytes: (PIXELS_MAX as usize) * 4 + (1 << 20),
    };
    let decodeur = ::png::Decoder::new_with_limits(lecteur, limites);
    let mut lecteur = decodeur.read_info().map_err(|_| ErreurBreezy::Abime)?;
    let entete = lecteur.info();
    if entete.color_type != ::png::ColorType::Rgba || entete.bit_depth != ::png::BitDepth::Eight {
        return Err(ErreurBreezy::Abime);
    }
    if !taille_permise(entete.width, entete.height) {
        return Err(ErreurBreezy::TropGros);
    }
    let taille = lecteur.output_buffer_size().ok_or(ErreurBreezy::Abime)?;
    if taille as u64 != u64::from(entete.width) * u64::from(entete.height) * 4 {
        return Err(ErreurBreezy::Abime);
    }
    let mut rgba = vec![0u8; taille];
    let info = lecteur
        .next_frame(&mut rgba)
        .map_err(|_| ErreurBreezy::Abime)?;
    if info.color_type != ::png::ColorType::Rgba
        || info.bit_depth != ::png::BitDepth::Eight
        || info.buffer_size() != taille
    {
        return Err(ErreurBreezy::Abime);
    }
    Ok(Pixels {
        largeur: info.width,
        hauteur: info.height,
        rgba,
    })
}

// ---------- l'archive ZIP (entrées stockées) ----------

const SIG_LOCALE: u32 = 0x0403_4b50;
const SIG_CENTRALE: u32 = 0x0201_4b50;
const SIG_FIN: u32 = 0x0605_4b50;
/// 1980-01-01 00:00:00 : la date d'une entrée n'a pas d'importance (reproductible).
const DOS_HEURE: u16 = 0;
const DOS_DATE: u16 = 0x0021;
/// Bit 11 : noms en UTF-8.
const DRAPEAU_UTF8: u16 = 0x0800;

fn zip_ecrire(entrees: &[(&str, &[u8])]) -> Result<Vec<u8>, String> {
    let mut sortie: Vec<u8> = Vec::new();
    let mut centrale: Vec<u8> = Vec::new();
    for (nom, donnees) in entrees {
        let taille = u32::try_from(donnees.len()).map_err(|_| CAUSE_TROP_GROS.to_string())?;
        let decalage = u32::try_from(sortie.len()).map_err(|_| CAUSE_TROP_GROS.to_string())?;
        let crc = crc32(donnees);
        let nom_octets = nom.as_bytes();
        let nom_long = nom_octets.len() as u16;
        // En-tête local.
        sortie.extend_from_slice(&SIG_LOCALE.to_le_bytes());
        sortie.extend_from_slice(&20u16.to_le_bytes()); // version nécessaire
        sortie.extend_from_slice(&DRAPEAU_UTF8.to_le_bytes());
        sortie.extend_from_slice(&0u16.to_le_bytes()); // méthode : stockée
        sortie.extend_from_slice(&DOS_HEURE.to_le_bytes());
        sortie.extend_from_slice(&DOS_DATE.to_le_bytes());
        sortie.extend_from_slice(&crc.to_le_bytes());
        sortie.extend_from_slice(&taille.to_le_bytes());
        sortie.extend_from_slice(&taille.to_le_bytes());
        sortie.extend_from_slice(&nom_long.to_le_bytes());
        sortie.extend_from_slice(&0u16.to_le_bytes()); // extra
        sortie.extend_from_slice(nom_octets);
        sortie.extend_from_slice(donnees);
        // Entrée du répertoire central.
        centrale.extend_from_slice(&SIG_CENTRALE.to_le_bytes());
        centrale.extend_from_slice(&20u16.to_le_bytes()); // version faite par
        centrale.extend_from_slice(&20u16.to_le_bytes()); // version nécessaire
        centrale.extend_from_slice(&DRAPEAU_UTF8.to_le_bytes());
        centrale.extend_from_slice(&0u16.to_le_bytes());
        centrale.extend_from_slice(&DOS_HEURE.to_le_bytes());
        centrale.extend_from_slice(&DOS_DATE.to_le_bytes());
        centrale.extend_from_slice(&crc.to_le_bytes());
        centrale.extend_from_slice(&taille.to_le_bytes());
        centrale.extend_from_slice(&taille.to_le_bytes());
        centrale.extend_from_slice(&nom_long.to_le_bytes());
        centrale.extend_from_slice(&0u16.to_le_bytes()); // extra
        centrale.extend_from_slice(&0u16.to_le_bytes()); // commentaire
        centrale.extend_from_slice(&0u16.to_le_bytes()); // disque
        centrale.extend_from_slice(&0u16.to_le_bytes()); // attributs internes
        centrale.extend_from_slice(&0u32.to_le_bytes()); // attributs externes
        centrale.extend_from_slice(&decalage.to_le_bytes());
        centrale.extend_from_slice(nom_octets);
    }
    let debut_centrale = u32::try_from(sortie.len()).map_err(|_| CAUSE_TROP_GROS.to_string())?;
    let taille_centrale = u32::try_from(centrale.len()).map_err(|_| CAUSE_TROP_GROS.to_string())?;
    sortie.extend_from_slice(&centrale);
    let n = entrees.len() as u16;
    sortie.extend_from_slice(&SIG_FIN.to_le_bytes());
    sortie.extend_from_slice(&0u16.to_le_bytes()); // ce disque
    sortie.extend_from_slice(&0u16.to_le_bytes()); // disque du répertoire
    sortie.extend_from_slice(&n.to_le_bytes());
    sortie.extend_from_slice(&n.to_le_bytes());
    sortie.extend_from_slice(&taille_centrale.to_le_bytes());
    sortie.extend_from_slice(&debut_centrale.to_le_bytes());
    sortie.extend_from_slice(&0u16.to_le_bytes()); // commentaire
    Ok(sortie)
}

fn u16_a(octets: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        octets.get(i..i.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn u32_a(octets: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        octets.get(i..i.checked_add(4)?)?.try_into().ok()?,
    ))
}

/// Les entrées (nom, données) d'une archive ; rien n'est copié. Seules les entrées stockées,
/// non chiffrées, d'une archive sans ZIP64 et sans commentaire trop long sont lues.
fn zip_lire(octets: &[u8]) -> Result<Vec<(String, &[u8])>, ErreurBreezy> {
    // Fin du répertoire central : dans les 22 derniers octets, ou plus loin si l'archive
    // porte un commentaire (65 535 octets au plus).
    let loin = octets.len().saturating_sub(22 + 65_535);
    let mut fin = None;
    if octets.len() >= 22 {
        let mut i = octets.len() - 22;
        loop {
            if u32_a(octets, i) == Some(SIG_FIN) {
                fin = Some(i);
                break;
            }
            if i == loin {
                break;
            }
            i -= 1;
        }
    }
    let fin = fin.ok_or(ErreurBreezy::Abime)?;
    let entrees = usize::from(u16_a(octets, fin + 10).ok_or(ErreurBreezy::Abime)?);
    let taille_centrale = u32_a(octets, fin + 12).ok_or(ErreurBreezy::Abime)? as usize;
    let debut_centrale = u32_a(octets, fin + 16).ok_or(ErreurBreezy::Abime)? as usize;
    // Ni disque multiple, ni ZIP64 (compteurs saturés), ni trop d'entrées.
    if u16_a(octets, fin + 4) != Some(0)
        || u16_a(octets, fin + 6) != Some(0)
        || u16_a(octets, fin + 8) != u16_a(octets, fin + 10)
        || entrees == 0xFFFF
        || entrees > ENTREES_MAX
        || taille_centrale == 0xFFFF_FFFF
        || debut_centrale == 0xFFFF_FFFF
    {
        return Err(ErreurBreezy::Abime);
    }
    let fin_centrale = debut_centrale
        .checked_add(taille_centrale)
        .filter(|f| *f <= fin)
        .ok_or(ErreurBreezy::Abime)?;
    let mut resultat = Vec::with_capacity(entrees);
    let mut p = debut_centrale;
    for _ in 0..entrees {
        if u32_a(octets, p) != Some(SIG_CENTRALE) {
            return Err(ErreurBreezy::Abime);
        }
        let drapeaux = u16_a(octets, p + 8).ok_or(ErreurBreezy::Abime)?;
        let methode = u16_a(octets, p + 10).ok_or(ErreurBreezy::Abime)?;
        let crc = u32_a(octets, p + 16).ok_or(ErreurBreezy::Abime)?;
        let compresse = u32_a(octets, p + 20).ok_or(ErreurBreezy::Abime)? as usize;
        let brut = u32_a(octets, p + 24).ok_or(ErreurBreezy::Abime)? as usize;
        let nom_long = usize::from(u16_a(octets, p + 28).ok_or(ErreurBreezy::Abime)?);
        let extra_long = usize::from(u16_a(octets, p + 30).ok_or(ErreurBreezy::Abime)?);
        let commentaire_long = usize::from(u16_a(octets, p + 32).ok_or(ErreurBreezy::Abime)?);
        let decalage = u32_a(octets, p + 42).ok_or(ErreurBreezy::Abime)? as usize;
        let nom_debut = p + 46;
        let nom = octets
            .get(nom_debut..nom_debut.checked_add(nom_long).ok_or(ErreurBreezy::Abime)?)
            .ok_or(ErreurBreezy::Abime)?;
        p = nom_debut
            .checked_add(nom_long)
            .and_then(|x| x.checked_add(extra_long))
            .and_then(|x| x.checked_add(commentaire_long))
            .filter(|x| *x <= fin_centrale)
            .ok_or(ErreurBreezy::Abime)?;
        // Une entrée qu'on ne lit pas (nom inconnu) est ignorée avant tout calcul : un fichier
        // ne peut pas faire recalculer des empreintes sur des données qu'il ne sert pas.
        if ![NOM_MANIFESTE, NOM_IMAGE, NOM_ANNOTATIONS]
            .iter()
            .any(|n| n.as_bytes() == nom)
        {
            continue;
        }
        // Chiffrée (bit 0), ou autre chose que « stockée » : pas une archive BreezyShot.
        if drapeaux & 1 != 0 || methode != 0 || compresse != brut {
            return Err(ErreurBreezy::Abime);
        }
        // L'en-tête local : sa signature, et sa longueur propre (nom et extra).
        if u32_a(octets, decalage) != Some(SIG_LOCALE) {
            return Err(ErreurBreezy::Abime);
        }
        let local_nom = usize::from(u16_a(octets, decalage + 26).ok_or(ErreurBreezy::Abime)?);
        let local_extra = usize::from(u16_a(octets, decalage + 28).ok_or(ErreurBreezy::Abime)?);
        let donnees_debut = decalage
            .checked_add(30)
            .and_then(|x| x.checked_add(local_nom))
            .and_then(|x| x.checked_add(local_extra))
            .ok_or(ErreurBreezy::Abime)?;
        let donnees_fin = donnees_debut.checked_add(brut).ok_or(ErreurBreezy::Abime)?;
        // Les données restent avant le répertoire central.
        if donnees_fin > debut_centrale {
            return Err(ErreurBreezy::Abime);
        }
        let nom = std::str::from_utf8(nom).map_err(|_| ErreurBreezy::Abime)?;
        // Un nom déjà lu est refusé avant tout calcul : un fichier ne peut pas faire
        // recalculer l'empreinte d'une même grosse image par dizaines d'entrées.
        if resultat.iter().any(|(deja, _)| deja == nom) {
            return Err(ErreurBreezy::Abime);
        }
        let donnees = &octets[donnees_debut..donnees_fin];
        if crc32(donnees) != crc {
            return Err(ErreurBreezy::Abime);
        }
        resultat.push((nom.to_string(), donnees));
    }
    Ok(resultat)
}

// ---------- CRC-32 (IEEE, celui du ZIP) ----------

const TABLE_CRC: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
};

fn crc32(donnees: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in donnees {
        c = TABLE_CRC[((c ^ u32::from(b)) & 0xFF) as usize] ^ (c >> 8);
    }
    !c
}

#[cfg(test)]
mod tests;
