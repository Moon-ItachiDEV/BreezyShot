//! Les fichiers `.breezy` côté Application (0.7.9, Pro) : décisions pures de l'enregistrement
//! depuis l'éditeur et de la réouverture d'un fichier. Les gestes sont dans
//! `geste_breezy.rs`, le format dans `sortie/breezy.rs`, le document et son dessin dans la
//! page (`interface/editeur/`). Jamais de chemin, de nom de fichier ni de contenu dans les
//! lignes du journal.

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Serialize;

use super::editeur::{self, EtatEditeur};
use crate::fenetres::{Genre, Message};
use crate::sortie::{ErreurBreezy, ANNOTATIONS_OCTETS_MAX};

/// Titre montré par la page quand l'enregistrement échoue.
pub const TITRE_ENREGISTREMENT_IMPOSSIBLE: &str = "Impossible d'enregistrer le .breezy";

/// Pourquoi un enregistrement `.breezy` est refusé (code rendu à la page et au journal).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refus {
    /// Pas le document en cours.
    Id,
    /// Sans licence : Réglages › Licence s'ouvre à la place.
    Pro,
    /// Annotations vides ou au-delà de `ANNOTATIONS_OCTETS_MAX`.
    Taille,
    /// Un export ou un enregistrement `.breezy` est déjà en cours.
    DejaEnCours,
}

impl Refus {
    pub fn code(self) -> &'static str {
        match self {
            Refus::Id => "id",
            Refus::Pro => "pro",
            Refus::Taille => "taille",
            Refus::DejaEnCours => "deja_en_cours",
        }
    }
}

/// Accepte un enregistrement `.breezy` : le document en cours, une licence (`pro` : ce que
/// dit `droits::ouverte`), des annotations de taille permise, rien d'autre en cours.
pub fn accepter(
    etat: &EtatEditeur,
    id: u64,
    octets: usize,
    pro: bool,
    maintenant: Instant,
) -> Result<(), Refus> {
    if id != etat.document.id {
        return Err(Refus::Id);
    }
    if !pro {
        return Err(Refus::Pro);
    }
    if octets == 0 || octets > ANNOTATIONS_OCTETS_MAX {
        return Err(Refus::Taille);
    }
    if editeur::breezy_en_cours(etat, maintenant) || editeur::export_en_cours(etat, maintenant) {
        return Err(Refus::DejaEnCours);
    }
    Ok(())
}

/// Ce qu'est devenu l'enregistrement (`editeur:breezy.etat`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issue {
    Fait,
    /// Dialogue annulé, ou déjà ouvert : rien n'a été tenté.
    Annule,
    Echec,
}

impl Issue {
    pub fn nom(self) -> &'static str {
        match self {
            Issue::Fait => "fait",
            Issue::Annule => "annule",
            Issue::Echec => "echec",
        }
    }
}

/// La charge de `editeur:breezy`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChargeBreezy {
    pub id: u64,
    /// La version du document qui a été enregistrée (celle de l'en-tête de la demande).
    pub version: u64,
    pub etat: &'static str,
    /// Un titre du §7.4 pour la page, en cas d'échec seulement.
    pub message: Option<&'static str>,
}

pub fn charge(id: u64, version: u64, issue: Issue) -> ChargeBreezy {
    ChargeBreezy {
        id,
        version,
        etat: issue.nom(),
        message: (issue == Issue::Echec).then_some(TITRE_ENREGISTREMENT_IMPOSSIBLE),
    }
}

/// `editeur breezy <id> refuse (<code>)`.
pub fn ligne_refus(id: u64, refus: Refus) -> String {
    format!("editeur breezy {id} refuse ({})", refus.code())
}

/// `editeur breezy <id> version <v> <etat> en <ms> ms`.
pub fn ligne_fin(id: u64, version: u64, issue: Issue, ms: u128) -> String {
    format!(
        "editeur breezy {id} version {version} {} en {ms} ms",
        issue.nom()
    )
}

/// Le nom proposé dans le dialogue : celui du fichier d'où vient le document, sinon le nom
/// d'une capture (`nom_png`, du modèle des réglages) sans son `.png`.
pub fn nom_propose(origine: Option<&Path>, nom_png: &str) -> String {
    if let Some(nom) = origine
        .and_then(Path::file_name)
        .and_then(|n| n.to_str())
        .filter(|n| !n.is_empty())
    {
        return nom.to_string();
    }
    let dernier = nom_png.rsplit(['/', '\\']).next().unwrap_or(nom_png);
    match dernier.len().checked_sub(4) {
        Some(i) if dernier.is_char_boundary(i) && dernier[i..].eq_ignore_ascii_case(".png") => {
            dernier[..i].to_string()
        }
        _ => dernier.to_string(),
    }
}

/// Le dossier d'où le dialogue part : celui du fichier d'origine s'il existe encore, sinon
/// le dossier d'enregistrement des réglages.
pub fn dossier_depart(
    origine: Option<&Path>,
    reglages: &Path,
    existe: impl Fn(&Path) -> bool,
) -> PathBuf {
    origine
        .and_then(Path::parent)
        .filter(|d| !d.as_os_str().is_empty() && existe(d))
        .map(Path::to_path_buf)
        .unwrap_or_else(|| reglages.to_path_buf())
}

// ---------- ouvrir ----------

/// Le bandeau d'un fichier `.breezy` qui n'a pas pu être ouvert (Erreur, titre seul).
pub fn message_ouverture(erreur: ErreurBreezy) -> Message {
    let titre = match erreur {
        ErreurBreezy::Abime => "Impossible d'ouvrir ce fichier .breezy",
        ErreurBreezy::PlusRecent => "Fichier fait par une version plus récente de BreezyShot",
        ErreurBreezy::TropGros => "Ce fichier .breezy est trop grand",
    };
    Message {
        genre: Genre::Erreur,
        titre: titre.into(),
        detail: String::new(),
        actions: Vec::new(),
        duree_ms: None,
    }
}

/// `breezy ouverture refusee (<code>)` : jamais de chemin.
pub fn ligne_ouverture_refusee(erreur: ErreurBreezy) -> String {
    format!("breezy ouverture refusee ({})", erreur.code())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::editeur::{Document, EtatEditeur};
    use crate::systeme::gel::Pixels;
    use chrono::Local;
    use std::sync::Arc;

    fn etat() -> EtatEditeur {
        EtatEditeur::nouveau(
            Document {
                id: 7,
                pixels: Arc::new(Pixels {
                    largeur: 4,
                    hauteur: 3,
                    rgba: vec![0; 48],
                }),
                transparence: false,
                maintenant: Local::now(),
                annotations: None,
                breezy: None,
            },
            Instant::now(),
            1,
        )
    }

    #[test]
    fn les_bornes_de_la_sortie_sont_celles_de_l_editeur() {
        assert_eq!(crate::sortie::COTE_MAX_BREEZY, editeur::COTE_MAX);
        assert_eq!(crate::sortie::PIXELS_MAX_BREEZY, editeur::PIXELS_MAX);
    }

    #[test]
    fn accepter_dans_l_ordre_id_licence_taille_en_cours() {
        let mut e = etat();
        let t = Instant::now();
        assert_eq!(accepter(&e, 8, 10, true, t), Err(Refus::Id));
        assert_eq!(accepter(&e, 7, 10, false, t), Err(Refus::Pro));
        assert_eq!(accepter(&e, 7, 0, true, t), Err(Refus::Taille));
        assert_eq!(
            accepter(&e, 7, ANNOTATIONS_OCTETS_MAX + 1, true, t),
            Err(Refus::Taille)
        );
        assert_eq!(accepter(&e, 7, ANNOTATIONS_OCTETS_MAX, true, t), Ok(()));
        e.breezy_en_cours = Some(editeur::BreezyEnCours {
            debut: t,
            dialogue: true,
        });
        assert_eq!(accepter(&e, 7, 10, true, t), Err(Refus::DejaEnCours));
        // Tant que le dialogue est ouvert, jamais oublié, même après OUBLI_BREEZY.
        assert_eq!(
            accepter(&e, 7, 10, true, t + editeur::OUBLI_BREEZY),
            Err(Refus::DejaEnCours)
        );
        // Une fois le dialogue fermé, OUBLI_BREEZY court à partir de sa fermeture.
        let apres = t + editeur::OUBLI_BREEZY;
        e.dialogue_breezy_ferme(apres);
        assert_eq!(accepter(&e, 7, 10, true, apres), Err(Refus::DejaEnCours));
        assert_eq!(
            accepter(&e, 7, 10, true, apres + editeur::OUBLI_BREEZY),
            Ok(())
        );
        // Et un export de pixels en cours bloque aussi.
        e.breezy_en_cours = None;
        e.export = Some(editeur::ExportEnCours {
            numero: 1,
            but: editeur::But::Copier,
            version: 1,
            debut: t,
            dialogue: false,
        });
        assert_eq!(accepter(&e, 7, 10, true, t), Err(Refus::DejaEnCours));
    }

    #[test]
    fn un_enregistrement_breezy_en_cours_bloque_les_exports_de_pixels() {
        let mut e = etat();
        let en_tete = editeur::EnTete {
            id: 7,
            version: 1,
            but: editeur::But::Copier,
            largeur: 4,
            hauteur: 3,
        };
        assert_eq!(
            editeur::accepter_export(&e, &en_tete, 48, Instant::now()),
            Ok(())
        );
        e.breezy_en_cours = Some(editeur::BreezyEnCours {
            debut: Instant::now(),
            dialogue: false,
        });
        assert_eq!(
            editeur::accepter_export(&e, &en_tete, 48, Instant::now()),
            Err(editeur::Refus::DejaEnCours)
        );
    }

    #[test]
    fn un_fichier_rouvert_est_propre_puis_un_autre_document_l_est_aussi() {
        let mut e = etat();
        assert_eq!(e.exportee, None);
        let mut d = e.document.clone();
        d.annotations = Some(Arc::new("{}".into()));
        d.breezy = Some(PathBuf::from("/x/a.breezy"));
        let c = editeur::charge(
            &EtatEditeur::nouveau(d.clone(), Instant::now(), 1),
            false,
            None,
            false,
            true,
        );
        assert!(c.depuis_breezy && c.pro);
        // Version de départ de la page : 0, déjà « exportée » : fermer ne demande rien.
        let neuf = EtatEditeur::nouveau(d.clone(), Instant::now(), 1);
        assert_eq!(neuf.exportee, Some(0));
        assert_eq!(
            editeur::fermeture(0, false, neuf.exportee),
            editeur::Fermeture::Directe
        );
        assert_eq!(
            editeur::fermeture(1, false, neuf.exportee),
            editeur::Fermeture::Demander
        );
        // Le remplacement garde la règle : il passe par `nouveau`.
        e.remplacer(d, Instant::now());
        assert_eq!(e.exportee, Some(0));
    }

    #[test]
    fn nom_propose_vient_du_fichier_sinon_de_la_capture() {
        assert_eq!(
            nom_propose(Some(Path::new("/d/Mon dessin.breezy")), "x.png"),
            "Mon dessin.breezy"
        );
        assert_eq!(
            nom_propose(None, "BreezyShot 2026-11-16 à 10.00.00.png"),
            "BreezyShot 2026-11-16 à 10.00.00"
        );
        assert_eq!(nom_propose(None, "Capture.PNG"), "Capture");
        assert_eq!(nom_propose(None, "Capture"), "Capture");
        assert_eq!(nom_propose(None, "é"), "é");
        assert_eq!(nom_propose(None, ""), "");
    }

    #[test]
    fn dossier_de_depart() {
        let reglages = Path::new("/r");
        let oui = |_: &Path| true;
        let non = |_: &Path| false;
        assert_eq!(
            dossier_depart(Some(Path::new("/d/a.breezy")), reglages, oui),
            PathBuf::from("/d")
        );
        assert_eq!(
            dossier_depart(Some(Path::new("/d/a.breezy")), reglages, non),
            PathBuf::from("/r")
        );
        assert_eq!(dossier_depart(None, reglages, oui), PathBuf::from("/r"));
        assert_eq!(
            dossier_depart(Some(Path::new("a.breezy")), reglages, oui),
            PathBuf::from("/r")
        );
    }

    #[test]
    fn charge_et_lignes_ne_montrent_ni_chemin_ni_contenu() {
        let c = charge(7, 3, Issue::Echec);
        assert_eq!(
            serde_json::to_string(&c).unwrap(),
            r#"{"id":7,"version":3,"etat":"echec","message":"Impossible d'enregistrer le .breezy"}"#
        );
        assert_eq!(charge(7, 3, Issue::Fait).message, None);
        assert_eq!(charge(7, 3, Issue::Annule).etat, "annule");
        assert_eq!(ligne_refus(7, Refus::Pro), "editeur breezy 7 refuse (pro)");
        assert_eq!(
            ligne_fin(7, 3, Issue::Fait, 12),
            "editeur breezy 7 version 3 fait en 12 ms"
        );
        assert_eq!(
            ligne_ouverture_refusee(ErreurBreezy::PlusRecent),
            "breezy ouverture refusee (plus_recent)"
        );
        for e in [
            ErreurBreezy::Abime,
            ErreurBreezy::PlusRecent,
            ErreurBreezy::TropGros,
        ] {
            assert_eq!(message_ouverture(e).genre, Genre::Erreur);
        }
    }
}
