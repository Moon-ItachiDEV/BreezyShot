//! Copier le texte, côté décisions (contrat 0.5, §5.4 à §5.6) : fonctions pures, sans
//! Tauri ni système. Les gestes sont dans `geste_texte.rs`.
//!
//! Le texte lu ne sort jamais d'ici vers le journal ni vers une page : les lignes du
//! journal ne portent que des nombres et des codes, la page de l'éditeur ne reçoit que les
//! boîtes des mots.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;

use super::etats::Declencheur;
use crate::fenetres::{Action, Genre, Message};
use crate::sortie::Etape;
use crate::systeme::texte::Disponibilite;
use crate::texte::{extrait, Issue, Lu, PETITE_IMAGE};

/// Vrai quand Copier le texte est construit (chantier E) : seulement alors l'action
/// `texte` est construite (si une licence est active et qu'une langue existe).
pub const CONSTRUIT: bool = true;

/// Le balayage sur la zone lue, et sa version réduite.
pub const BALAYAGE: Duration = Duration::from_millis(400);
pub const BALAYAGE_REDUIT: Duration = Duration::from_millis(120);

/// L'écran figé n'est jamais gardé plus longtemps après le relâcher.
pub const ECRAN_FIGE_MAX_LECTURE: Duration = Duration::from_millis(1500);

/// L'action du bandeau « Aucun texte trouvé ».
pub const ACTION_REESSAYER: &str = "reessayer";
pub const LIBELLE_REESSAYER: &str = "Réessayer";

/// Les titres du bandeau.
pub const TITRE_COPIE: &str = "Texte copié";
pub const TITRE_AUCUN_TEXTE: &str = "Aucun texte trouvé";
pub const TITRE_LECTURE_IMPOSSIBLE: &str = "Impossible de lire le texte";
pub const TITRE_COPIE_IMPOSSIBLE: &str = "Impossible de copier";
/// « Aucun texte trouvé » part seul après ce délai (croix gardée, U5).
pub const DUREE_AUCUN_TEXTE_MS: u32 = 4000;

/// Événements vers la page de l'éditeur (§6.3).
pub const EVENEMENT_EDITEUR_TEXTE: &str = "editeur:texte";
pub const EVENEMENT_EDITEUR_TEXTE_COPIE: &str = "editeur:texte_copie";

/// Cause d'une lecture dont l'échéance est tombée.
pub const CAUSE_ECHEANCE: &str = "echeance";
/// Cause d'une lecture dont un fil n'a pas pu être lancé ou s'est arrêté (panique).
pub const CAUSE_FIL: &str = "fil";

/// La lecture en cours depuis le raccourci (état Lecture), du relâcher au résultat.
#[derive(Debug, Clone, PartialEq)]
pub struct LectureEnCours {
    pub numero: u64,
    pub relache: Instant,
    /// L'écran figé reste au moins ce temps après le relâcher (`balayage`).
    pub minimum: Duration,
    /// Le résultat arrivé avant `minimum` : montré quand l'écran se libère.
    pub fin: Option<Box<FinLecture>>,
}

/// Ce que le fil `texte` rend au fil principal.
#[derive(Debug, Clone, PartialEq)]
pub struct FinLecture {
    pub numero: u64,
    pub issue: Issue,
    /// La copie du texte assemblé, si un texte a été lu (raccourci seulement).
    pub copie: Option<Etape>,
    /// La lecture seule (copie non comprise).
    pub duree: Duration,
    /// `Some(id)` : lecture du bouton « Texte » de l'éditeur, document `id` ; `None` :
    /// raccourci ou menu.
    pub document: Option<u64>,
    /// Le facteur de la préparation (`facteur_de`), pour le journal.
    pub facteur: f32,
}

/// Ce que devient le résultat d'une lecture du raccourci à son arrivée.
#[derive(Debug, Clone, PartialEq)]
pub enum Arrivee {
    /// Pas la lecture en cours (finie, oubliée, ou une autre) : ignoré.
    Perimee,
    /// Arrivé avant la fin du balayage : gardé jusqu'à `balayage_fini`.
    Gardee,
    /// À montrer tout de suite (`finir`).
    Finir(FinLecture),
}

/// Le résultat `fin` arrive, `maintenant` (machine pure de l'état Lecture).
pub fn arrivee(
    lecture: &mut Option<LectureEnCours>,
    fin: FinLecture,
    maintenant: Instant,
) -> Arrivee {
    let Some(x) = lecture
        .as_mut()
        .filter(|x| x.numero == fin.numero && x.fin.is_none())
    else {
        return Arrivee::Perimee;
    };
    let ecoule = maintenant.saturating_duration_since(x.relache);
    if liberer_ecran(ecoule, x.minimum, true) {
        Arrivee::Finir(fin)
    } else {
        x.fin = Some(Box::new(fin));
        Arrivee::Gardee
    }
}

/// Le balayage de la lecture `numero` est fini : son résultat gardé, s'il y en a un.
pub fn balayage_fini(lecture: &mut Option<LectureEnCours>, numero: u64) -> Option<FinLecture> {
    lecture
        .as_mut()
        .filter(|x| x.numero == numero)
        .and_then(|x| x.fin.take())
        .map(|f| *f)
}

/// L'échéance (ou l'arrêt d'un fil, `cause`) de la lecture `numero` : l'échec à montrer,
/// si aucun résultat n'est arrivé ; un résultat gardé l'emporte.
pub fn echeance(lecture: &Option<LectureEnCours>, numero: u64, cause: &str) -> Option<FinLecture> {
    lecture
        .as_ref()
        .filter(|x| x.numero == numero && x.fin.is_none())
        .map(|_| FinLecture {
            numero,
            issue: Issue::Echec(cause.into()),
            copie: None,
            duree: crate::texte::DELAI_LECTURE,
            document: None,
            facteur: 1.0,
        })
}

/// Retire la lecture `numero` (sa fin est montrée) ; `None` si ce n'est pas elle.
pub fn terminer(lecture: &mut Option<LectureEnCours>, numero: u64) -> Option<LectureEnCours> {
    lecture.take_if(|x| x.numero == numero)
}

/// Qui rend le résultat de la lecture attendue : le fil `texte` (et alors il copie) ou
/// l'échéance, jamais les deux. `poser` à la zone choisie ; le premier `prendre` gagne.
#[derive(Debug, Default)]
pub struct Attendue(AtomicU64);

impl Attendue {
    pub const fn new() -> Attendue {
        Attendue(AtomicU64::new(0))
    }

    /// La lecture `numero` est attendue (0 : aucune).
    pub fn poser(&self, numero: u64) {
        self.0.store(numero, Ordering::SeqCst);
    }

    /// Vrai pour le premier qui prend la lecture `numero` ; ensuite aucune n'est attendue.
    pub fn prendre(&self, numero: u64) -> bool {
        numero != 0
            && self
                .0
                .compare_exchange(numero, 0, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
    }

    /// Plus rien n'est attendu (fin montrée, arrêt).
    pub fn oublier(&self) {
        self.0.store(0, Ordering::SeqCst);
    }
}

/// Un code court ASCII pour le journal à partir d'une cause (une phrase devient
/// `presse_papiers`).
pub fn code_court(cause: &str) -> &str {
    let court = !cause.is_empty()
        && cause.len() <= 32
        && cause
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if court {
        cause
    } else {
        "presse_papiers"
    }
}

/// La durée du balayage (minimum de l'écran figé).
pub fn balayage(reduites: bool) -> Duration {
    if reduites {
        BALAYAGE_REDUIT
    } else {
        BALAYAGE
    }
}

/// Vrai si l'écran figé doit être libéré, `ecoule` après le relâcher : à la borne
/// `ECRAN_FIGE_MAX_LECTURE` toujours, sinon une fois le balayage fini et la lecture rendue.
pub fn liberer_ecran(ecoule: Duration, minimum: Duration, rendue: bool) -> bool {
    ecoule >= ECRAN_FIGE_MAX_LECTURE || (rendue && ecoule >= minimum.min(ECRAN_FIGE_MAX_LECTURE))
}

/// Le facteur que `texte::preparer` appliquera à une image `l × h` (même règle : ×2 sous
/// `PETITE_IMAGE` si le double tient, réduite au-delà de `dimension_max`), pour le journal.
pub fn facteur_de(l: u32, h: u32, dimension_max: u32) -> f32 {
    let grand = l.max(h);
    if dimension_max == 0 || grand == 0 {
        return 1.0;
    }
    if u64::from(l) * u64::from(h) <= PETITE_IMAGE
        && u64::from(grand) * 2 <= u64::from(dimension_max)
    {
        2.0
    } else if grand > dimension_max {
        dimension_max as f32 / grand as f32
    } else {
        1.0
    }
}

/// Le déclencheur de la fin d'une lecture du raccourci.
pub fn declencheur(issue: &Issue) -> Declencheur {
    match issue {
        Issue::Texte(_) | Issue::Vide => Declencheur::TexteLu,
        Issue::AucuneLangue | Issue::Echec(_) => Declencheur::LectureImpossible,
    }
}

fn message(genre: Genre, titre: &str, detail: &str) -> Message {
    Message {
        genre,
        titre: titre.into(),
        detail: detail.into(),
        actions: Vec::new(),
        duree_ms: None,
    }
}

/// « Impossible de lire le texte » : le titre seul.
pub fn message_impossible() -> Message {
    message(Genre::Erreur, TITRE_LECTURE_IMPOSSIBLE, "")
}

/// Le bandeau de la fin d'une lecture du raccourci (§5.4, étape 5).
pub fn message_fin(fin: &FinLecture) -> Message {
    match (&fin.issue, &fin.copie) {
        (Issue::Texte(_), Some(Etape::Faite)) => message(Genre::Reussite, TITRE_COPIE, ""),
        (Issue::Texte(_), Some(Etape::Echec(cause))) => {
            message(Genre::Erreur, TITRE_COPIE_IMPOSSIBLE, cause)
        }
        (Issue::Texte(_), _) => message(Genre::Erreur, TITRE_COPIE_IMPOSSIBLE, ""),
        (Issue::Vide, _) => Message::erreur_breve(
            TITRE_AUCUN_TEXTE,
            vec![Action {
                id: ACTION_REESSAYER.into(),
                libelle: LIBELLE_REESSAYER.into(),
            }],
            DUREE_AUCUN_TEXTE_MS,
        ),
        (Issue::AucuneLangue | Issue::Echec(_), _) => message_impossible(),
    }
}

/// Vrai si une langue de lecture existe.
pub fn disponible(d: &Disponibilite) -> bool {
    matches!(d, Disponibilite::Langues(l) if !l.is_empty())
}

// Lignes du journal (jamais un texte lu).

fn ms(d: Duration) -> u128 {
    d.as_millis()
}

/// Le nombre de lignes d'une lecture (les lignes sont comptées de 0, sans trou).
pub fn lignes(lu: &Lu) -> u32 {
    lu.mots.last().map_or(0, |m| m.ligne + 1)
}

/// `texte langues <l1,l2,…>|aucune|aucune (essai)`, `texte non pris en charge`.
pub fn ligne_langues(d: &Disponibilite, essai: bool) -> String {
    match d {
        Disponibilite::Langues(l) if !l.is_empty() => format!("texte langues {}", l.join(",")),
        Disponibilite::NonPrisEnCharge => "texte non pris en charge".into(),
        _ if essai => "texte langues aucune (essai)".into(),
        _ => "texte langues aucune".into(),
    }
}

/// `texte lu <n> lignes <m> mots en <ms> ms langue <l> facteur <f>`, `texte vide en <ms>
/// ms`, `texte impossible: <aucune_langue|moteur (<code>)|echeance>`.
pub fn ligne_issue(issue: &Issue, duree: Duration, facteur: f32) -> String {
    match issue {
        Issue::Texte(lu) => format!(
            "texte lu {} lignes {} mots en {} ms langue {} facteur {facteur}",
            lignes(lu),
            lu.mots.len(),
            ms(duree),
            lu.langue
        ),
        Issue::Vide => format!("texte vide en {} ms", ms(duree)),
        Issue::AucuneLangue => "texte impossible: aucune_langue".into(),
        Issue::Echec(c) if c == CAUSE_ECHEANCE => "texte impossible: echeance".into(),
        Issue::Echec(c) => format!("texte impossible: moteur ({c})"),
    }
}

/// `texte moteur cree en <ms> ms`.
pub fn ligne_moteur(d: Duration) -> String {
    format!("texte moteur cree en {} ms", ms(d))
}

/// `texte copie`, `texte copie echec (<code>)` ; `None` sans copie.
pub fn ligne_copie(copie: Option<&Etape>) -> Option<String> {
    match copie? {
        Etape::Faite => Some("texte copie".into()),
        Etape::Echec(c) => Some(format!("texte copie echec ({})", code_court(c))),
        Etape::NonDemandee => None,
    }
}

/// `chrono texte relache-gel-cache <ms> relache-message <ms>`.
pub fn ligne_chrono(gel_cache: Duration, message: Duration) -> String {
    format!(
        "chrono texte relache-gel-cache {} relache-message {}",
        ms(gel_cache),
        ms(message)
    )
}

/// Le code d'une lecture qui n'a rien rendu (`None` : texte lu ou vide).
pub fn code_echec(issue: &Issue) -> Option<&str> {
    match issue {
        Issue::AucuneLangue => Some("aucune_langue"),
        Issue::Echec(c) => Some(c),
        Issue::Texte(_) | Issue::Vide => None,
    }
}

/// `editeur texte <id> <n> lu <l> lignes <m> mots en <ms> ms|vide|echec (<code>)`.
pub fn ligne_editeur(id: u64, numero: u64, issue: &Issue, duree: Duration) -> String {
    let suite = match issue {
        Issue::Texte(lu) => format!(
            "lu {} lignes {} mots en {} ms",
            lignes(lu),
            lu.mots.len(),
            ms(duree)
        ),
        Issue::Vide => "vide".into(),
        autre => format!("echec ({})", code_echec(autre).unwrap_or_default()),
    };
    format!("editeur texte {id} {numero} {suite}")
}

/// `editeur texte copie <id> <n> <k> mots|echec (<code>)`.
pub fn ligne_editeur_copie(id: u64, numero: u64, issue: Result<usize, &str>) -> String {
    match issue {
        Ok(k) => format!("editeur texte copie {id} {numero} {k} mots"),
        Err(c) => format!("editeur texte copie {id} {numero} echec ({c})"),
    }
}

// Ce que reçoit la page de l'éditeur.

/// La boîte d'un mot, en pixels de l'image composée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MotPage {
    pub x: u32,
    pub y: u32,
    pub largeur: u32,
    pub hauteur: u32,
    pub ligne: u32,
}

/// `editeur:texte { id, numero, etat: "lu"|"vide"|"echec", mots }` : jamais le texte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChargeTexte {
    pub id: u64,
    pub numero: u64,
    pub etat: &'static str,
    pub mots: Vec<MotPage>,
}

/// `editeur:texte_copie { id, numero, etat: "fait"|"echec" }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ChargeTexteCopie {
    pub id: u64,
    pub numero: u64,
    pub etat: &'static str,
}

/// La charge de `editeur:texte` pour une lecture.
pub fn charge_editeur(id: u64, numero: u64, issue: &Issue) -> ChargeTexte {
    let (etat, mots) = match issue {
        Issue::Texte(lu) => (
            "lu",
            lu.mots
                .iter()
                .map(|m| MotPage {
                    x: m.x,
                    y: m.y,
                    largeur: m.largeur,
                    hauteur: m.hauteur,
                    ligne: m.ligne,
                })
                .collect(),
        ),
        Issue::Vide => ("vide", Vec::new()),
        Issue::AucuneLangue | Issue::Echec(_) => ("echec", Vec::new()),
    };
    ChargeTexte {
        id,
        numero,
        etat,
        mots,
    }
}

/// Le texte des mots `debut` à `fin` (inclus) de la lecture gardée par l'éditeur, s'il
/// s'agit de la lecture `numero`, et le nombre de mots ; sinon le code du refus
/// (`numero` : aucune lecture, ou une autre ; `bornes` : hors de la liste ou inversées).
pub fn extrait_choisi(
    garde: Option<&(u64, Lu)>,
    numero: u64,
    debut: u32,
    fin: u32,
    fin_de_ligne: &str,
) -> Result<(String, usize), &'static str> {
    let (n, lu) = garde.ok_or("numero")?;
    if *n != numero {
        return Err("numero");
    }
    let (d, f) = (debut as usize, fin as usize);
    let texte = extrait(lu, d, f, fin_de_ligne).ok_or("bornes")?;
    Ok((texte, f - d + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systeme::gel::Pixels;
    use crate::systeme::texte::{LigneBrute, MotBrut, Reconnu};
    use crate::texte::{depuis_reconnu, preparer};

    fn lu(lignes: &[&[&str]]) -> Lu {
        let lignes = lignes
            .iter()
            .enumerate()
            .map(|(i, mots)| LigneBrute {
                texte: mots.join(" "),
                mots: mots
                    .iter()
                    .enumerate()
                    .map(|(j, t)| MotBrut {
                        texte: (*t).into(),
                        x: j as f32 * 10.0,
                        y: i as f32 * 20.0,
                        largeur: 8.0,
                        hauteur: 12.0,
                    })
                    .collect(),
            })
            .collect();
        depuis_reconnu(
            Reconnu {
                langue: "en-US".into(),
                lignes,
            },
            1.0,
            200,
            200,
        )
    }

    fn fin(issue: Issue, copie: Option<Etape>) -> FinLecture {
        FinLecture {
            numero: 1,
            issue,
            copie,
            duree: Duration::from_millis(42),
            document: None,
            facteur: 2.0,
        }
    }

    #[test]
    fn ecran_garde_entre_le_balayage_et_la_borne() {
        let ms = Duration::from_millis;
        assert_eq!(balayage(false), ms(400));
        assert_eq!(balayage(true), ms(120));
        // Lecture rendue avant la fin du balayage : l'écran reste.
        assert!(!liberer_ecran(ms(100), ms(400), true));
        assert!(liberer_ecran(ms(400), ms(400), true));
        assert!(liberer_ecran(ms(130), ms(120), true));
        // Lecture pas rendue : l'écran reste jusqu'à la borne, jamais au-delà.
        assert!(!liberer_ecran(ms(1499), ms(400), false));
        assert!(liberer_ecran(ms(1500), ms(400), false));
        assert!(liberer_ecran(ms(60_000), ms(400), false));
        // Un minimum au-delà de la borne ne la repousse pas.
        assert!(liberer_ecran(ms(1500), ms(5000), true));
    }

    #[test]
    fn facteur_comme_preparer() {
        let image = |l: u32, h: u32| Pixels {
            largeur: l,
            hauteur: h,
            rgba: vec![255; (l * h * 4) as usize],
        };
        for (l, h, dim) in [
            (640, 200, 10_000),
            (60, 60, 100),
            (200, 50, 100),
            (50, 40, 100),
            (1, 1, 1),
            (1200, 900, 10_000),
        ] {
            let p = preparer(&image(l, h), dim).expect("preparee");
            assert_eq!(facteur_de(l, h, dim), p.facteur, "{l}x{h} {dim}");
        }
        assert_eq!(facteur_de(10, 10, 0), 1.0);
    }

    #[test]
    fn fins_et_bandeau() {
        let texte = Issue::Texte(lu(&[&["a", "b"]]));
        let f = fin(texte.clone(), Some(Etape::Faite));
        assert_eq!(declencheur(&f.issue), Declencheur::TexteLu);
        let m = message_fin(&f);
        assert_eq!(
            (m.genre, m.titre.as_str()),
            (Genre::Reussite, "Texte copié")
        );
        assert_eq!(m.duree_ms, None);
        let m = message_fin(&fin(texte, Some(Etape::Echec("occupe".into()))));
        assert_eq!(
            (m.genre, m.titre.as_str(), m.detail.as_str()),
            (Genre::Erreur, "Impossible de copier", "occupe")
        );
        let vide = fin(Issue::Vide, None);
        assert_eq!(declencheur(&vide.issue), Declencheur::TexteLu);
        let m = message_fin(&vide);
        assert_eq!(
            (m.genre, m.titre.as_str()),
            (Genre::Erreur, "Aucun texte trouvé")
        );
        assert_eq!(m.duree_ms, Some(4000));
        assert_eq!(m.actions.len(), 1);
        assert_eq!(m.actions[0].id, "reessayer");
        assert_eq!(m.actions[0].libelle, "Réessayer");
        for issue in [Issue::AucuneLangue, Issue::Echec("echeance".into())] {
            assert_eq!(declencheur(&issue), Declencheur::LectureImpossible);
            let m = message_fin(&fin(issue, None));
            assert_eq!(m.titre, "Impossible de lire le texte");
            assert!(m.detail.is_empty() && m.actions.is_empty());
        }
    }

    #[test]
    fn lignes_du_journal_sans_texte() {
        let l = lu(&[&["Secret", "mot"], &["autre"]]);
        let ms = Duration::from_millis(37);
        let ligne = ligne_issue(&Issue::Texte(l.clone()), ms, 2.0);
        assert_eq!(
            ligne,
            "texte lu 2 lignes 3 mots en 37 ms langue en-US facteur 2"
        );
        assert!(!ligne.contains("Secret"));
        assert_eq!(ligne_issue(&Issue::Vide, ms, 1.0), "texte vide en 37 ms");
        assert_eq!(
            ligne_issue(&Issue::AucuneLangue, ms, 1.0),
            "texte impossible: aucune_langue"
        );
        assert_eq!(
            ligne_issue(&Issue::Echec("echeance".into()), ms, 1.0),
            "texte impossible: echeance"
        );
        assert_eq!(
            ligne_issue(&Issue::Echec("deja_en_cours".into()), ms, 1.0),
            "texte impossible: moteur (deja_en_cours)"
        );
        assert_eq!(
            ligne_moteur(Duration::from_millis(4)),
            "texte moteur cree en 4 ms"
        );
        assert_eq!(
            ligne_copie(Some(&Etape::Faite)).as_deref(),
            Some("texte copie")
        );
        assert_eq!(
            ligne_copie(Some(&Etape::Echec("x".into()))).as_deref(),
            Some("texte copie echec (x)")
        );
        assert_eq!(ligne_copie(None), None);
        assert_eq!(
            ligne_chrono(Duration::from_millis(400), Duration::from_millis(455)),
            "chrono texte relache-gel-cache 400 relache-message 455"
        );
        let e = ligne_editeur(3, 9, &Issue::Texte(l), ms);
        assert_eq!(e, "editeur texte 3 9 lu 2 lignes 3 mots en 37 ms");
        assert_eq!(
            ligne_editeur(3, 9, &Issue::Vide, ms),
            "editeur texte 3 9 vide"
        );
        assert_eq!(
            ligne_editeur(3, 9, &Issue::AucuneLangue, ms),
            "editeur texte 3 9 echec (aucune_langue)"
        );
        assert_eq!(
            ligne_editeur_copie(3, 9, Ok(2)),
            "editeur texte copie 3 9 2 mots"
        );
        assert_eq!(
            ligne_editeur_copie(3, 9, Err("bornes")),
            "editeur texte copie 3 9 echec (bornes)"
        );
    }

    #[test]
    fn langues() {
        let fr = Disponibilite::Langues(vec!["fr-FR".into(), "en-US".into()]);
        assert!(disponible(&fr));
        assert_eq!(ligne_langues(&fr, false), "texte langues fr-FR,en-US");
        assert!(!disponible(&Disponibilite::Aucune));
        assert!(!disponible(&Disponibilite::Langues(Vec::new())));
        assert!(!disponible(&Disponibilite::NonPrisEnCharge));
        assert_eq!(
            ligne_langues(&Disponibilite::Aucune, false),
            "texte langues aucune"
        );
        assert_eq!(
            ligne_langues(&Disponibilite::Aucune, true),
            "texte langues aucune (essai)"
        );
        assert_eq!(
            ligne_langues(&Disponibilite::NonPrisEnCharge, false),
            "texte non pris en charge"
        );
    }

    #[test]
    fn charge_de_la_page_sans_texte() {
        let l = lu(&[&["BreezyShot", "lit"], &["ce"]]);
        let c = charge_editeur(4, 7, &Issue::Texte(l));
        assert_eq!((c.id, c.numero, c.etat, c.mots.len()), (4, 7, "lu", 3));
        assert_eq!(
            c.mots[2],
            MotPage {
                x: 0,
                y: 20,
                largeur: 8,
                hauteur: 12,
                ligne: 1
            }
        );
        let json = serde_json::to_string(&c).expect("json");
        assert!(
            !json.contains("BreezyShot") && !json.contains("lit"),
            "{json}"
        );
        assert_eq!(charge_editeur(4, 7, &Issue::Vide).etat, "vide");
        let e = charge_editeur(4, 7, &Issue::Echec("x".into()));
        assert_eq!((e.etat, e.mots.len()), ("echec", 0));
        let copie = ChargeTexteCopie {
            id: 4,
            numero: 7,
            etat: "fait",
        };
        assert_eq!(
            serde_json::to_string(&copie).expect("json"),
            r#"{"id":4,"numero":7,"etat":"fait"}"#
        );
    }

    #[test]
    fn extrait_de_la_lecture_gardee() {
        let garde = (7, lu(&[&["a", "b", "c"], &["d"]]));
        assert_eq!(
            extrait_choisi(Some(&garde), 7, 1, 3, "\r\n"),
            Ok(("b c\r\nd".into(), 3))
        );
        assert_eq!(
            extrait_choisi(Some(&garde), 7, 0, 0, "\n"),
            Ok(("a".into(), 1))
        );
        // Un autre numéro, ou aucune lecture : refusé.
        assert_eq!(extrait_choisi(Some(&garde), 6, 0, 1, "\n"), Err("numero"));
        assert_eq!(extrait_choisi(None, 7, 0, 1, "\n"), Err("numero"));
        // Bornes inversées ou hors de la liste.
        assert_eq!(extrait_choisi(Some(&garde), 7, 2, 1, "\n"), Err("bornes"));
        assert_eq!(extrait_choisi(Some(&garde), 7, 0, 4, "\n"), Err("bornes"));
    }

    fn en_cours(numero: u64, relache: Instant) -> Option<LectureEnCours> {
        Some(LectureEnCours {
            numero,
            relache,
            minimum: BALAYAGE,
            fin: None,
        })
    }

    #[test]
    fn resultat_garde_jusqu_a_la_fin_du_balayage() {
        let t = Instant::now();
        let ms = Duration::from_millis;
        let mut l = en_cours(1, t);
        // Arrivé à 100 ms : gardé ; l'échéance ne l'emporte pas sur lui.
        let f = fin(Issue::Vide, None);
        assert_eq!(arrivee(&mut l, f.clone(), t + ms(100)), Arrivee::Gardee);
        assert_eq!(echeance(&l, 1, CAUSE_ECHEANCE), None);
        // Un second résultat du même numéro est périmé.
        assert_eq!(arrivee(&mut l, f.clone(), t + ms(150)), Arrivee::Perimee);
        assert_eq!(balayage_fini(&mut l, 2), None);
        assert_eq!(balayage_fini(&mut l, 1), Some(f.clone()));
        assert_eq!(balayage_fini(&mut l, 1), None);
        assert!(terminer(&mut l, 2).is_none());
        assert!(terminer(&mut l, 1).is_some());
        assert!(l.is_none());
        // Plus de lecture : tout est périmé.
        assert_eq!(arrivee(&mut l, f, t + ms(500)), Arrivee::Perimee);
        assert_eq!(echeance(&l, 1, CAUSE_ECHEANCE), None);
    }

    #[test]
    fn resultat_apres_le_balayage_ou_la_borne() {
        let t = Instant::now();
        let ms = Duration::from_millis;
        let f = fin(Issue::Vide, None);
        let mut l = en_cours(1, t);
        assert_eq!(
            arrivee(&mut l, f.clone(), t + ms(400)),
            Arrivee::Finir(f.clone())
        );
        // Minimum au-delà de la borne : fini à 1,5 s quand même.
        let mut l = Some(LectureEnCours {
            minimum: Duration::from_secs(5),
            ..en_cours(1, t).unwrap()
        });
        assert_eq!(arrivee(&mut l, f.clone(), t + ms(1499)), Arrivee::Gardee);
        let mut l = Some(LectureEnCours {
            minimum: Duration::from_secs(5),
            ..en_cours(1, t).unwrap()
        });
        assert_eq!(arrivee(&mut l, f.clone(), t + ms(1500)), Arrivee::Finir(f));
        // Autre numéro : périmé, rien n'est gardé.
        let mut l = en_cours(2, t);
        assert_eq!(arrivee(&mut l, fin(Issue::Vide, None), t), Arrivee::Perimee);
        assert!(l.as_ref().is_some_and(|x| x.fin.is_none()));
    }

    #[test]
    fn echeance_avant_le_resultat() {
        let t = Instant::now();
        let mut l = en_cours(1, t);
        let e = echeance(&l, 1, CAUSE_ECHEANCE).expect("echec");
        assert_eq!(e.issue, Issue::Echec("echeance".into()));
        assert_eq!(declencheur(&e.issue), Declencheur::LectureImpossible);
        assert_eq!(echeance(&l, 2, CAUSE_ECHEANCE), None);
        let p = echeance(&l, 1, CAUSE_FIL).expect("fil");
        assert_eq!(
            ligne_issue(&p.issue, p.duree, 1.0),
            "texte impossible: moteur (fil)"
        );
        assert!(terminer(&mut l, 1).is_some());
        // Le résultat arrivé ensuite est périmé.
        let tard = fin(Issue::Texte(lu(&[&["a"]])), None);
        assert_eq!(arrivee(&mut l, tard, t), Arrivee::Perimee);
    }

    #[test]
    fn un_seul_gagne_le_resultat() {
        let a = Attendue::new();
        assert!(!a.prendre(1), "rien d'attendu");
        a.poser(3);
        assert!(!a.prendre(2));
        // Le fil `texte` et l'échéance à 10 s pile : un seul des deux.
        assert!(a.prendre(3));
        assert!(!a.prendre(3));
        a.poser(4);
        a.oublier();
        assert!(!a.prendre(4), "lecture abandonnee : rien n'est copie");
        assert!(!a.prendre(0));
        // Deux fils en même temps.
        let a = std::sync::Arc::new(Attendue::new());
        a.poser(9);
        let gagnes: usize = (0..8)
            .map(|_| {
                let a = std::sync::Arc::clone(&a);
                std::thread::spawn(move || a.prendre(9))
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|h| usize::from(h.join().unwrap_or(false)))
            .sum();
        assert_eq!(gagnes, 1);
    }

    #[test]
    fn codes_courts() {
        assert_eq!(code_court("occupe"), "occupe");
        assert_eq!(code_court("deja_en_cours"), "deja_en_cours");
        assert_eq!(
            code_court("Le presse-papiers est occupé."),
            "presse_papiers"
        );
        assert_eq!(code_court(""), "presse_papiers");
        assert_eq!(
            ligne_copie(Some(&Etape::Echec("Accès refusé".into()))).as_deref(),
            Some("texte copie echec (presse_papiers)")
        );
    }
}
