//! L'historique : le dossier « Recents » lu, et ses apercus (contrat 0.5, §3.1).
//!
//! Le dossier fait foi, sans index. Une entree est un fichier au format exact des noms de
//! « Recents » ; un autre nom n'est jamais montre ni efface. Les apercus vivent dans
//! `<local>/Apercus`, a cote de « Recents », sous le meme nom que leur capture.
//!
//! Fonctions bloquantes (disque, decodage) : elles sont faites pour etre appelees hors du
//! fil principal.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{BufReader, ErrorKind, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDateTime, Utc};

use super::fichier::ecrire_sans_ecraser;
use super::png;
use super::recents::cle_recent;
use crate::systeme::gel::Pixels;

/// Nom du dossier des apercus, a cote de « Recents » (`<local>/Apercus`).
pub(super) const SOUS_DOSSIER_APERCUS: &str = "Apercus";

/// Octets lus pour connaitre la taille : signature (8), longueur et type du premier bloc
/// (8), contenu de IHDR (13), son CRC (4).
const ENTETE: usize = 33;

/// Signature de tout fichier PNG.
const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// Le dernier bloc de tout PNG complet : longueur 0, `IEND`, son CRC.
const FIN_PNG: [u8; 12] = [0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82];

/// Memoire au plus pour relire une capture (8 ecrans 4K cote a cote tiennent dedans).
const OCTETS_MAX: usize = 1 << 30;

/// Cause quand une capture ne peut pas etre relue (jamais de chemin ni de message de crate).
const CAUSE_LECTURE: &str = "La capture est illisible.";

/// Une capture de « Recents ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntreeRecente {
    pub nom: String,
    /// Lu dans le nom (UTC, millisecondes).
    pub horodatage: DateTime<Utc>,
    /// Lues dans l'en-tete IHDR.
    pub largeur: u32,
    pub hauteur: u32,
}

/// `<local>/Apercus`, a cote du dossier « Recents » donne. `None` sans dossier parent.
pub(super) fn dossier_apercus_de(dossier_recents: &Path) -> Option<PathBuf> {
    let parent = dossier_recents.parent()?;
    if parent.as_os_str().is_empty() {
        return None;
    }
    Some(parent.join(SOUS_DOSSIER_APERCUS))
}

/// L'heure portee par un nom au format exact (`AAAAMMJJ-HHMMSS-mmm[_n].png`, UTC).
/// `None` pour un autre nom ou une date impossible (13e mois…).
pub(super) fn horodatage_du_nom(nom: &str) -> Option<DateTime<Utc>> {
    let (base, _) = cle_recent(nom)?;
    NaiveDateTime::parse_from_str(base, "%Y%m%d-%H%M%S-%3f")
        .ok()
        .map(|t| t.and_utc())
}

/// CRC-32 des PNG (polynome reflechi 0xEDB88320), bit a bit : il ne sert qu'a 17 octets.
fn crc32(octets: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &o in octets {
        crc ^= u32::from(o);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Largeur et hauteur d'un en-tete PNG RGBA 8 bits (33 premiers octets), sinon `None` :
/// signature, bloc IHDR de 13 octets et son CRC, taille non nulle, profondeur 8, couleur
/// RGBA, sans entrelacement (ce que `sortir` ecrit).
pub(super) fn taille_ihdr(entete: &[u8]) -> Option<(u32, u32)> {
    let entete = entete.get(..ENTETE)?;
    if entete[..8] != SIGNATURE || entete[8..12] != [0, 0, 0, 13] || &entete[12..16] != b"IHDR" {
        return None;
    }
    let lu = u32::from_be_bytes([entete[29], entete[30], entete[31], entete[32]]);
    if crc32(&entete[12..29]) != lu {
        return None;
    }
    let mot =
        |i: usize| u32::from_be_bytes([entete[i], entete[i + 1], entete[i + 2], entete[i + 3]]);
    let (largeur, hauteur) = (mot(16), mot(20));
    // Profondeur 8, couleur 6 (RGBA), compression 0, filtre 0, sans entrelacement.
    if largeur == 0 || hauteur == 0 || entete[24..29] != [8, 6, 0, 0, 0] {
        return None;
    }
    Some((largeur, hauteur))
}

/// Taille d'un PNG complet sur le disque : en-tete lisible et bloc de fin present (un
/// fichier tronque, en cours d'ecriture ou d'un autre format est ecarte).
fn taille_png(chemin: &Path) -> Option<(u32, u32)> {
    let mut f = File::open(chemin).ok()?;
    let mut entete = [0u8; ENTETE];
    f.read_exact(&mut entete).ok()?;
    let taille = taille_ihdr(&entete)?;
    let longueur = f.metadata().ok()?.len();
    if longueur < (ENTETE + FIN_PNG.len()) as u64 {
        return None;
    }
    f.seek(SeekFrom::End(-(FIN_PNG.len() as i64))).ok()?;
    let mut fin = [0u8; FIN_PNG.len()];
    f.read_exact(&mut fin).ok()?;
    (fin == FIN_PNG).then_some(taille)
}

/// Les noms des fichiers ordinaires de `dossier` qui ont le format exact (sans lien
/// symbolique ni sous-dossier). Dossier absent ou illisible : aucun.
fn noms_au_format(dossier: &Path) -> Vec<String> {
    if dossier.as_os_str().is_empty() {
        return Vec::new();
    }
    let Ok(entrees) = fs::read_dir(dossier) else {
        return Vec::new();
    };
    entrees
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| cle_recent(n).is_some())
        .collect()
}

/// Les entrees du dossier, la plus recente d'abord (ordre des noms, celui de `cle_recent`) ;
/// un fichier illisible, tronque ou dont le nom porte une date impossible est ecarte.
pub fn lister_recents(dossier: &Path) -> Vec<EntreeRecente> {
    let mut liste: Vec<EntreeRecente> = noms_au_format(dossier)
        .into_iter()
        .filter_map(|nom| {
            let horodatage = horodatage_du_nom(&nom)?;
            let (largeur, hauteur) = taille_png(&dossier.join(&nom))?;
            Some(EntreeRecente {
                nom,
                horodatage,
                largeur,
                hauteur,
            })
        })
        .collect();
    liste.sort_by(|a, b| {
        let cle = |e: &EntreeRecente| cle_recent(&e.nom).map(|(b, n)| (b.to_owned(), n));
        cle(b).cmp(&cle(a))
    });
    liste
}

/// Le chemin de `nom` dans `dossier` si `nom` a le format exact, sinon `None`.
pub fn chemin_recent(dossier: &Path, nom: &str) -> Option<PathBuf> {
    (!dossier.as_os_str().is_empty() && cle_recent(nom).is_some()).then(|| dossier.join(nom))
}

/// Relit un PNG de « Recents » (RGBA 8 bits, ce que `sortir` ecrit) ; le booleen dit s'il a
/// un alpha < 255. Tout autre PNG, tronque ou abime : une erreur, jamais une panique.
pub fn lire_recent(chemin: &Path) -> Result<(Pixels, bool), String> {
    let f = File::open(chemin).map_err(|_| CAUSE_LECTURE.to_string())?;
    let limites = ::png::Limits { bytes: OCTETS_MAX };
    let decodeur = ::png::Decoder::new_with_limits(BufReader::new(f), limites);
    let mut lecteur = decodeur.read_info().map_err(|_| CAUSE_LECTURE)?;
    let taille = lecteur.output_buffer_size().ok_or(CAUSE_LECTURE)?;
    if taille > OCTETS_MAX {
        return Err(CAUSE_LECTURE.into());
    }
    let mut rgba = vec![0u8; taille];
    let info = lecteur.next_frame(&mut rgba).map_err(|_| CAUSE_LECTURE)?;
    if info.color_type != ::png::ColorType::Rgba || info.bit_depth != ::png::BitDepth::Eight {
        return Err(CAUSE_LECTURE.into());
    }
    rgba.truncate(info.buffer_size());
    let pixels = Pixels {
        largeur: info.width,
        hauteur: info.height,
        rgba,
    };
    png::valider(&pixels).map_err(|_| CAUSE_LECTURE)?;
    let transparence = pixels.rgba.chunks_exact(4).any(|p| p[3] < 255);
    Ok((pixels, transparence))
}

/// Ecrit l'apercu de `nom` dans `dossier_apercus` (cree si besoin), alpha garde ; un apercu
/// deja la est remplace. `nom` doit avoir le format exact.
pub fn ranger_apercu(dossier_apercus: &Path, nom: &str, apercu: &Pixels) -> Result<(), String> {
    let chemin = chemin_apercu(dossier_apercus, nom).ok_or("nom refuse")?;
    png::valider(apercu)?;
    let octets = png::encoder_rgba(apercu.largeur, apercu.hauteur, &apercu.rgba)?;
    match fs::remove_file(&chemin) {
        Err(e) if e.kind() != ErrorKind::NotFound => return Err("apercu tenu".into()),
        _ => {}
    }
    // Temporaire du meme dossier puis lien dur : jamais un apercu a moitie ecrit.
    match ecrire_sans_ecraser(dossier_apercus, "", &octets, |_| chemin.clone()) {
        Ok(_) => Ok(()),
        // Fait en meme temps par un autre fil (capture et ouverture de l'historique).
        Err(_) if taille_png(&chemin).is_some() => Ok(()),
        Err(cause) => Err(cause),
    }
}

/// Le chemin de l'apercu de `nom` s'il a le format exact, sinon `None`.
pub fn chemin_apercu(dossier_apercus: &Path, nom: &str) -> Option<PathBuf> {
    chemin_recent(dossier_apercus, nom)
}

/// Efface un fichier ; deja absent compte comme fait.
fn effacer(chemin: &Path) -> Result<(), ErrorKind> {
    match fs::remove_file(chemin) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.kind()),
    }
}

/// Efface le PNG de « Recents » et son apercu, `nom` au format exact seulement. Jamais le
/// fichier enregistre ni le presse-papiers (R14). Deja absent : fait. L'echec de l'apercu
/// seul n'est pas un echec (il partira au prochain nettoyage).
pub fn supprimer_recent(
    dossier_recents: &Path,
    dossier_apercus: &Path,
    nom: &str,
) -> Result<(), String> {
    let recent = chemin_recent(dossier_recents, nom).ok_or("nom refuse")?;
    if recent.is_dir() {
        return Err("pas un fichier".into());
    }
    effacer(&recent).map_err(|_| "suppression refusee".to_string())?;
    if let Some(apercu) = chemin_apercu(dossier_apercus, nom) {
        if !apercu.is_dir() {
            let _ = effacer(&apercu);
        }
    }
    Ok(())
}

/// Ce que « Tout effacer » a fait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BilanVidage {
    pub effaces: usize,
    pub echecs: usize,
}

impl BilanVidage {
    fn compter(&mut self, resultat: Result<(), ErrorKind>) {
        match resultat {
            Ok(()) => self.effaces += 1,
            Err(_) => self.echecs += 1,
        }
    }
}

/// Les noms des fichiers ordinaires `.png` (toute casse) de `dossier`.
fn png_du_dossier(dossier: &Path) -> Vec<String> {
    if dossier.as_os_str().is_empty() {
        return Vec::new();
    }
    let Ok(entrees) = fs::read_dir(dossier) else {
        return Vec::new();
    };
    entrees
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| {
            Path::new(n)
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("png"))
        })
        .collect()
}

/// Efface tout ce qui a le format de « Recents » et des apercus, et les `.png` de
/// `dossier_glisser`. Rien d'autre : ni un autre nom, ni un sous-dossier, ni un lien
/// symbolique. `effaces` compte les captures de « Recents » parties ; `echecs`, tout
/// fichier reste.
pub fn vider(
    dossier_recents: &Path,
    dossier_apercus: &Path,
    dossier_glisser: &Path,
) -> BilanVidage {
    let mut bilan = BilanVidage::default();
    for nom in noms_au_format(dossier_recents) {
        bilan.compter(effacer(&dossier_recents.join(nom)));
    }
    let mut reste = BilanVidage::default();
    for nom in noms_au_format(dossier_apercus) {
        reste.compter(effacer(&dossier_apercus.join(nom)));
    }
    for nom in png_du_dossier(dossier_glisser) {
        reste.compter(effacer(&dossier_glisser.join(nom)));
    }
    bilan.echecs += reste.echecs;
    bilan
}

/// Apres le nettoyage de « Recents » : efface les apercus des noms `partis`, et tout apercu
/// au format exact dont la capture n'est plus dans « Recents » (supprimee par la vignette,
/// a la main, ou apercu fait pendant une suppression). Les apercus sont lus AVANT
/// « Recents » : un apercu se fait toujours apres sa capture, il ne peut donc pas etre vu
/// sans elle s'il lui appartient.
pub(super) fn nettoyer_apercus(dossier_recents: &Path, partis: &[String]) {
    let Some(dossier_apercus) = dossier_apercus_de(dossier_recents) else {
        return;
    };
    for nom in partis {
        if let Some(apercu) = chemin_apercu(&dossier_apercus, nom) {
            let _ = effacer(&apercu);
        }
    }
    let apercus = noms_au_format(&dossier_apercus);
    if apercus.is_empty() {
        return;
    }
    let presents: BTreeSet<String> = noms_au_format(dossier_recents).into_iter().collect();
    if presents.is_empty() && fs::read_dir(dossier_recents).is_err() {
        // « Recents » illisible : rien ne prouve que les apercus sont orphelins.
        return;
    }
    for nom in apercus.iter().filter(|n| !presents.contains(*n)) {
        let _ = effacer(&dossier_apercus.join(nom));
    }
}
