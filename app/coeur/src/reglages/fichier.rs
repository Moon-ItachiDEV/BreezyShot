//! Lecture et écriture de `reglages.json` sur le disque. Ce fichier reste mince : les
//! décisions sont dans `lecture` et dans les fonctions pures en bas de ce fichier.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};

use super::constat::{Constat, ErreurEcriture};
use super::lecture::{analyser, duree_valide, interpreter, TAILLE_MAX};
use super::modele::{vers_json, Plateforme, Reglages, VERSION_ACTUELLE};

/// Nom du fichier de réglages dans le dossier donné.
pub const NOM_FICHIER: &str = "reglages.json";
const NOM_TEMPORAIRE: &str = "reglages.json.tmp";
const PREFIXE_ABIME: &str = "reglages.abime-";
const SUFFIXE_ABIME: &str = ".json";
/// Nombre de fichiers abîmés gardés au plus.
const ABIMES_GARDES: usize = 3;

/// Lit les réglages de `dossier/reglages.json`, en les créant, réparant ou mettant à niveau
/// au besoin. Ne panique jamais et ne renvoie jamais d'erreur : au pire, valeurs par défaut
/// et un constat. Un fichier déjà correct n'est ni réécrit ni signalé.
pub fn charger(dossier: &Path, plateforme: Plateforme) -> (Reglages, Vec<Constat>) {
    charger_a(dossier, plateforme, Local::now())
}

pub(super) fn charger_a(
    dossier: &Path,
    plateforme: Plateforme,
    maintenant: DateTime<Local>,
) -> (Reglages, Vec<Constat>) {
    let chemin = dossier.join(NOM_FICHIER);
    let mut constats = Vec::new();
    let octets = match lire_limite(&chemin) {
        Ok(octets) => octets,
        // Un dossier parent qui est un fichier compte comme « absent » : la création
        // échouera et le dira.
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            let reglages = Reglages::par_defaut(plateforme);
            match ecrire(dossier, &reglages) {
                Ok(()) => constats.push(Constat::Cree { chemin }),
                Err(e) => constats.push(Constat::EcritureImpossible {
                    erreur: e.to_string(),
                }),
            }
            return (reglages, constats);
        }
        Err(e) => {
            constats.push(Constat::LectureImpossible {
                erreur: e.to_string(),
            });
            return (Reglages::par_defaut_protege(plateforme), constats);
        }
    };

    match analyser(&octets) {
        Ok(objet) => {
            let lecture = interpreter(objet, plateforme);
            constats.extend(lecture.constats);
            if lecture.a_reecrire {
                if let Err(e) = ecrire(dossier, &lecture.reglages) {
                    constats.push(Constat::EcritureImpossible {
                        erreur: e.to_string(),
                    });
                }
            }
            (lecture.reglages, constats)
        }
        Err(raison) => match mettre_de_cote(dossier, &chemin, maintenant) {
            Ok(sauvegarde) => {
                let reglages = Reglages::par_defaut(plateforme);
                let nom = sauvegarde
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_string();
                constats.push(Constat::Abime { raison, sauvegarde });
                constats.extend(supprimer_vieilles_sauvegardes(dossier, &nom));
                if let Err(e) = ecrire(dossier, &reglages) {
                    constats.push(Constat::EcritureImpossible {
                        erreur: e.to_string(),
                    });
                }
                (reglages, constats)
            }
            // On ne détruit jamais ce que l'utilisateur avait : sans sauvegarde, on n'écrit
            // pas par-dessus, ni maintenant ni plus tard (réglages protégés).
            Err(e) => {
                constats.push(Constat::AbimeNonSauvegarde {
                    raison,
                    erreur: e.to_string(),
                });
                (Reglages::par_defaut_protege(plateforme), constats)
            }
        },
    }
}

/// Écrit les réglages dans `dossier/reglages.json` (dossier créé au besoin) : JSON indenté,
/// UTF-8, fin de ligne finale, par un fichier temporaire renommé ensuite. Refuse une
/// combinaison sans sortie active, une valeur invalide, un fichier en lecture seule et un
/// fichier venu d'une version plus récente. En cas d'échec, l'ancien fichier reste intact.
pub fn ecrire(dossier: &Path, reglages: &Reglages) -> Result<(), ErreurEcriture> {
    let texte = texte_du_fichier(reglages)?;
    fs::create_dir_all(dossier).map_err(|erreur| ErreurEcriture::Disque {
        etape: "creation du dossier",
        erreur,
    })?;
    let chemin = dossier.join(NOM_FICHIER);
    // On ne remplace jamais un fichier présent qu'on ne peut pas lire.
    if let Err(e) = fs::File::open(&chemin).and_then(|mut f| f.read(&mut [0u8; 1])) {
        if e.kind() != io::ErrorKind::NotFound {
            return Err(ErreurEcriture::FichierProtege);
        }
    }
    if let Ok(meta) = fs::metadata(&chemin) {
        if meta.permissions().readonly() {
            return Err(ErreurEcriture::LectureSeule { chemin });
        }
    }
    let temporaire = dossier.join(NOM_TEMPORAIRE);
    let _ = fs::remove_file(&temporaire);
    let resultat = ecrire_temporaire(&temporaire, texte.as_bytes())
        .map_err(|erreur| ErreurEcriture::Disque {
            etape: "ecriture du fichier temporaire",
            erreur,
        })
        .and_then(|()| {
            fs::rename(&temporaire, &chemin).map_err(|erreur| ErreurEcriture::Disque {
                etape: "remplacement du fichier",
                erreur,
            })
        });
    if resultat.is_err() {
        let _ = fs::remove_file(&temporaire);
    }
    resultat
}

/// Lit au plus `TAILLE_MAX + 1` octets : un fichier énorme n'est jamais lu en entier.
fn lire_limite(chemin: &Path) -> io::Result<Vec<u8>> {
    let mut octets = Vec::new();
    fs::File::open(chemin)?
        .take(TAILLE_MAX as u64 + 1)
        .read_to_end(&mut octets)?;
    Ok(octets)
}

fn ecrire_temporaire(chemin: &Path, octets: &[u8]) -> io::Result<()> {
    let mut f = fs::File::create(chemin)?;
    f.write_all(octets)?;
    f.sync_all()
}

/// Vérifie les réglages et donne le texte exact du fichier. Fonction pure.
pub(super) fn texte_du_fichier(reglages: &Reglages) -> Result<String, ErreurEcriture> {
    if reglages.protege {
        return Err(ErreurEcriture::FichierProtege);
    }
    if reglages.version_fichier > VERSION_ACTUELLE {
        return Err(ErreurEcriture::VersionPlusRecente {
            version: reglages.version_fichier,
        });
    }
    valider(reglages)?;
    let valeur = vers_json(reglages).map_err(|champ| ErreurEcriture::ValeurInvalide { champ })?;
    let mut texte = serde_json::to_string_pretty(&valeur).map_err(|e| ErreurEcriture::Disque {
        etape: "mise en forme",
        erreur: io::Error::other(e),
    })?;
    texte.push('\n');
    if texte.len() > TAILLE_MAX {
        return Err(ErreurEcriture::TropGros {
            taille: texte.len(),
        });
    }
    Ok(texte)
}

/// Longueur maximale du modèle de nom, en caractères (un nom de fichier Windows tient en
/// 255 caractères ; au-delà, une page fausse seulement).
pub const MODELE_NOM_MAX: usize = 200;

/// Les contrôles de valeur que fait `ecrire`, sans le disque ni la protection : au moins
/// une sortie active, durée permise, modèle de nom non vide, dossier non vide. Fonction
/// pure, pour refuser un changement avant de l'appliquer.
pub fn valider(reglages: &Reglages) -> Result<(), ErreurEcriture> {
    if !reglages.une_sortie_active() {
        return Err(ErreurEcriture::AucuneSortie);
    }
    if duree_valide(&reglages.vignette.duree_s.into()).is_none() {
        return Err(ErreurEcriture::ValeurInvalide {
            champ: "vignette.duree_s",
        });
    }
    if reglages.modele_nom.trim().is_empty() || reglages.modele_nom.chars().count() > MODELE_NOM_MAX
    {
        return Err(ErreurEcriture::ValeurInvalide {
            champ: "modele_nom",
        });
    }
    if let Some(d) = &reglages.dossier_enregistrement {
        if d.as_os_str().to_string_lossy().trim().is_empty() {
            return Err(ErreurEcriture::ValeurInvalide {
                champ: "dossier_enregistrement",
            });
        }
    }
    Ok(())
}

/// Renomme le fichier abîmé en `reglages.abime-<horodatage>.json` sans écraser une
/// sauvegarde existante. Un fichier en lecture seule n'est pas déplacé.
fn mettre_de_cote(
    dossier: &Path,
    chemin: &Path,
    maintenant: DateTime<Local>,
) -> io::Result<PathBuf> {
    if fs::metadata(chemin)?.permissions().readonly() {
        return Err(io::Error::other("fichier en lecture seule"));
    }
    for n in 1..=99 {
        let cible = dossier.join(nom_sauvegarde(maintenant, n));
        if !cible.exists() {
            fs::rename(chemin, &cible)?;
            return Ok(cible);
        }
    }
    Err(io::Error::other("trop de sauvegardes au meme instant"))
}

/// Ne garde que la sauvegarde `neuve` et les plus récentes des autres (`ABIMES_GARDES` en
/// tout).
fn supprimer_vieilles_sauvegardes(dossier: &Path, neuve: &str) -> Vec<Constat> {
    let noms: Vec<String> = match fs::read_dir(dossier) {
        Ok(entrees) => entrees
            .filter_map(Result::ok)
            .filter_map(|e| e.file_name().into_string().ok())
            .collect(),
        Err(_) => return Vec::new(),
    };
    sauvegardes_a_supprimer(noms, neuve)
        .into_iter()
        .filter_map(|nom| {
            let chemin = dossier.join(nom);
            fs::remove_file(&chemin)
                .ok()
                .map(|()| Constat::AncienneSauvegardeSupprimee { chemin })
        })
        .collect()
}

/// Nom de la `n`-ième sauvegarde faite à `maintenant` (`n` vaut 1 d'habitude), au format
/// `reglages.abime-AAAAMMJJ-HHMMSS-mmm[_n].json`. Fonction pure.
pub(super) fn nom_sauvegarde(maintenant: DateTime<Local>, n: u32) -> String {
    let horodatage = maintenant.format("%Y%m%d-%H%M%S-%3f");
    if n <= 1 {
        format!("{PREFIXE_ABIME}{horodatage}{SUFFIXE_ABIME}")
    } else {
        format!("{PREFIXE_ABIME}{horodatage}_{n}{SUFFIXE_ABIME}")
    }
}

/// Clé de tri d'un nom produit par `nom_sauvegarde` : (horodatage, n). `None` pour tout autre
/// nom, même proche (« … - Copie.json »), qui n'est alors jamais touché. Fonction pure.
pub(super) fn cle_sauvegarde(nom: &str) -> Option<(&str, u32)> {
    let milieu = nom
        .strip_prefix(PREFIXE_ABIME)?
        .strip_suffix(SUFFIXE_ABIME)?;
    let (horodatage, n) = match milieu.split_once('_') {
        None => (milieu, 1),
        Some((h, n)) => {
            if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            (h, n.parse().ok()?)
        }
    };
    // AAAAMMJJ-HHMMSS-mmm : chiffres aux places 0-7, 9-14, 16-18, tirets en 8 et 15.
    let b = horodatage.as_bytes();
    let forme = b.len() == 19
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 15 => *c == b'-',
            _ => c.is_ascii_digit(),
        });
    forme.then_some((horodatage, n))
}

/// Parmi les noms d'un dossier, les sauvegardes de trop. La sauvegarde `neuve` (celle qu'on
/// vient de faire) n'est jamais candidate, même si l'horloge a reculé : on garde elle et les
/// `ABIMES_GARDES - 1` plus récentes des autres. Fonction pure.
pub(super) fn sauvegardes_a_supprimer(noms: Vec<String>, neuve: &str) -> Vec<String> {
    let mut autres: Vec<((String, u32), String)> = noms
        .into_iter()
        .filter(|n| n != neuve)
        .filter_map(|n| {
            let (h, rang) = cle_sauvegarde(&n)?;
            Some(((h.to_string(), rang), n))
        })
        .collect();
    autres.sort();
    let trop = autres.len().saturating_sub(ABIMES_GARDES - 1);
    autres.into_iter().take(trop).map(|(_, n)| n).collect()
}
