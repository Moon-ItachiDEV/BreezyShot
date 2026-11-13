//! Choix des raccourcis au démarrage : fonction pure, sans Tauri ni système.
//!
//! Pour chaque action, on parcourt ses candidats dans l'ordre et on garde le premier
//! qui passe. Le repli n'est pas un cas particulier : c'est le candidat suivant.

use std::collections::{BTreeMap, HashSet};

use super::notation::{analyser, ErreurNotation, Raccourci};

/// Les actions dont l'ordre de traitement est fixé. Les autres suivent, par ordre
/// alphabétique.
pub const ACTIONS_PRIORITAIRES: [&str; 6] = [
    "zone",
    "plein_ecran",
    "fenetre",
    "derniere_zone",
    "historique",
    "texte",
];

/// Pourquoi un candidat a été écarté.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Raison {
    /// La notation n'a pas pu être lue.
    Invalide(ErreurNotation),
    /// `ImprEcran` seule alors que Windows la réserve à l'Outil Capture.
    ReserveParLeSysteme,
    /// Déjà retenu dans cette session par l'action nommée.
    DejaPrisParUneAutreAction(String),
    /// Le système a refusé la réservation (souvent : une autre application l'a déjà).
    RefuseParLeSysteme,
}

/// Un candidat écarté, tel qu'il était écrit dans les réglages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ecarte {
    pub candidat: String,
    pub raison: Raison,
}

/// Le résultat pour une action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoixAction {
    pub action: String,
    /// Le raccourci retenu, ou rien.
    pub retenu: Option<Raccourci>,
    /// Les candidats écartés avant d'arriver au retenu (ou tous, si aucun n'est retenu).
    /// Les candidats placés après le retenu ne sont pas examinés.
    pub ecartes: Vec<Ecarte>,
    /// Nombre de candidats donnés par les réglages pour cette action.
    pub nb_candidats: usize,
}

impl ChoixAction {
    /// Vrai si le premier candidat n'a pas été retenu (liste non vide).
    pub fn premier_ecarte(&self) -> bool {
        self.nb_candidats > 0 && !self.ecartes.is_empty()
    }
}

/// Le bilan complet, actions dans l'ordre où elles ont été traitées.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bilan {
    pub actions: Vec<ChoixAction>,
}

impl Bilan {
    /// Le résultat d'une action, si elle figurait dans les réglages.
    pub fn action(&self, nom: &str) -> Option<&ChoixAction> {
        self.actions.iter().find(|a| a.action == nom)
    }

    /// Le raccourci retenu pour une action.
    pub fn retenu(&self, action: &str) -> Option<&Raccourci> {
        self.action(action).and_then(|a| a.retenu.as_ref())
    }

    /// L'action à laquelle appartient un raccourci retenu (pour traduire un appui).
    pub fn action_de(&self, raccourci: &Raccourci) -> Option<&str> {
        self.actions
            .iter()
            .find(|a| a.retenu.as_ref() == Some(raccourci))
            .map(|a| a.action.as_str())
    }

    /// Le libellé du raccourci actif d'une action, pour le menu de l'icône
    /// (`Ctrl + Maj + 1`), ou rien si l'action n'a pas de raccourci.
    pub fn libelle_menu(&self, action: &str) -> Option<String> {
        self.retenu(action).map(Raccourci::affichage)
    }
}

/// Rang de tri : d'abord les actions prioritaires dans leur ordre, puis les autres.
fn ordre_actions<'a>(noms: impl Iterator<Item = &'a String>) -> Vec<&'a String> {
    let mut noms: Vec<&String> = noms.collect();
    noms.sort_by(|a, b| {
        let rang = |n: &str| {
            ACTIONS_PRIORITAIRES
                .iter()
                .position(|p| *p == n)
                .unwrap_or(ACTIONS_PRIORITAIRES.len())
        };
        rang(a).cmp(&rang(b)).then_with(|| a.cmp(b))
    });
    noms
}

/// Choisit un raccourci par action.
///
/// - `candidats` : par nom d'action, les notations par ordre de préférence (réglages).
/// - `impr_ecran_reserve` : réponse de `systeme::impr_ecran_reserve()`.
/// - `essayer` : tente la réservation auprès du système, vrai si elle a réussi.
///   Elle n'est jamais appelée pour un raccourci déjà retenu par une autre action, ni deux
///   fois pour un même raccourci refusé.
pub fn choisir<F>(
    candidats: &BTreeMap<String, Vec<String>>,
    impr_ecran_reserve: bool,
    mut essayer: F,
) -> Bilan
where
    F: FnMut(&Raccourci) -> bool,
{
    let mut pris: Vec<(Raccourci, String)> = Vec::new();
    let mut refuses: HashSet<Raccourci> = HashSet::new();
    let mut bilan = Bilan::default();

    for action in ordre_actions(candidats.keys()) {
        let choix = choisir_parmi(
            action,
            &candidats[action],
            &pris,
            None,
            &mut refuses,
            impr_ecran_reserve,
            &mut essayer,
        );
        if let Some(r) = choix.retenu {
            pris.push((r, action.clone()));
        }
        bilan.actions.push(choix);
    }
    bilan
}

/// Choisit le raccourci d'une seule action, les autres gardant les leurs (changement fait
/// dans la fenêtre Réglages, étape 10).
///
/// - `pris` : les raccourcis retenus par les autres actions, avec leur nom d'action.
/// - `actuel` : le raccourci que l'action tient déjà ; s'il est parmi les candidats, il est
///   retenu sans redemander au système (il est déjà à elle).
///
/// Les autres règles sont celles de `choisir`.
pub fn choisir_action<F>(
    action: &str,
    candidats: &[String],
    pris: &[(Raccourci, String)],
    actuel: Option<Raccourci>,
    impr_ecran_reserve: bool,
    mut essayer: F,
) -> ChoixAction
where
    F: FnMut(&Raccourci) -> bool,
{
    let mut refuses = HashSet::new();
    choisir_parmi(
        action,
        candidats,
        pris,
        actuel,
        &mut refuses,
        impr_ecran_reserve,
        &mut essayer,
    )
}

fn choisir_parmi<F>(
    action: &str,
    liste: &[String],
    pris: &[(Raccourci, String)],
    actuel: Option<Raccourci>,
    refuses: &mut HashSet<Raccourci>,
    impr_ecran_reserve: bool,
    essayer: &mut F,
) -> ChoixAction
where
    F: FnMut(&Raccourci) -> bool,
{
    let mut choix = ChoixAction {
        action: action.to_string(),
        retenu: None,
        ecartes: Vec::new(),
        nb_candidats: liste.len(),
    };
    for texte in liste {
        let raison = match analyser(texte) {
            Err(e) => Raison::Invalide(e),
            Ok(r) if impr_ecran_reserve && r.est_impr_ecran_seule() => Raison::ReserveParLeSysteme,
            Ok(r) => {
                if let Some((_, autre)) = pris.iter().find(|(p, _)| *p == r) {
                    Raison::DejaPrisParUneAutreAction(autre.clone())
                } else if actuel == Some(r) {
                    choix.retenu = Some(r);
                    break;
                } else if refuses.contains(&r) || !essayer(&r) {
                    refuses.insert(r);
                    Raison::RefuseParLeSysteme
                } else {
                    choix.retenu = Some(r);
                    break;
                }
            }
        };
        choix.ecartes.push(Ecarte {
            candidat: texte.clone(),
            raison,
        });
    }
    choix
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(lignes: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
        lignes
            .iter()
            .map(|(a, c)| (a.to_string(), c.iter().map(|s| s.to_string()).collect()))
            .collect()
    }

    fn r(t: &str) -> Raccourci {
        analyser(t).unwrap()
    }

    fn defaut() -> BTreeMap<String, Vec<String>> {
        table(&[
            ("zone", &["ImprEcran", "Ctrl+Maj+1"]),
            ("plein_ecran", &["Ctrl+Maj+2"]),
            ("fenetre", &["Ctrl+Maj+3"]),
        ])
    }

    #[test]
    fn tout_passe() {
        let mut essais = Vec::new();
        let b = choisir(&defaut(), false, |x| {
            essais.push(x.canonique());
            true
        });
        let noms: Vec<_> = b.actions.iter().map(|a| a.action.as_str()).collect();
        assert_eq!(noms, ["zone", "plein_ecran", "fenetre"]);
        assert_eq!(essais, ["ImprEcran", "Ctrl+Maj+2", "Ctrl+Maj+3"]);
        assert_eq!(b.retenu("zone"), Some(&r("ImprEcran")));
        assert!(b.actions.iter().all(|a| a.ecartes.is_empty()));
        assert_eq!(b.libelle_menu("zone").as_deref(), Some("Impr. écran"));
        assert_eq!(b.action_de(&r("ctrl+maj+3")), Some("fenetre"));
        assert_eq!(b.action_de(&r("ctrl+maj+9")), None);
    }

    #[test]
    fn impr_ecran_reservee_sans_essai() {
        let mut essais = Vec::new();
        let b = choisir(&defaut(), true, |x| {
            essais.push(x.canonique());
            true
        });
        assert!(!essais.contains(&"ImprEcran".to_string()));
        let zone = b.action("zone").unwrap();
        assert_eq!(zone.retenu, Some(r("Ctrl+Maj+1")));
        assert_eq!(
            zone.ecartes,
            vec![Ecarte {
                candidat: "ImprEcran".into(),
                raison: Raison::ReserveParLeSysteme
            }]
        );
        assert!(zone.premier_ecarte());
        assert_eq!(b.libelle_menu("zone").as_deref(), Some("Ctrl + Maj + 1"));
    }

    #[test]
    fn reserve_ne_vise_que_la_touche_seule() {
        let t = table(&[("zone", &["Ctrl+ImprEcran"])]);
        let b = choisir(&t, true, |_| true);
        assert_eq!(b.retenu("zone"), Some(&r("Ctrl+ImprEcran")));
    }

    #[test]
    fn refuse_par_le_systeme() {
        let b = choisir(&defaut(), false, |x| *x != r("ImprEcran"));
        let zone = b.action("zone").unwrap();
        assert_eq!(zone.retenu, Some(r("Ctrl+Maj+1")));
        assert_eq!(zone.ecartes[0].raison, Raison::RefuseParLeSysteme);
    }

    #[test]
    fn candidat_invalide() {
        let t = table(&[("zone", &["Ctrl+Truc", "A", "Ctrl+Maj+1"])]);
        let b = choisir(&t, false, |_| true);
        let zone = b.action("zone").unwrap();
        assert_eq!(zone.retenu, Some(r("Ctrl+Maj+1")));
        assert_eq!(
            zone.ecartes[0].raison,
            Raison::Invalide(ErreurNotation::ToucheInconnue("Truc".into()))
        );
        assert_eq!(
            zone.ecartes[1].raison,
            Raison::Invalide(ErreurNotation::ToucheSeuleInterdite("A".into()))
        );
    }

    #[test]
    fn tous_refuses() {
        let b = choisir(&defaut(), false, |_| false);
        for a in &b.actions {
            assert_eq!(a.retenu, None);
            assert_eq!(a.ecartes.len(), a.nb_candidats);
            assert!(a
                .ecartes
                .iter()
                .all(|e| e.raison == Raison::RefuseParLeSysteme));
        }
        assert_eq!(b.libelle_menu("zone"), None);
    }

    #[test]
    fn meme_candidat_pour_deux_actions() {
        let t = table(&[
            ("zone", &["Ctrl+Maj+1"]),
            ("fenetre", &["ctrl + maj + 1", "Ctrl+Maj+3"]),
        ]);
        let mut essais = Vec::new();
        let b = choisir(&t, false, |x| {
            essais.push(x.canonique());
            true
        });
        // La zone passe en premier, même si « fenetre » vient avant dans l'alphabet.
        assert_eq!(b.actions[0].action, "zone");
        assert_eq!(essais, ["Ctrl+Maj+1", "Ctrl+Maj+3"]);
        let fenetre = b.action("fenetre").unwrap();
        assert_eq!(fenetre.retenu, Some(r("Ctrl+Maj+3")));
        assert_eq!(
            fenetre.ecartes,
            vec![Ecarte {
                candidat: "ctrl + maj + 1".into(),
                raison: Raison::DejaPrisParUneAutreAction("zone".into())
            }]
        );
    }

    #[test]
    fn refus_pas_redemande() {
        let t = table(&[
            ("zone", &["Ctrl+Maj+1", "Ctrl+Maj+1", "Ctrl+Maj+4"]),
            ("plein_ecran", &["Ctrl+Maj+1"]),
        ]);
        let mut essais = Vec::new();
        let b = choisir(&t, false, |x| {
            essais.push(x.canonique());
            *x != r("Ctrl+Maj+1")
        });
        assert_eq!(essais, ["Ctrl+Maj+1", "Ctrl+Maj+4"]);
        assert_eq!(b.retenu("zone"), Some(&r("Ctrl+Maj+4")));
        assert_eq!(b.retenu("plein_ecran"), None);
    }

    #[test]
    fn actions_inconnues_apres_les_connues() {
        let t = table(&[
            ("zz_autre", &["Ctrl+Maj+9"]),
            ("annoter", &["Ctrl+Maj+1", "Ctrl+Maj+8"]),
            ("fenetre", &["Ctrl+Maj+3"]),
            ("zone", &["Ctrl+Maj+1"]),
            ("plein_ecran", &["Ctrl+Maj+2"]),
        ]);
        let b = choisir(&t, false, |_| true);
        let noms: Vec<_> = b.actions.iter().map(|a| a.action.as_str()).collect();
        assert_eq!(
            noms,
            ["zone", "plein_ecran", "fenetre", "annoter", "zz_autre"]
        );
        // Une action inconnue est traitée comme les autres, mais ne prend rien aux connues.
        assert_eq!(b.retenu("annoter"), Some(&r("Ctrl+Maj+8")));
        assert_eq!(b.retenu("zone"), Some(&r("Ctrl+Maj+1")));
        assert!(b.action("absente").is_none());
        assert_eq!(b.libelle_menu("absente"), None);
    }

    #[test]
    fn liste_vide() {
        let t = table(&[("zone", &[]), ("fenetre", &["Ctrl+Maj+3"])]);
        let b = choisir(&t, false, |_| true);
        let zone = b.action("zone").unwrap();
        assert_eq!(zone.retenu, None);
        assert!(zone.ecartes.is_empty());
        assert!(!zone.premier_ecarte());
        assert!(choisir(&BTreeMap::new(), true, |_| true).actions.is_empty());
    }

    #[test]
    fn une_action_a_la_fois() {
        let pris = vec![(r("Ctrl+Maj+1"), "zone".to_string())];
        let liste = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        // Déjà pris par une autre action : jamais essayé.
        let mut essais = Vec::new();
        let c = choisir_action(
            "plein_ecran",
            &liste(&["Ctrl+Maj+1"]),
            &pris,
            Some(r("Ctrl+Maj+2")),
            false,
            |x| {
                essais.push(x.canonique());
                true
            },
        );
        assert_eq!(c.retenu, None);
        assert!(essais.is_empty());
        assert_eq!(
            c.ecartes[0].raison,
            Raison::DejaPrisParUneAutreAction("zone".into())
        );
        // Le raccourci actuel est gardé sans redemander au système.
        let c = choisir_action(
            "plein_ecran",
            &liste(&["ctrl + maj + 2"]),
            &pris,
            Some(r("Ctrl+Maj+2")),
            false,
            |_| panic!("ne doit pas être essayé"),
        );
        assert_eq!(c.retenu, Some(r("Ctrl+Maj+2")));
        // Refusé par le système, puis repli.
        let c = choisir_action(
            "plein_ecran",
            &liste(&["Ctrl+Maj+7", "Ctrl+Maj+8"]),
            &pris,
            None,
            false,
            |x| *x != r("Ctrl+Maj+7"),
        );
        assert_eq!(c.retenu, Some(r("Ctrl+Maj+8")));
        assert_eq!(c.ecartes[0].raison, Raison::RefuseParLeSysteme);
        // Impr. écran réservée par Windows.
        let c = choisir_action("zone", &liste(&["ImprEcran"]), &[], None, true, |_| true);
        assert_eq!(c.retenu, None);
        assert_eq!(c.ecartes[0].raison, Raison::ReserveParLeSysteme);
    }
}
