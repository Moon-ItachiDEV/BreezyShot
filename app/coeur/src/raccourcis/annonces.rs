//! Message à montrer au démarrage, tiré du bilan du choix (textes du contrat, §5) : une
//! ligne de titre, sans détail ni astuce ; la raison précise reste dans `Situation.raison`.
//!
//! Le module ne montre rien : l'Application transforme l'`Annonce` en message du bandeau
//! (genre « info », ou « erreur » si `erreur`). L'info et l'erreur restent affichées jusqu'au
//! clic sur la croix. Au plus une annonce par démarrage, et une seule fois par situation
//! (`decider_annonce`).

use super::choix::{Bilan, ChoixAction, Raison};
use super::notation::analyser;

/// Longueur maximale d'un candidat recopié dans un texte (au-delà : coupé, suivi de « … »).
const LONGUEUR_CANDIDAT: usize = 40;

/// Un message à montrer à l'utilisateur.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Annonce {
    pub erreur: bool,
    pub titre: String,
    pub detail: String,
}

/// La situation qui a donné lieu à une annonce. L'Application la garde dans
/// `<local>/etat.json` (trois champs texte) pour ne pas répéter le même message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Situation {
    /// Nom de l'action (`zone`…).
    pub action: String,
    /// Le premier candidat de l'action, tel qu'écrit dans les réglages (coupé à 40 caractères).
    pub candidat: String,
    /// `invalide`, `reserve_par_le_systeme`, `deja_pris`, `refuse_par_le_systeme` ;
    /// `action_sans_raccourci` quand cette action n'a rien obtenu mais une autre si ;
    /// `aucun_raccourci` quand aucune action n'a rien obtenu.
    pub raison: String,
}

/// Ce que l'Application doit faire au démarrage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    /// Le message à montrer, ou rien.
    pub annonce: Option<Annonce>,
    /// La situation à garder dans `etat.json` ; `None` : effacer celle qui y est.
    pub a_retenir: Option<Situation>,
}

fn couper(candidat: &str) -> String {
    let propre = candidat.trim();
    if propre.chars().count() <= LONGUEUR_CANDIDAT {
        propre.to_string()
    } else {
        let debut: String = propre.chars().take(LONGUEUR_CANDIDAT).collect();
        format!("{debut}…")
    }
}

fn code_raison(raison: &Raison) -> &'static str {
    match raison {
        Raison::Invalide(_) => "invalide",
        Raison::ReserveParLeSysteme => "reserve_par_le_systeme",
        Raison::DejaPrisParUneAutreAction(_) => "deja_pris",
        Raison::RefuseParLeSysteme => "refuse_par_le_systeme",
    }
}

/// Le nom d'une action dans « Aucun raccourci pour … ».
fn nom_dans_annonce(action: &str) -> String {
    match action {
        "zone" => "la zone".into(),
        "plein_ecran" => "l'écran".into(),
        "fenetre" => "la fenêtre".into(),
        "derniere_zone" => "la dernière zone".into(),
        "historique" => "l'historique".into(),
        "texte" => "le texte".into(),
        "defilante" => "la capture défilante".into(),
        autre => autre.into(),
    }
}

/// L'annonce d'une action qui avait des candidats et n'a rien obtenu. `aucune_autre` :
/// aucune action n'a obtenu de raccourci (erreur « Aucun raccourci disponible ») ; sinon
/// une info qui nomme l'action, pas la touche.
fn annonce_sans_raccourci(choix: &ChoixAction, aucune_autre: bool) -> (Annonce, Situation) {
    let candidat = choix
        .ecartes
        .first()
        .map(|e| couper(&e.candidat))
        .unwrap_or_default();
    let (erreur, titre, raison) = if aucune_autre {
        (
            true,
            "Aucun raccourci disponible".to_string(),
            "aucun_raccourci",
        )
    } else {
        (
            false,
            format!("Aucun raccourci pour {}", nom_dans_annonce(&choix.action)),
            "action_sans_raccourci",
        )
    };
    let annonce = Annonce {
        erreur,
        titre,
        detail: String::new(),
    };
    let situation = Situation {
        action: choix.action.clone(),
        candidat,
        raison: raison.into(),
    };
    (annonce, situation)
}

/// L'annonce d'une action et sa situation, si son premier candidat n'a pas été retenu
/// et qu'un repli l'a été.
fn annonce_action(choix: &ChoixAction) -> Option<(Annonce, Situation)> {
    if !choix.premier_ecarte() {
        return None;
    }
    let premier = &choix.ecartes[0];
    let situation = Situation {
        action: choix.action.clone(),
        candidat: couper(&premier.candidat),
        raison: code_raison(&premier.raison).to_string(),
    };
    let retenu = choix.retenu.as_ref()?;
    // Un message tient en une ligne de titre, sans astuce ; la raison précise reste dans
    // la situation (et le journal).
    let y = retenu.affichage();
    let lu = analyser(&premier.candidat).ok();
    let impr_ecran = lu.is_some_and(|r| r.est_impr_ecran_seule());
    let titre = match &premier.raison {
        Raison::ReserveParLeSysteme | Raison::RefuseParLeSysteme if impr_ecran => {
            format!("Impr. écran est prise. Raccourci : {y}")
        }
        Raison::Invalide(_) => format!(
            "« {} » n'est pas un raccourci valide. Raccourci : {y}",
            couper(&premier.candidat)
        ),
        _ => {
            let x = lu
                .map(|r| couper(&r.affichage()))
                .unwrap_or_else(|| couper(&premier.candidat));
            format!("{x} est pris. Raccourci : {y}")
        }
    };
    let detail = String::new();
    let annonce = Annonce {
        erreur: false,
        titre,
        detail,
    };
    Some((annonce, situation))
}

impl Bilan {
    /// L'annonce du démarrage et sa situation : au plus une. D'abord une action qui avait
    /// des candidats et n'a rien obtenu : erreur « Aucun raccourci disponible » si aucune
    /// action n'a de raccourci, sinon info « Aucun raccourci pour … » ; sinon la première
    /// action, dans l'ordre de traitement, dont le premier candidat n'a pas été retenu
    /// (info).
    fn annonce_et_situation(&self) -> Option<(Annonce, Situation)> {
        let sans_rien = self
            .actions
            .iter()
            .find(|a| a.nb_candidats > 0 && a.retenu.is_none());
        match sans_rien {
            Some(a) => {
                let aucune_autre = self.actions.iter().all(|x| x.retenu.is_none());
                Some(annonce_sans_raccourci(a, aucune_autre))
            }
            None => self.actions.iter().find_map(annonce_action),
        }
    }

    /// L'annonce à montrer au démarrage, s'il y en a une (sans tenir compte des démarrages
    /// précédents : voir `decider_annonce`).
    pub fn annonce(&self) -> Option<Annonce> {
        self.annonce_et_situation().map(|(a, _)| a)
    }
}

/// Annoncer ou non, d'après la dernière situation annoncée (lue dans `etat.json`).
/// - tout est normal : pas d'annonce, `a_retenir: None` (l'Application efface) ;
/// - même situation que la dernière : pas d'annonce, la situation reste retenue ;
/// - situation nouvelle : l'annonce, et la situation à retenir.
pub fn decider_annonce(derniere: Option<&Situation>, bilan: &Bilan) -> Decision {
    match bilan.annonce_et_situation() {
        None => Decision {
            annonce: None,
            a_retenir: None,
        },
        Some((annonce, situation)) => Decision {
            annonce: (derniere != Some(&situation)).then_some(annonce),
            a_retenir: Some(situation),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raccourcis::choisir;
    use std::collections::BTreeMap;

    fn table(lignes: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
        lignes
            .iter()
            .map(|(a, c)| (a.to_string(), c.iter().map(|s| s.to_string()).collect()))
            .collect()
    }

    fn defaut() -> BTreeMap<String, Vec<String>> {
        table(&[
            ("zone", &["ImprEcran", "Ctrl+Maj+1"]),
            ("plein_ecran", &["Ctrl+Maj+2"]),
            ("fenetre", &["Ctrl+Maj+3"]),
        ])
    }

    fn zone() -> BTreeMap<String, Vec<String>> {
        table(&[("zone", &["ImprEcran", "Ctrl+Maj+1"])])
    }

    #[test]
    fn rien_a_dire() {
        assert_eq!(choisir(&defaut(), false, |_| true).annonce(), None);
        let vide = table(&[("zone", &[])]);
        assert_eq!(choisir(&vide, false, |_| true).annonce(), None);
    }

    #[test]
    fn reservee() {
        assert_eq!(
            choisir(&defaut(), true, |_| true).annonce(),
            Some(Annonce {
                erreur: false,
                titre: "Impr. écran est prise. Raccourci : Ctrl + Maj + 1".into(),
                detail: String::new(),
            })
        );
    }

    #[test]
    fn prise_par_une_autre_application() {
        assert_eq!(
            choisir(&defaut(), false, |r| r.canonique() != "ImprEcran").annonce(),
            Some(Annonce {
                erreur: false,
                titre: "Impr. écran est prise. Raccourci : Ctrl + Maj + 1".into(),
                detail: String::new(),
            })
        );
    }

    #[test]
    fn le_repli_affiche_est_le_vrai_retenu() {
        let t = table(&[("zone", &["ImprEcran", "Ctrl+Maj+1", "alt+f9"])]);
        let a = choisir(&t, true, |r| r.canonique() != "Ctrl+Maj+1").annonce();
        assert_eq!(
            a.unwrap().titre,
            "Impr. écran est prise. Raccourci : Alt + F9"
        );
    }

    #[test]
    fn aucun_raccourci() {
        assert_eq!(
            choisir(&defaut(), false, |_| false).annonce(),
            Some(Annonce {
                erreur: true,
                titre: "Aucun raccourci disponible".into(),
                detail: String::new(),
            })
        );
    }

    #[test]
    fn une_seule_annonce_l_action_sans_raccourci_d_abord() {
        // zone : info (réservée) ; plein_ecran : rien obtenu ; fenetre : info.
        let t = table(&[
            ("fenetre", &["Ctrl+Maj+3", "Ctrl+Maj+4"]),
            ("zone", &["ImprEcran", "Ctrl+Maj+1"]),
            ("plein_ecran", &["Ctrl+Maj+2"]),
        ]);
        let b = choisir(&t, true, |r| {
            r.canonique() != "Ctrl+Maj+3" && r.canonique() != "Ctrl+Maj+2"
        });
        assert_eq!(
            b.annonce(),
            Some(Annonce {
                erreur: false,
                titre: "Aucun raccourci pour l'écran".into(),
                detail: String::new(),
            })
        );
        assert_eq!(
            decider_annonce(None, &b).a_retenir,
            Some(Situation {
                action: "plein_ecran".into(),
                candidat: "Ctrl+Maj+2".into(),
                raison: "action_sans_raccourci".into(),
            })
        );
    }

    #[test]
    fn action_sans_raccourci_nommee() {
        // La zone n'obtient rien, le plein écran garde le sien.
        let b = choisir(&defaut(), true, |r| r.canonique() != "Ctrl+Maj+1");
        let a = b.annonce().unwrap();
        assert!(!a.erreur);
        assert_eq!(a.titre, "Aucun raccourci pour la zone");
        assert_eq!(a.detail, "");
        let t = table(&[("zone", &["Ctrl+Maj+1"]), ("fenetre", &["Ctrl+Maj+3"])]);
        let b = choisir(&t, false, |r| r.canonique() != "Ctrl+Maj+3");
        assert_eq!(
            b.annonce().unwrap().titre,
            "Aucun raccourci pour la fenêtre"
        );
        // Une action sans candidat (aucun raccourci voulu) ne compte pas comme privée,
        // mais n'a pas de raccourci non plus : si la seule autre échoue, c'est l'erreur.
        let t = table(&[("zone", &[]), ("plein_ecran", &["Ctrl+Maj+2"])]);
        let a = choisir(&t, false, |_| false).annonce().unwrap();
        assert!(a.erreur);
        assert_eq!(a.titre, "Aucun raccourci disponible");
    }

    /// Contrat 0.6, §7.2 : `Ctrl + Maj + 5` pris par une autre app, le repli existant.
    #[test]
    fn defilante_sans_raccourci_nommee() {
        let t = table(&[("zone", &["Ctrl+Maj+1"]), ("defilante", &["Ctrl+Maj+5"])]);
        let b = choisir(&t, false, |r| r.canonique() != "Ctrl+Maj+5");
        let a = b.annonce().unwrap();
        assert!(!a.erreur);
        assert_eq!(a.titre, "Aucun raccourci pour la capture défilante");
        assert_eq!(
            decider_annonce(None, &b).a_retenir.unwrap().action,
            "defilante"
        );
    }

    /// D'« une action sans raccourci » à « plus aucune » : situation différente, l'erreur
    /// est réaffichée ; et retour.
    #[test]
    fn passage_d_une_action_a_aucune() {
        let une = choisir(&defaut(), false, |r| r.canonique() != "Ctrl+Maj+3");
        let d1 = decider_annonce(None, &une);
        assert_eq!(
            d1.a_retenir.as_ref().unwrap().raison,
            "action_sans_raccourci"
        );
        let aucune = choisir(&defaut(), false, |_| false);
        let d2 = decider_annonce(d1.a_retenir.as_ref(), &aucune);
        assert!(d2.annonce.unwrap().erreur);
        assert_eq!(d2.a_retenir.as_ref().unwrap().raison, "aucun_raccourci");
        let d3 = decider_annonce(d2.a_retenir.as_ref(), &une);
        assert_eq!(d3.annonce.unwrap().titre, "Aucun raccourci pour la fenêtre");
        // Même situation au démarrage suivant : silence.
        assert_eq!(decider_annonce(d3.a_retenir.as_ref(), &une).annonce, None);
    }

    #[test]
    fn une_seule_annonce_la_premiere_action_sinon() {
        let t = table(&[
            ("fenetre", &["Ctrl+Maj+3", "Ctrl+Maj+4"]),
            ("zone", &["ImprEcran", "Ctrl+Maj+1"]),
        ]);
        let b = choisir(&t, true, |r| r.canonique() != "Ctrl+Maj+3");
        let a = b.annonce().unwrap();
        assert_eq!(a.titre, "Impr. écran est prise. Raccourci : Ctrl + Maj + 1");
    }

    #[test]
    fn premier_invalide_ou_deja_pris() {
        let t = table(&[("zone", &["Ctrl+Truc", "Ctrl+Maj+1"])]);
        let a = choisir(&t, false, |_| true).annonce().unwrap();
        assert_eq!(
            a.titre,
            "« Ctrl+Truc » n'est pas un raccourci valide. Raccourci : Ctrl + Maj + 1"
        );
        assert_eq!(a.detail, "");

        let t = table(&[
            ("zone", &["Ctrl+Maj+1"]),
            ("plein_ecran", &["Ctrl+Maj+1", "Ctrl+Maj+2"]),
        ]);
        let a = choisir(&t, false, |_| true).annonce().unwrap();
        assert_eq!(
            a.titre,
            "Ctrl + Maj + 1 est pris. Raccourci : Ctrl + Maj + 2"
        );
        assert_eq!(a.detail, "");

        let t = table(&[("zone", &["Ctrl+Maj+5", "Ctrl+Maj+6"])]);
        let a = choisir(&t, false, |r| r.canonique() != "Ctrl+Maj+5").annonce();
        assert_eq!(
            a.unwrap().titre,
            "Ctrl + Maj + 5 est pris. Raccourci : Ctrl + Maj + 6"
        );
    }

    #[test]
    fn candidat_long_coupe_a_40() {
        let long = format!("Ctrl+{}", "X".repeat(200));
        let t = table(&[("zone", &[long.as_str(), "Ctrl+Maj+1"])]);
        let b = choisir(&t, false, |_| true);
        let a = b.annonce().unwrap();
        let attendu: String = long.chars().take(40).collect();
        assert_eq!(
            a.titre,
            format!("« {attendu}… » n'est pas un raccourci valide. Raccourci : Ctrl + Maj + 1")
        );
        let d = decider_annonce(None, &b);
        assert_eq!(d.a_retenir.unwrap().candidat, format!("{attendu}…"));
        // Exactement 40 caractères : pas coupé.
        assert_eq!(couper(&"é".repeat(40)), "é".repeat(40));
        assert_eq!(couper(&"é".repeat(41)), format!("{}…", "é".repeat(40)));
    }

    #[test]
    fn decider_tout_normal() {
        let b = choisir(&zone(), false, |_| true);
        let ancienne = Situation {
            action: "zone".into(),
            candidat: "ImprEcran".into(),
            raison: "reserve_par_le_systeme".into(),
        };
        let attendu = Decision {
            annonce: None,
            a_retenir: None,
        };
        assert_eq!(decider_annonce(None, &b), attendu);
        assert_eq!(decider_annonce(Some(&ancienne), &b), attendu);
    }

    #[test]
    fn decider_nouvelle_puis_meme_situation() {
        let b = choisir(&zone(), true, |_| true);
        let d = decider_annonce(None, &b);
        let situation = Situation {
            action: "zone".into(),
            candidat: "ImprEcran".into(),
            raison: "reserve_par_le_systeme".into(),
        };
        assert_eq!(d.annonce, b.annonce());
        assert!(d.annonce.is_some());
        assert_eq!(d.a_retenir.as_ref(), Some(&situation));
        // Démarrage suivant, même situation : silence, on la garde.
        let d2 = decider_annonce(Some(&situation), &b);
        assert_eq!(d2.annonce, None);
        assert_eq!(d2.a_retenir, Some(situation.clone()));
        // La situation change (Impr. écran prise au lieu de réservée) : on annonce.
        let b3 = choisir(&zone(), false, |r| !r.est_impr_ecran_seule());
        let d3 = decider_annonce(Some(&situation), &b3);
        // Même texte que « réservée », mais une autre situation : on annonce quand même.
        assert_eq!(
            d3.annonce.unwrap().titre,
            "Impr. écran est prise. Raccourci : Ctrl + Maj + 1"
        );
        assert_eq!(d3.a_retenir.unwrap().raison, "refuse_par_le_systeme");
        // Plus aucun raccourci : situation « aucun_raccourci », annonce d'erreur.
        let b4 = choisir(&zone(), true, |_| false);
        let d4 = decider_annonce(Some(&situation), &b4);
        assert!(d4.annonce.unwrap().erreur);
        assert_eq!(
            d4.a_retenir,
            Some(Situation {
                action: "zone".into(),
                candidat: "ImprEcran".into(),
                raison: "aucun_raccourci".into(),
            })
        );
    }
}
