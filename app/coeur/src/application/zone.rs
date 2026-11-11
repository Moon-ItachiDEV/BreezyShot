//! Capture d'une zone : les décisions de l'Application, en fonctions pures (§3.6).
//!
//! `mod.rs` fait les gestes (montrer, cacher, réserver Échap, lancer la sortie) ; ce
//! fichier dit lesquels, sans toucher à aucune fenêtre : que faire de chaque signal
//! pendant une capture, quel message montrer, quel dossier, quelles lignes de journal.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::etats::Declencheur;
use crate::capture;
use crate::fenetres::selection::Signal as SignalSelection;
use crate::fenetres::{Genre, Message};
use crate::sortie::{self, Annonce, Bilan, Etape, EtapeFichier};
use crate::systeme::gel::{Changement, EcranInfo};
use crate::systeme::Rect;

/// Titre de toute erreur avant la sortie (gel, sélection, découpe).
pub const TITRE_CAPTURE_IMPOSSIBLE: &str = "Impossible de capturer";
/// Nom affiché du dossier d'enregistrement par défaut.
pub const DOSSIER_PAR_DEFAUT_AFFICHE: &str = "Images › BreezyShot";
/// Sous-dossier du dossier local qui garde les captures récentes.
pub const SOUS_DOSSIER_RECENTS: &str = "Recents";

/// Ce qui peut arriver pendant qu'une capture de zone est en cours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Arrivee<'a> {
    /// Signal du module de sélection.
    Selection(&'a SignalSelection),
    /// Échap global (réservé le temps de la sélection).
    Echap,
    /// Écrans changés ou session verrouillée (l'écran figé s'est déjà caché).
    Bureau(Changement),
}

/// Pourquoi une capture n'a pas pu se faire. Chaque cas a sa cause dans le bandeau.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probleme {
    /// Le Gel n'existe pas sur ce système.
    NonDisponible,
    /// La photo du bureau a échoué.
    Photo,
    /// L'écran figé n'a pas pu s'afficher.
    EcranFige,
    /// La sélection n'a pas pu s'afficher, ou a rendu une donnée invalide.
    Selection,
    /// La sélection n'a pas répondu à temps.
    Muette,
    /// Les pixels de la zone n'ont pas pu être lus.
    Lecture,
}

/// Ce que l'Application doit faire d'une arrivée.
#[derive(Debug, Clone, PartialEq)]
pub enum Geste {
    /// Rien : pas de capture en cours, signal d'un autre affichage, ou `Prete`.
    Rien,
    /// Tout cacher, libérer Échap, appliquer `declencheur` (retour au Repos), puis, s'il y a
    /// un `probleme`, le bandeau d'erreur.
    Abandonner {
        declencheur: Declencheur,
        probleme: Option<Probleme>,
    },
    /// Cacher l'écran figé tout de suite, libérer Échap, `ZoneChoisie`, découper cette zone
    /// (déjà bornée à un écran et assez grande), puis la sortie.
    Decouper(Rect),
}

/// Ce que l'Application sait de la capture en cours, pour décider.
#[derive(Debug, Clone, Copy)]
pub struct Vue<'a> {
    /// L'affichage de sélection en cours ; `None` tant que `montrer` n'a pas rendu son
    /// `id` (un signal de fin arrivé pendant `montrer` vaut alors pour cette capture).
    pub selection: Option<u64>,
    /// Les écrans de la photo.
    pub ecrans: &'a [EcranInfo],
}

/// Décide d'une arrivée (fonction pure). `capture` : `None` hors capture.
pub fn decider(capture: Option<Vue<'_>>, arrivee: Arrivee<'_>) -> Geste {
    let Some(vue) = capture else {
        return Geste::Rien;
    };
    let abandon = |declencheur, probleme| Geste::Abandonner {
        declencheur,
        probleme,
    };
    match arrivee {
        Arrivee::Echap => abandon(Declencheur::SelectionAnnulee, None),
        Arrivee::Bureau(Changement::Ecrans) => abandon(Declencheur::EcransChanges, None),
        Arrivee::Bureau(Changement::SessionVerrouillee) => {
            abandon(Declencheur::SessionVerrouillee, None)
        }
        Arrivee::Selection(signal) => {
            if vue.selection.is_some_and(|id| id != signal.id()) {
                return Geste::Rien;
            }
            match signal {
                SignalSelection::Prete { .. } => Geste::Rien,
                SignalSelection::Zone {
                    depart, arrivee, ..
                } => match zone_choisie(*depart, *arrivee, vue.ecrans) {
                    Some(zone) => Geste::Decouper(zone),
                    None => abandon(Declencheur::SelectionAnnulee, None),
                },
                SignalSelection::Annulee { .. } => abandon(Declencheur::SelectionAnnulee, None),
                SignalSelection::Muette { .. } => {
                    abandon(Declencheur::SelectionMuette, Some(Probleme::Muette))
                }
                SignalSelection::Impossible { .. } => {
                    abandon(Declencheur::SelectionImpossible, Some(Probleme::Selection))
                }
            }
        }
    }
}

/// La zone à découper : le rectangle des deux coins (inclus), coupé à l'écran du départ.
/// `None` si le départ n'est sur aucun écran ou si la zone est trop petite.
pub fn zone_choisie(depart: (i32, i32), arrivee: (i32, i32), ecrans: &[EcranInfo]) -> Option<Rect> {
    let zone = capture::rect_entre(depart, arrivee);
    capture::borner_a_un_ecran(zone, depart, ecrans).filter(|z| capture::zone_valide(*z))
}

/// La cause, en seconde ligne du bandeau d'erreur.
pub fn cause(probleme: Probleme) -> &'static str {
    match probleme {
        Probleme::NonDisponible => "La capture n'existe pas encore sur ce système.",
        Probleme::Photo => "Windows n'a pas donné l'image de l'écran.",
        Probleme::EcranFige => "L'écran n'a pas pu être figé.",
        Probleme::Selection => "La sélection n'a pas pu se faire.",
        Probleme::Muette => "La sélection n'a pas répondu à temps.",
        Probleme::Lecture => "L'image de la zone n'a pas pu être lue.",
    }
}

/// Nom court d'un problème pour le journal.
pub fn nom(probleme: Probleme) -> &'static str {
    match probleme {
        Probleme::NonDisponible => "non_disponible",
        Probleme::Photo => "photo",
        Probleme::EcranFige => "ecran_fige",
        Probleme::Selection => "selection",
        Probleme::Muette => "muette",
        Probleme::Lecture => "lecture",
    }
}

/// Le bandeau d'erreur d'une capture qui n'a pas pu se faire.
pub fn message_probleme(probleme: Probleme) -> Message {
    Message {
        genre: Genre::Erreur,
        titre: TITRE_CAPTURE_IMPOSSIBLE.into(),
        detail: cause(probleme).into(),
        actions: Vec::new(),
    }
}

/// Le bandeau d'après l'annonce de la sortie : une réussite n'a jamais de détail.
pub fn message_annonce(annonce: &Annonce) -> Message {
    match annonce {
        Annonce::Reussite { titre } => Message {
            genre: Genre::Reussite,
            titre: titre.clone(),
            detail: String::new(),
            actions: Vec::new(),
        },
        Annonce::Erreur { titre, cause } => Message {
            genre: Genre::Erreur,
            titre: titre.clone(),
            detail: cause.clone(),
            actions: Vec::new(),
        },
    }
}

/// Ligne de journal de l'annonce : le genre et le titre seuls (jamais la cause, qui peut
/// nommer un dossier).
pub fn ligne_annonce(annonce: &Annonce) -> String {
    match annonce {
        Annonce::Reussite { titre } => format!("sortie annonce reussite: {titre}"),
        Annonce::Erreur { titre, .. } => format!("sortie annonce erreur: {titre}"),
    }
}

/// `sortie copie <faite|non demandee|echec> recent <ok|echec> fichier <fait|non
/// demande|echec>` : aucun chemin, aucun contenu.
pub fn ligne_sortie(bilan: &Bilan) -> String {
    let copie = match bilan.copie {
        Etape::Faite => "faite",
        Etape::NonDemandee => "non demandee",
        Etape::Echec(_) => "echec",
    };
    let recent = if bilan.recent.is_ok() { "ok" } else { "echec" };
    let fichier = match bilan.enregistrement {
        EtapeFichier::Fait(_) => "fait",
        EtapeFichier::NonDemande => "non demande",
        EtapeFichier::Echec { .. } => "echec",
    };
    format!("sortie copie {copie} recent {recent} fichier {fichier}")
}

/// Le dossier d'enregistrement et son nom pour l'utilisateur. Réglage absent ou vide :
/// le dossier Images › BreezyShot résolu par `systeme` (vide s'il est introuvable :
/// l'enregistrement échouera alors avec sa cause). Sinon le réglage, nommé par
/// `sortie::nom_affiche`.
pub fn dossier_enregistrement(reglage: Option<&Path>, images: Option<&Path>) -> (PathBuf, String) {
    match reglage.filter(|d| !d.as_os_str().is_empty()) {
        Some(d) => (d.to_path_buf(), sortie::nom_affiche(d)),
        None => (
            images.map(Path::to_path_buf).unwrap_or_default(),
            DOSSIER_PAR_DEFAUT_AFFICHE.into(),
        ),
    }
}

/// Durée en millisecondes entières, pour le journal.
pub fn ms(duree: Duration) -> u128 {
    duree.as_millis()
}

/// `chrono gel photo <ms> total <ms> visible <ms>` (`visible` : de la réception du
/// raccourci à la fin de `montrer_ecran_fige`).
pub fn ligne_chrono_gel(photo: Duration, total: Duration, visible: Duration) -> String {
    format!(
        "chrono gel photo {} total {} visible {}",
        ms(photo),
        ms(total),
        ms(visible)
    )
}

/// `chrono decoupe <ms> sortie <ms> relache-annonce <ms>` (`relache-annonce` : du signal
/// `Zone` à la demande d'affichage du bandeau).
pub fn ligne_chrono_sortie(
    decoupe: Duration,
    sortie: Duration,
    relache_annonce: Duration,
) -> String {
    format!(
        "chrono decoupe {} sortie {} relache-annonce {}",
        ms(decoupe),
        ms(sortie),
        ms(relache_annonce)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fenetres::selection::Cause;

    fn r(x: i32, y: i32, largeur: i32, hauteur: i32) -> Rect {
        Rect {
            x,
            y,
            largeur,
            hauteur,
        }
    }

    /// Un écran à gauche (coordonnées négatives) et le principal.
    fn ecrans() -> Vec<EcranInfo> {
        let e = |rect: Rect, principal| EcranInfo {
            rect,
            zone_utile: rect,
            echelle: 1.0,
            principal,
        };
        vec![
            e(r(-1920, 0, 1920, 1080), false),
            e(r(0, 0, 1920, 1080), true),
        ]
    }

    fn vue(selection: Option<u64>, ecrans: &[EcranInfo]) -> Vue<'_> {
        Vue { selection, ecrans }
    }

    fn abandon(declencheur: Declencheur, probleme: Option<Probleme>) -> Geste {
        Geste::Abandonner {
            declencheur,
            probleme,
        }
    }

    #[test]
    fn hors_capture_rien_ne_se_passe() {
        let zone = SignalSelection::Zone {
            id: 1,
            depart: (0, 0),
            arrivee: (10, 10),
        };
        for a in [
            Arrivee::Selection(&zone),
            Arrivee::Echap,
            Arrivee::Bureau(Changement::Ecrans),
            Arrivee::Bureau(Changement::SessionVerrouillee),
        ] {
            assert_eq!(decider(None, a), Geste::Rien, "{a:?}");
        }
    }

    #[test]
    fn chaque_signal_de_la_selection() {
        let e = ecrans();
        let v = vue(Some(7), &e);
        let s = |signal: SignalSelection| decider(Some(v), Arrivee::Selection(&signal));
        assert_eq!(
            s(SignalSelection::Prete {
                id: 7,
                duree: Duration::from_millis(40)
            }),
            Geste::Rien
        );
        assert_eq!(
            s(SignalSelection::Zone {
                id: 7,
                depart: (110, 120),
                arrivee: (10, 20)
            }),
            Geste::Decouper(r(10, 20, 101, 101))
        );
        for cause in [Cause::Echap, Cause::ClicDroit, Cause::FocusPerdu] {
            assert_eq!(
                s(SignalSelection::Annulee { id: 7, cause }),
                abandon(Declencheur::SelectionAnnulee, None)
            );
        }
        assert_eq!(
            s(SignalSelection::Muette { id: 7 }),
            abandon(Declencheur::SelectionMuette, Some(Probleme::Muette))
        );
        assert_eq!(
            s(SignalSelection::Impossible {
                id: 7,
                cause: "x".into()
            }),
            abandon(Declencheur::SelectionImpossible, Some(Probleme::Selection))
        );
    }

    #[test]
    fn signal_d_un_autre_affichage_ignore() {
        let e = ecrans();
        let annulee = SignalSelection::Annulee {
            id: 6,
            cause: Cause::Echap,
        };
        assert_eq!(
            decider(Some(vue(Some(7), &e)), Arrivee::Selection(&annulee)),
            Geste::Rien
        );
        // Pendant `montrer` (id pas encore connu), un signal de fin vaut pour la capture.
        assert_eq!(
            decider(Some(vue(None, &e)), Arrivee::Selection(&annulee)),
            abandon(Declencheur::SelectionAnnulee, None)
        );
    }

    #[test]
    fn echap_et_bureau() {
        let e = ecrans();
        let v = Some(vue(Some(3), &e));
        assert_eq!(
            decider(v, Arrivee::Echap),
            abandon(Declencheur::SelectionAnnulee, None)
        );
        assert_eq!(
            decider(v, Arrivee::Bureau(Changement::Ecrans)),
            abandon(Declencheur::EcransChanges, None)
        );
        assert_eq!(
            decider(v, Arrivee::Bureau(Changement::SessionVerrouillee)),
            abandon(Declencheur::SessionVerrouillee, None)
        );
    }

    #[test]
    fn zone_bornee_a_l_ecran_du_depart_ou_annulee() {
        let e = ecrans();
        // Commencée à gauche, finie sur le principal : coupée au bord de l'écran de gauche.
        assert_eq!(
            zone_choisie((-100, 10), (200, 50), &e),
            Some(r(-100, 10, 100, 41))
        );
        // Les quatre sens donnent la même zone.
        let attendu = Some(r(10, 20, 200, 100));
        for (a, b) in [
            ((10, 20), (209, 119)),
            ((209, 119), (10, 20)),
            ((10, 119), (209, 20)),
            ((209, 20), (10, 119)),
        ] {
            assert_eq!(zone_choisie(a, b, &e), attendu);
        }
        // Départ hors de tout écran, ou zone trop petite après bornage : annulée sans
        // message.
        let v = Some(vue(Some(1), &e));
        let dehors = SignalSelection::Zone {
            id: 1,
            depart: (5000, 10),
            arrivee: (10, 10),
        };
        assert_eq!(
            decider(v, Arrivee::Selection(&dehors)),
            abandon(Declencheur::SelectionAnnulee, None)
        );
        assert_eq!(zone_choisie((-1, 10), (300, 300), &e), None);
        assert_eq!(zone_choisie((10, 10), (11, 300), &e), None);
        assert_eq!(zone_choisie((10, 10), (12, 12), &e), Some(r(10, 10, 3, 3)));
    }

    #[test]
    fn messages_du_bandeau() {
        let m = message_probleme(Probleme::Photo);
        assert_eq!(m.genre, Genre::Erreur);
        assert_eq!(m.titre, "Impossible de capturer");
        assert!(!m.detail.is_empty());
        assert!(m.actions.is_empty());
        for p in [
            Probleme::NonDisponible,
            Probleme::Photo,
            Probleme::EcranFige,
            Probleme::Selection,
            Probleme::Muette,
            Probleme::Lecture,
        ] {
            assert!(cause(p).ends_with('.'), "{p:?}");
            assert!(!nom(p).contains(' '));
        }
        let reussite = message_annonce(&Annonce::Reussite {
            titre: "Copié dans le presse-papiers".into(),
        });
        assert_eq!(reussite.genre, Genre::Reussite);
        assert_eq!(reussite.titre, "Copié dans le presse-papiers");
        assert!(reussite.detail.is_empty());
        let erreur = Annonce::Erreur {
            titre: "Impossible d'enregistrer".into(),
            cause: "Le dossier Images › BreezyShot est introuvable.".into(),
        };
        let m = message_annonce(&erreur);
        assert_eq!(m.genre, Genre::Erreur);
        assert_eq!(m.detail, "Le dossier Images › BreezyShot est introuvable.");
        assert_eq!(
            ligne_annonce(&erreur),
            "sortie annonce erreur: Impossible d'enregistrer"
        );
        assert_eq!(
            ligne_annonce(&Annonce::Reussite {
                titre: "Copié et enregistré".into()
            }),
            "sortie annonce reussite: Copié et enregistré"
        );
    }

    #[test]
    fn ligne_de_sortie_sans_chemin() {
        let b = Bilan {
            copie: Etape::Faite,
            recent: Ok(PathBuf::from("/secret/Recents/x.png")),
            enregistrement: EtapeFichier::NonDemande,
        };
        assert_eq!(
            ligne_sortie(&b),
            "sortie copie faite recent ok fichier non demande"
        );
        let b = Bilan {
            copie: Etape::NonDemandee,
            recent: Err("disque plein".into()),
            enregistrement: EtapeFichier::Fait(PathBuf::from("/secret/a.png")),
        };
        assert_eq!(
            ligne_sortie(&b),
            "sortie copie non demandee recent echec fichier fait"
        );
        let b = Bilan {
            copie: Etape::Echec("occupe".into()),
            recent: Ok(PathBuf::new()),
            enregistrement: EtapeFichier::Echec {
                dossier: PathBuf::from("/secret"),
                cause: "x".into(),
            },
        };
        let l = ligne_sortie(&b);
        assert_eq!(l, "sortie copie echec recent ok fichier echec");
        assert!(!l.contains("secret"));
    }

    #[test]
    fn dossier_affiche() {
        let images = PathBuf::from("/u/Images/BreezyShot");
        assert_eq!(
            dossier_enregistrement(None, Some(&images)),
            (images.clone(), "Images › BreezyShot".to_string())
        );
        assert_eq!(
            dossier_enregistrement(Some(Path::new("")), Some(&images)),
            (images.clone(), "Images › BreezyShot".to_string())
        );
        assert_eq!(
            dossier_enregistrement(None, None),
            (PathBuf::new(), "Images › BreezyShot".to_string())
        );
        let choisi = PathBuf::from("/u/Documents/Captures");
        assert_eq!(
            dossier_enregistrement(Some(&choisi), Some(&images)),
            (choisi.clone(), "Documents › Captures".to_string())
        );
    }

    #[test]
    fn lignes_de_chrono() {
        let d = Duration::from_micros;
        assert_eq!(
            ligne_chrono_gel(d(12_900), d(14_100), d(87_000)),
            "chrono gel photo 12 total 14 visible 87"
        );
        assert_eq!(
            ligne_chrono_sortie(d(3_000), d(180_400), d(200_000)),
            "chrono decoupe 3 sortie 180 relache-annonce 200"
        );
    }
}
