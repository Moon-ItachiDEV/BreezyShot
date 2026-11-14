//! Pages sous un clic (S4, D2, §18), règles pures de la recherche : le suivi des réponses
//! d'UI Automation (une première liste à `BUDGET_PAGES`, puis les pages tardives jusqu'à
//! `PAGES_TARDIVES_MAX`), ce qui n'est pas fouillé, et les lignes du journal qui disent ce
//! que la recherche a coûté, fenêtre par fenêtre (classes, nombres et codes seulement).
//!
//! Une liste proposée n'est jamais changée par la suivante : les pages d'une fenêtre qui
//! répond en retard sont **ajoutées à la fin**. Un indice donné à la Sélection reste donc la
//! même page (un clic décidé sur une liste vise la même page dans la suivante), et la page
//! survolée ne change pas. L'ordre entre fenêtres ne sert pas à viser : la page d'une
//! fenêtre est déjà privée de ce que les fenêtres de devant couvrent (`assembler`), deux
//! fenêtres n'ont donc jamais de page au même endroit ; dans une fenêtre, la plus petite
//! reste la première.

#![cfg_attr(not(windows), allow(dead_code))]

use std::time::Duration;

use super::{pages_de_fenetre, PageTrouvee, Rect, BUDGET_PAGES};

/// Les pages trouvées par UI Automation après `BUDGET_PAGES` sont encore proposées jusqu'à
/// ce délai après le début de la recherche ; au-delà, plus rien n'est attendu. Même valeur
/// que la recherche du défileur (`RECHERCHE_MAX`, Chromium : 0,3 à 0,4 s pour ouvrir son
/// arbre, trois essais) : la visée dure en pratique plus longtemps (R1), une page qui vient
/// à 1 s sert encore.
pub const PAGES_TARDIVES_MAX: Duration = Duration::from_millis(1500);

/// Classes de fenêtre jamais fouillées par UI Automation : la barre des tâches (son arbre
/// XAML est long à parcourir et n'a jamais de page à capturer).
const CLASSES_ECARTEES: [&str; 2] = ["Shell_TrayWnd", "Shell_SecondaryTrayWnd"];

/// Vrai si la fenêtre de premier niveau de classe `classe` n'est pas fouillée par UI
/// Automation.
pub fn ecartee(classe: &str) -> bool {
    CLASSES_ECARTEES.contains(&classe)
}

/// Types de contrôle d'UI Automation (`UIA_*ControlTypeId`, valeurs d'UIAutomationClient.h)
/// dont les enfants ne sont jamais regardés : ce sont des feuilles, ou des éléments dont
/// l'intérieur n'est jamais « la page » (une image, un texte, un bouton, une ligne de liste,
/// une barre de défilement…). Un élément qui défile est gardé avant ce test.
const SANS_ENFANTS_A_FOUILLER: [i32; 22] = [
    50000, // Button
    50002, // CheckBox
    50003, // ComboBox
    50004, // Edit
    50005, // Hyperlink
    50006, // Image
    50007, // ListItem
    50011, // MenuItem
    50012, // ProgressBar
    50013, // RadioButton
    50014, // ScrollBar
    50015, // Slider
    50016, // Spinner
    50019, // TabItem
    50020, // Text
    50022, // ToolTip
    50024, // TreeItem
    50027, // Thumb
    50029, // DataItem
    50031, // SplitButton
    50035, // HeaderItem
    50038, // Separator
];

/// Vrai si les enfants d'un élément de type `type_controle` (qui ne défile pas) sont à
/// regarder. Un type inconnu (0, illisible) est fouillé.
pub fn a_fouiller(type_controle: i32) -> bool {
    !SANS_ENFANTS_A_FOUILLER.contains(&type_controle)
}

/// Le suivi d'une recherche : les pages reçues de chaque fenêtre (premier plan d'abord), et
/// la liste déjà proposée.
#[derive(Debug, Clone)]
pub struct Suivi {
    bords: Vec<Rect>,
    /// `None` : la fenêtre n'a pas encore répondu.
    recues: Vec<Option<Vec<PageTrouvee>>>,
    liste: Vec<PageTrouvee>,
    proposee: bool,
    /// Des pages ont été ajoutées depuis la dernière proposition.
    ajout: bool,
}

impl Suivi {
    /// `bords[i]` : le rectangle de la fenêtre `i` ; `recues[i]` : ses pages si elles sont
    /// déjà connues (fenêtres filles, fenêtre sans HWND ou écartée : `Some`), `None` si UI
    /// Automation doit encore répondre.
    pub fn nouveau(bords: Vec<Rect>, mut recues: Vec<Option<Vec<PageTrouvee>>>) -> Suivi {
        recues.resize(bords.len(), Some(Vec::new()));
        Suivi {
            bords,
            recues,
            liste: Vec::new(),
            proposee: false,
            ajout: false,
        }
    }

    /// Les pages de la fenêtre `i`, arrivées `ecoule` après le début. Ignorées si la
    /// fenêtre a déjà répondu, si `i` est hors de la liste, ou après `PAGES_TARDIVES_MAX`.
    /// Après la première proposition, elles s'ajoutent à la fin de la liste.
    pub fn recevoir(&mut self, i: usize, pages: Vec<PageTrouvee>, ecoule: Duration) {
        if ecoule >= PAGES_TARDIVES_MAX {
            return;
        }
        let Some(place) = self.recues.get_mut(i).filter(|r| r.is_none()) else {
            return;
        };
        if self.proposee {
            let ajoutees = pages_de_fenetre(&self.bords, i, &pages);
            self.ajout |= !ajoutees.is_empty();
            self.liste.extend(ajoutees);
        }
        *place = Some(pages);
    }

    /// Les fenêtres qui n'ont pas encore répondu.
    pub fn en_attente(&self) -> usize {
        self.recues.iter().filter(|r| r.is_none()).count()
    }

    /// Ce qu'il faut proposer `ecoule` après le début : la première liste dès que toutes
    /// les fenêtres ont répondu, au plus tard à `BUDGET_PAGES` (même vide : D2, la ligne
    /// `defilante pages`) ; ensuite, la liste grandie de pages tardives, seulement si elle a
    /// grandi. `None` : rien de nouveau.
    pub fn proposer(&mut self, ecoule: Duration) -> Option<Vec<PageTrouvee>> {
        if !self.proposee {
            if self.en_attente() > 0 && ecoule < BUDGET_PAGES {
                return None;
            }
            self.proposee = true;
            self.ajout = false;
            self.liste = self
                .recues
                .iter()
                .enumerate()
                .filter_map(|(i, r)| r.as_ref().map(|p| pages_de_fenetre(&self.bords, i, p)))
                .flatten()
                .collect();
            return Some(self.liste.clone());
        }
        if !self.ajout {
            return None;
        }
        self.ajout = false;
        Some(self.liste.clone())
    }

    /// Le prochain instant (depuis le début) où `proposer` peut avoir changé d'avis sans
    /// nouvelle réponse : `BUDGET_PAGES` avant la première proposition, puis
    /// `PAGES_TARDIVES_MAX`.
    pub fn echeance(&self) -> Duration {
        if self.proposee {
            PAGES_TARDIVES_MAX
        } else {
            BUDGET_PAGES
        }
    }

    /// Vrai quand plus rien ne sera proposé : la première liste est partie, et toutes les
    /// fenêtres ont répondu ou `PAGES_TARDIVES_MAX` est passé.
    pub fn fini(&self, ecoule: Duration) -> bool {
        self.proposee && !self.ajout && (self.en_attente() == 0 || ecoule >= PAGES_TARDIVES_MAX)
    }

    /// Plus personne ne répondra (le bassin d'UI Automation a disparu) : les fenêtres en
    /// attente comptent comme sans page.
    pub fn abandonner(&mut self) {
        for r in self.recues.iter_mut().filter(|r| r.is_none()) {
            *r = Some(Vec::new());
        }
    }
}

// ---------------------------------------------------------------------------------------
// Journal.

/// Où en était UI Automation (le bassin de `windows.rs`) quand la recherche a commencé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EtatAutomation {
    /// Aucune fenêtre ne lui a été demandée.
    Inutile,
    /// Un client est prêt depuis `depuis` ; COM et la création du client ont pris `com` et
    /// `client`.
    Prete {
        com: Duration,
        client: Duration,
        depuis: Duration,
    },
    /// Le premier client se prépare, depuis `depuis`.
    EnPreparation { depuis: Duration },
    /// Aucun client possible (code).
    Impossible(String),
}

/// Ce que la recherche a vu d'une fenêtre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Regard {
    /// Pas de HWND (fenêtre de la liste sans poignée) : rien de regardé.
    SansFenetre,
    /// Ses fenêtres filles ont donné `pages` pages : UI Automation n'est pas demandé.
    Filles { pages: usize },
    /// Écartée par sa classe (`ecartee`).
    Ecartee,
    /// Demandée à UI Automation, pas encore commencée (tous les fils occupés, ou client pas
    /// prêt).
    EnFile,
    /// Commencée `depuis` après le début de la recherche, pas finie.
    EnCours { depuis: Duration },
    /// Finie : `duree` de fouille, finie `fin` après le début de la recherche ;
    /// `elements` regardés, `noeuds` ouverts, `pages` trouvées ; `coupee` : arrêtée par son
    /// échéance ou ses bornes avant la fin de l'arbre.
    Automation {
        duree: Duration,
        fin: Duration,
        elements: u32,
        noeuds: u32,
        pages: usize,
        coupee: bool,
    },
    /// UI Automation a refusé la fenêtre (code), après `duree`.
    Erreur { duree: Duration, code: String },
}

impl Regard {
    /// Vrai si la fenêtre attend encore UI Automation.
    pub fn en_retard(&self) -> bool {
        matches!(self, Regard::EnFile | Regard::EnCours { .. })
    }

    /// Vrai si UI Automation y a trouvé au moins une page.
    pub fn par_automation(&self) -> bool {
        matches!(self, Regard::Automation { pages, .. } if *pages > 0)
    }
}

/// Une fenêtre regardée : sa classe (déjà coupée pour le journal) et ce qui en a été vu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vue {
    pub classe: String,
    pub regard: Regard,
}

/// Ce qui a précédé la recherche dans la fenêtre : lancement du fil `defilante-pages`
/// (`fil`), liste des fenêtres (`liste`), fenêtres filles (`filles`), et l'état d'UI
/// Automation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lancement {
    pub fil: Duration,
    pub liste: Duration,
    pub filles: Duration,
    pub automation: EtatAutomation,
    /// Clients d'UI Automation ratés depuis la création du bassin, et le dernier code.
    pub rates: Option<(u32, String)>,
}

/// `UIA_E_ELEMENTNOTAVAILABLE` : la fenêtre ou l'élément n'existe plus.
const ELEMENT_DISPARU: u32 = 0x8004_0201;

/// Une fouille refusée par UI Automation (`code`) est-elle refaite une fois, avec un client
/// neuf ? Oui, sauf si la fenêtre n'existe plus (`existe` faux, ou
/// `UIA_E_ELEMENTNOTAVAILABLE`) ou si l'échéance est passée (`reste` faux) : un client
/// devenu inutilisable rend `E_FAIL` ou une erreur de RPC tout de suite, pour chaque
/// fenêtre.
pub fn refaire_avec_un_client_neuf(code: u32, existe: bool, reste: bool) -> bool {
    existe && reste && code != ELEMENT_DISPARU
}

fn ms(d: Duration) -> u128 {
    d.as_millis()
}

/// Les compteurs de la ligne principale : (filles, automation, en retard).
fn compter(vues: &[Vue]) -> (usize, usize, usize) {
    let filles = vues
        .iter()
        .filter(|v| matches!(v.regard, Regard::Filles { pages } if pages > 0))
        .count();
    let automation = vues.iter().filter(|v| v.regard.par_automation()).count();
    let en_retard = vues.iter().filter(|v| v.regard.en_retard()).count();
    (filles, automation, en_retard)
}

/// `defiler pages: <n> (filles <f>, automation <a>, en retard <r>) sur <k> fenetres en
/// <ms> ms` : la première liste (forme lue par l'essai 14).
pub fn ligne_pages(n: usize, vues: &[Vue], duree: Duration) -> String {
    let (filles, automation, en_retard) = compter(vues);
    format!(
        "defiler pages: {n} (filles {filles}, automation {automation}, en retard {en_retard}) \
         sur {} fenetres en {} ms",
        vues.len(),
        ms(duree)
    )
}

/// `defiler pages tardives: <n> (automation <a>, en retard <r>) en <ms> ms` : la liste a
/// grandi de pages venues après le budget (`n` : toute la liste).
pub fn ligne_tardives(n: usize, vues: &[Vue], duree: Duration) -> String {
    let (_, automation, en_retard) = compter(vues);
    format!(
        "defiler pages tardives: {n} (automation {automation}, en retard {en_retard}) en {} ms",
        ms(duree)
    )
}

fn texte_automation(e: &EtatAutomation) -> String {
    match e {
        EtatAutomation::Inutile => "inutile".into(),
        EtatAutomation::Prete {
            com,
            client,
            depuis,
        } => format!(
            "prete depuis {} ms (com {} ms, client {} ms)",
            ms(*depuis),
            ms(*com),
            ms(*client)
        ),
        EtatAutomation::EnPreparation { depuis } => {
            format!("en preparation depuis {} ms", ms(*depuis))
        }
        EtatAutomation::Impossible(code) => format!("impossible ({code})"),
    }
}

fn texte_regard(r: &Regard) -> String {
    match r {
        Regard::SansFenetre => "sans fenetre".into(),
        Regard::Filles { pages } => format!("filles {pages}"),
        Regard::Ecartee => "ecartee".into(),
        Regard::EnFile => "en retard (pas commencee)".into(),
        Regard::EnCours { depuis } => format!("en retard (commencee a {} ms)", ms(*depuis)),
        Regard::Automation {
            duree,
            fin,
            elements,
            noeuds,
            pages,
            coupee,
        } => format!(
            "automation {} ms (fin a {} ms) {elements} elements {noeuds} noeuds {pages} \
             pages{}",
            ms(*duree),
            ms(*fin),
            if *coupee { " coupee" } else { "" }
        ),
        Regard::Erreur { duree, code } => {
            format!("automation erreur {code} en {} ms", ms(*duree))
        }
    }
}

fn texte_vues(vues: &[Vue]) -> String {
    vues.iter()
        .enumerate()
        .map(|(i, v)| format!("{} {} {}", i + 1, v.classe, texte_regard(&v.regard)))
        .collect::<Vec<_>>()
        .join(" ; ")
}

/// `defiler pages detail: fil <ms> ms, liste <ms> ms, filles <ms> ms, automation <état> ;
/// <rang> <classe> <regard> ; …` : écrite avec la première liste. Le rang compte depuis
/// 1, du premier plan vers le fond.
pub fn ligne_detail(lancement: &Lancement, vues: &[Vue]) -> String {
    let mut l = format!(
        "defiler pages detail: fil {} ms, liste {} ms, filles {} ms, automation {}",
        ms(lancement.fil),
        ms(lancement.liste),
        ms(lancement.filles),
        texte_automation(&lancement.automation)
    );
    if let Some((n, dernier)) = &lancement.rates {
        l.push_str(&format!(", clients rates {n} (dernier {dernier})"));
    }
    if !vues.is_empty() {
        l.push_str(" ; ");
        l.push_str(&texte_vues(vues));
    }
    l
}

/// `defiler pages fin: <n> (automation <a>, en retard <r>) en <ms> ms ; <rang> <classe>
/// <regard> ; …` : la fin d'une recherche qui avait des fenêtres en retard au budget.
pub fn ligne_fin(n: usize, vues: &[Vue], duree: Duration) -> String {
    let (_, automation, en_retard) = compter(vues);
    format!(
        "defiler pages fin: {n} (automation {automation}, en retard {en_retard}) en {} ms ; {}",
        ms(duree),
        texte_vues(vues)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, largeur: i32, hauteur: i32) -> Rect {
        Rect {
            x,
            y,
            largeur,
            hauteur,
        }
    }

    fn p(rect: Rect, fenetre: isize) -> PageTrouvee {
        PageTrouvee { rect, fenetre }
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// Trois fenêtres côte à côte (rien ne se couvre) : la 0 et la 2 attendent UI
    /// Automation, la 1 a sa page fille.
    fn suivi() -> Suivi {
        let bords = vec![r(0, 0, 400, 400), r(500, 0, 400, 400), r(1000, 0, 400, 400)];
        Suivi::nouveau(
            bords,
            vec![None, Some(vec![p(r(510, 10, 300, 300), 11)]), None],
        )
    }

    #[test]
    fn rien_avant_le_budget_tant_qu_une_fenetre_manque() {
        let mut s = suivi();
        assert_eq!(s.proposer(ms(0)), None);
        s.recevoir(2, vec![p(r(1010, 10, 200, 200), 2)], ms(50));
        assert_eq!(s.proposer(ms(100)), None);
        assert_eq!(s.en_attente(), 1);
        assert!(!s.fini(ms(100)));
        assert_eq!(s.echeance(), BUDGET_PAGES);
    }

    #[test]
    fn tout_recu_avant_le_budget_propose_aussitot_dans_l_ordre_des_fenetres() {
        let mut s = suivi();
        s.recevoir(2, vec![p(r(1010, 10, 200, 200), 2)], ms(20));
        s.recevoir(0, vec![p(r(10, 10, 100, 100), 1)], ms(30));
        let l = s.proposer(ms(31)).expect("liste");
        assert_eq!(
            l.iter().map(|p| p.fenetre).collect::<Vec<_>>(),
            vec![1, 11, 2]
        );
        assert!(s.fini(ms(31)));
        assert_eq!(s.proposer(ms(40)), None);
    }

    #[test]
    fn au_budget_la_liste_part_meme_vide() {
        let bords = vec![r(0, 0, 400, 400)];
        let mut s = Suivi::nouveau(bords, vec![None]);
        assert_eq!(s.proposer(BUDGET_PAGES), Some(Vec::new()));
        assert!(!s.fini(BUDGET_PAGES));
        assert_eq!(s.echeance(), PAGES_TARDIVES_MAX);
    }

    #[test]
    fn une_page_tardive_s_ajoute_a_la_fin_sans_rien_changer_devant() {
        let mut s = suivi();
        let premiere = s.proposer(BUDGET_PAGES).expect("première liste");
        assert_eq!(premiere, vec![p(r(510, 10, 300, 300), 11)]);
        // La fenêtre du premier plan répond en retard : ses pages vont à la fin, la page
        // déjà proposée garde son indice.
        s.recevoir(
            0,
            vec![p(r(10, 10, 300, 300), 1), p(r(20, 20, 100, 100), 1)],
            ms(700),
        );
        let seconde = s.proposer(ms(700)).expect("liste grandie");
        assert_eq!(&seconde[..premiere.len()], &premiere[..]);
        // Dans la fenêtre, la plus petite d'abord.
        assert_eq!(
            seconde[1..].iter().map(|p| p.rect).collect::<Vec<_>>(),
            vec![r(20, 20, 100, 100), r(10, 10, 300, 300)]
        );
        // Rien de nouveau : rien à proposer ; la dernière fenêtre manque encore.
        assert_eq!(s.proposer(ms(710)), None);
        assert!(!s.fini(ms(710)));
        // Une réponse sans page ne fait pas de nouvelle proposition, et finit la recherche.
        s.recevoir(2, Vec::new(), ms(900));
        assert_eq!(s.proposer(ms(900)), None);
        assert!(s.fini(ms(900)));
    }

    #[test]
    fn une_page_tardive_reste_coupee_par_les_fenetres_de_devant() {
        // La fenêtre 1 est derrière la 0 et dépasse à droite : sa page tardive est privée
        // de ce que la 0 couvre.
        let bords = vec![r(0, 0, 400, 400), r(200, 0, 400, 400)];
        let mut s = Suivi::nouveau(bords, vec![Some(Vec::new()), None]);
        assert_eq!(s.proposer(BUDGET_PAGES), Some(Vec::new()));
        s.recevoir(1, vec![p(r(200, 0, 400, 400), 7)], ms(400));
        assert_eq!(s.proposer(ms(400)), Some(vec![p(r(400, 0, 200, 400), 7)]));
    }

    #[test]
    fn rien_apres_la_borne_des_tardives() {
        let mut s = suivi();
        let premiere = s.proposer(BUDGET_PAGES).expect("liste");
        s.recevoir(0, vec![p(r(10, 10, 300, 300), 1)], PAGES_TARDIVES_MAX);
        assert_eq!(s.proposer(PAGES_TARDIVES_MAX), None);
        assert!(s.fini(PAGES_TARDIVES_MAX));
        assert_eq!(premiere.len(), 1);
    }

    #[test]
    fn une_fenetre_ne_repond_qu_une_fois() {
        let mut s = suivi();
        s.proposer(BUDGET_PAGES);
        s.recevoir(0, vec![p(r(10, 10, 300, 300), 1)], ms(400));
        s.recevoir(0, vec![p(r(10, 10, 200, 200), 1)], ms(410));
        s.recevoir(9, vec![p(r(10, 10, 200, 200), 1)], ms(410));
        assert_eq!(s.proposer(ms(420)).map(|l| l.len()), Some(2));
    }

    #[test]
    fn abandonner_finit_la_recherche() {
        let mut s = suivi();
        s.proposer(BUDGET_PAGES);
        assert!(!s.fini(ms(400)));
        s.abandonner();
        assert!(s.fini(ms(400)));
        assert_eq!(s.proposer(ms(400)), None);
    }

    #[test]
    fn une_fouille_refusee_est_refaite_une_fois_avec_un_client_neuf() {
        // E_FAIL, RPC coupé, objet déconnecté : refaite.
        for code in [0x8000_4005, 0x8001_0108, 0x8007_06BA, 0x8001_0012] {
            assert!(refaire_avec_un_client_neuf(code, true, true), "{code:#x}");
        }
        // Fenêtre partie, ou plus de temps : non.
        assert!(!refaire_avec_un_client_neuf(ELEMENT_DISPARU, true, true));
        assert!(!refaire_avec_un_client_neuf(0x8000_4005, false, true));
        assert!(!refaire_avec_un_client_neuf(0x8000_4005, true, false));
    }

    #[test]
    fn fenetres_ecartees_et_types_sans_enfants() {
        assert!(ecartee("Shell_TrayWnd"));
        assert!(ecartee("Shell_SecondaryTrayWnd"));
        assert!(!ecartee("HwndWrapper"));
        assert!(!ecartee("ConsoleWindowClass"));
        // Image, Text, Button, ScrollBar, ListItem : feuilles ; Pane, Window, Document,
        // Group, List, Custom, inconnu : fouillés.
        for t in [50006, 50020, 50000, 50014, 50007] {
            assert!(!a_fouiller(t), "{t}");
        }
        for t in [50033, 50032, 50030, 50026, 50008, 50025, 0] {
            assert!(a_fouiller(t), "{t}");
        }
    }

    fn vues() -> Vec<Vue> {
        vec![
            Vue {
                classe: "HwndWrapper".into(),
                regard: Regard::Automation {
                    duree: ms(412),
                    fin: ms(430),
                    elements: 3,
                    noeuds: 2,
                    pages: 1,
                    coupee: false,
                },
            },
            Vue {
                classe: "Shell_TrayWnd".into(),
                regard: Regard::Ecartee,
            },
            Vue {
                classe: "HwndWrapper".into(),
                regard: Regard::EnCours { depuis: ms(15) },
            },
            Vue {
                classe: "ConsoleWindowClass".into(),
                regard: Regard::EnFile,
            },
            Vue {
                classe: "Chrome_WidgetWin_1".into(),
                regard: Regard::Filles { pages: 2 },
            },
            Vue {
                classe: "aucune".into(),
                regard: Regard::Erreur {
                    duree: ms(3),
                    code: "0x80040201".into(),
                },
            },
        ]
    }

    #[test]
    fn lignes_du_journal() {
        let v = vues();
        assert_eq!(
            ligne_pages(3, &v, ms(300)),
            "defiler pages: 3 (filles 1, automation 1, en retard 2) sur 6 fenetres en 300 ms"
        );
        assert_eq!(
            ligne_tardives(4, &v, ms(812)),
            "defiler pages tardives: 4 (automation 1, en retard 2) en 812 ms"
        );
        let lancement = Lancement {
            fil: ms(1),
            liste: ms(4),
            filles: ms(2),
            automation: EtatAutomation::Prete {
                com: ms(1),
                client: ms(35),
                depuis: ms(180),
            },
            rates: None,
        };
        assert_eq!(
            ligne_detail(&lancement, &v),
            "defiler pages detail: fil 1 ms, liste 4 ms, filles 2 ms, automation prete depuis \
             180 ms (com 1 ms, client 35 ms) ; 1 HwndWrapper automation 412 ms (fin a 430 ms) \
             3 elements 2 noeuds 1 pages ; 2 Shell_TrayWnd ecartee ; 3 HwndWrapper en retard \
             (commencee a 15 ms) ; 4 ConsoleWindowClass en retard (pas commencee) ; 5 \
             Chrome_WidgetWin_1 filles 2 ; 6 aucune automation erreur 0x80040201 en 3 ms"
        );
        let sans = Lancement {
            automation: EtatAutomation::EnPreparation { depuis: ms(20) },
            ..lancement.clone()
        };
        assert!(ligne_detail(&sans, &[]).ends_with("automation en preparation depuis 20 ms"));
        let ratee = Lancement {
            rates: Some((3, "client CoCreateInstance 0x80004005".into())),
            ..lancement.clone()
        };
        assert!(ligne_detail(&ratee, &[]).ends_with(
            "client 35 ms), clients rates 3 (dernier client CoCreateInstance 0x80004005)"
        ));
        let impossible = Lancement {
            automation: EtatAutomation::Impossible("CoInitializeEx 0x80010106".into()),
            ..lancement
        };
        assert!(ligne_detail(&impossible, &[]).ends_with("impossible (CoInitializeEx 0x80010106)"));
        assert!(ligne_fin(4, &v, ms(1500))
            .starts_with("defiler pages fin: 4 (automation 1, en retard 2) en 1500 ms ; 1 "));
        // Coupée : dite.
        let coupee = [Vue {
            classe: "HwndWrapper".into(),
            regard: Regard::Automation {
                duree: ms(1490),
                fin: ms(1500),
                elements: 300,
                noeuds: 48,
                pages: 0,
                coupee: true,
            },
        }];
        assert!(ligne_fin(0, &coupee, ms(1500)).ends_with("0 pages coupee"));
        // La forme lue par l'essai 14 : « en retard <n>) » et « en <ms> ms » en fin.
        let l = ligne_pages(0, &v, ms(300));
        assert!(l.contains("en retard 2)") && l.ends_with(" en 300 ms"));
    }
}
