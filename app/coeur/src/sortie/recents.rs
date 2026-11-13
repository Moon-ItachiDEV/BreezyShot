//! Le dossier « Recents » : noms, ecriture, nettoyage.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, Utc};

use super::fichier::{ecrire_sans_ecraser, existe};
use super::historique::nettoyer_apercus;
use super::nom::nom_affiche;

/// Nombre de captures gardees dans « Recents ».
pub const RECENTS_GARDES: usize = 50;

/// Longueur de la partie horodatee : `AAAAMMJJ-HHMMSS-mmm`.
const LONGUEUR_BASE: usize = 19;

/// Nombre maximal de suffixes « _n » essayes.
const ESSAIS_MAX: u32 = 10_000;

/// `AAAAMMJJ-HHMMSS-mmm`, sans extension, en temps universel (UTC) : l'ordre
/// des noms reste l'ordre du temps, meme au changement d'heure.
pub(super) fn base_recent(maintenant: DateTime<Local>) -> String {
    maintenant
        .with_timezone(&Utc)
        .format("%Y%m%d-%H%M%S-%3f")
        .to_string()
}

/// Premier chemin libre : `base.png`, puis `base_2.png`, `base_3.png`…
pub(super) fn chemin_recent_libre(
    dossier: &Path,
    base: &str,
    existe: impl Fn(&Path) -> bool,
) -> PathBuf {
    let mut chemin = dossier.join(format!("{base}.png"));
    let mut n = 2;
    while existe(&chemin) && n <= ESSAIS_MAX {
        chemin = dossier.join(format!("{base}_{n}.png"));
        n += 1;
    }
    chemin
}

/// Cle de tri d'un nom au format exact `AAAAMMJJ-HHMMSS-mmm[_n].png`.
/// `None` si le nom n'a pas exactement ce format.
pub(super) fn cle_recent(nom: &str) -> Option<(&str, u32)> {
    let tige = nom.strip_suffix(".png")?;
    if tige.len() < LONGUEUR_BASE || !tige.is_char_boundary(LONGUEUR_BASE) {
        return None;
    }
    let (base, reste) = tige.split_at(LONGUEUR_BASE);
    let forme_ok = base.bytes().enumerate().all(|(i, b)| match i {
        8 | 15 => b == b'-',
        _ => b.is_ascii_digit(),
    });
    if !forme_ok {
        return None;
    }
    if reste.is_empty() {
        return Some((base, 1));
    }
    let chiffres = reste.strip_prefix('_')?;
    if chiffres.is_empty()
        || chiffres.len() > 5
        || chiffres.starts_with('0')
        || !chiffres.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let n: u32 = chiffres.parse().ok()?;
    if n < 2 {
        return None;
    }
    Some((base, n))
}

/// Les noms a supprimer pour ne garder que les `garder` plus recents.
/// Les noms qui n'ont pas exactement le format des « Recents » sont ignores.
/// Le resultat va du plus recent au plus ancien.
pub fn recents_a_supprimer(noms: &[String], garder: usize) -> Vec<String> {
    let mut valides: Vec<(&str, u32, &String)> = noms
        .iter()
        .filter_map(|nom| cle_recent(nom).map(|(base, n)| (base, n, nom)))
        .collect();
    // Du plus recent au plus ancien.
    valides.sort_by(|a, b| (b.0, b.1).cmp(&(a.0, a.1)));
    valides
        .into_iter()
        .skip(garder)
        .map(|(_, _, nom)| nom.clone())
        .collect()
}

/// Comme `recents_a_supprimer`, mais le fichier qu'on vient d'ecrire n'est
/// jamais dans le resultat, meme si l'horloge a recule.
pub(super) fn a_supprimer_apres_ecriture(
    noms: &[String],
    neuf: &str,
    garder: usize,
) -> Vec<String> {
    let autres: Vec<String> = noms.iter().filter(|n| *n != neuf).cloned().collect();
    recents_a_supprimer(&autres, garder.saturating_sub(1))
}

/// Ecrit la capture dans « Recents » puis fait le menage (captures au-dela de 50 et leurs
/// apercus, apercus orphelins). Un echec du menage n'est pas un echec de l'ecriture.
pub(super) fn ecrire(
    dossier: &Path,
    maintenant: DateTime<Local>,
    octets: &[u8],
) -> Result<PathBuf, String> {
    let base = base_recent(maintenant);
    let chemin = ecrire_sans_ecraser(dossier, &nom_affiche(dossier), octets, |d| {
        chemin_recent_libre(d, &base, existe)
    })?;
    nettoyer(dossier, &chemin);
    Ok(chemin)
}

fn nettoyer(dossier: &Path, neuf: &Path) {
    let Some(nom_neuf) = neuf.file_name().and_then(|n| n.to_str()) else {
        return;
    };
    let Ok(entrees) = fs::read_dir(dossier) else {
        return;
    };
    let noms: Vec<String> = entrees
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    let partis: Vec<String> = a_supprimer_apres_ecriture(&noms, nom_neuf, RECENTS_GARDES)
        .into_iter()
        .filter(|nom| fs::remove_file(dossier.join(nom)).is_ok())
        .collect();
    // Les apercus des captures parties, et ceux qui n'ont plus de capture (contrat 0.5).
    nettoyer_apercus(dossier, &partis);
}
