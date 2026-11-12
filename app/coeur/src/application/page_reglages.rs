//! Décisions de la fenêtre des Réglages (étape 10) : fonctions pures, testées sans écran.
//!
//! Ce que la page reçoit (`Vue`), ce qu'elle peut demander (`Changement`, `Saisie`), et la
//! traduction de chaque demande en réglages ou en refus d'une ligne. Les gestes (fichier,
//! raccourcis, registre, fenêtre) sont dans `fenetre_reglages.rs`.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use super::decompte::RETARDATEURS_PROPOSES;
use crate::fenetres::theme::ThemePage;
use crate::raccourcis::{self, ErreurNotation, Raison};
use crate::reglages::{self, Coin, ErreurEcriture, Raccourcis, Reglages, Theme};
use crate::sortie;
use crate::systeme::EtatDemarrage;

/// Durées proposées pour la vignette, dans l'ordre de la page (`0` = toujours).
pub const DUREES_PROPOSEES: [u32; 4] = [3, 6, 10, reglages::DUREE_TOUJOURS];

/// Texte de l'alerte quand rien ne peut être écrit (les changements valent pour la session).
pub const ALERTE_NON_ENREGISTRES: &str = "Réglages non enregistrés";

/// Texte d'une action sans raccourci.
pub const AUCUN_RACCOURCI: &str = "Aucun raccourci";

/// Les lignes fixes de la page, dans l'ordre (les raccourcis viennent de la vue). Seules
/// celles qui ont un effet dans cette version sont montrées (`lignes_montrees`) : la
/// rubrique Capture demande le Gel (`capture_disponible`, sinon aucune photo et la rubrique
/// se cache seule), la loupe demande en plus qu'elle existe (`loupe_disponible`).
pub fn lignes_montrees(
    vignette_construite: bool,
    capture_disponible: bool,
    loupe_disponible: bool,
) -> Vec<&'static str> {
    let mut l = Vec::new();
    if capture_disponible {
        l.push("retardateur");
        if loupe_disponible {
            l.push("loupe");
        }
        l.push("pointeur");
    }
    l.push("copier");
    if vignette_construite {
        l.extend(["vignette", "vignette_duree", "vignette_coin"]);
    }
    l.push("dossier");
    // Sans vignette, chaque capture est enregistrée (`zone::doit_enregistrer`) : le
    // réglage n'aurait aucun effet.
    if vignette_construite {
        l.push("enregistrer");
    }
    l.extend(["modele_nom", "theme", "demarrage", "version"]);
    l
}

/// Un changement demandé par la page. Une clé ou une valeur d'un autre type est refusée
/// par serde avant d'arriver ici.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "cle", content = "valeur", rename_all = "snake_case")]
pub enum Changement {
    Copier(bool),
    Enregistrer(bool),
    VignetteAfficher(bool),
    VignetteDuree(u32),
    VignetteCoin(String),
    ModeleNom(String),
    Theme(String),
    Demarrage(bool),
    Retardateur(u32),
    Loupe(bool),
    Pointeur(bool),
}

impl Changement {
    /// L'identifiant de la ligne de la page.
    pub fn ligne(&self) -> &'static str {
        match self {
            Changement::Copier(_) => "copier",
            Changement::Enregistrer(_) => "enregistrer",
            Changement::VignetteAfficher(_) => "vignette",
            Changement::VignetteDuree(_) => "vignette_duree",
            Changement::VignetteCoin(_) => "vignette_coin",
            Changement::ModeleNom(_) => "modele_nom",
            Changement::Theme(_) => "theme",
            Changement::Demarrage(_) => "demarrage",
            Changement::Retardateur(_) => "retardateur",
            Changement::Loupe(_) => "loupe",
            Changement::Pointeur(_) => "pointeur",
        }
    }
}

/// Un refus, dit sur une ligne de la page. `texte` vide : rien à montrer (demande qui ne
/// peut venir que d'une page fausse ; le journal le dit).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Refus {
    pub ligne: String,
    pub texte: String,
    /// Code pour le journal.
    #[serde(skip)]
    pub code: &'static str,
}

impl Refus {
    pub fn nouveau(ligne: &str, texte: &str, code: &'static str) -> Refus {
        Refus {
            ligne: ligne.to_string(),
            texte: texte.to_string(),
            code,
        }
    }
}

fn oui_non(b: bool) -> &'static str {
    if b {
        "oui"
    } else {
        "non"
    }
}

/// Ligne de journal d'un changement accepté. Jamais le modèle de nom (texte de
/// l'utilisateur) : seulement la clé.
pub fn ligne_changement(c: &Changement) -> String {
    let valeur = match c {
        Changement::Copier(b)
        | Changement::Enregistrer(b)
        | Changement::VignetteAfficher(b)
        | Changement::Demarrage(b)
        | Changement::Loupe(b)
        | Changement::Pointeur(b) => oui_non(*b).to_string(),
        Changement::VignetteDuree(n) | Changement::Retardateur(n) => n.to_string(),
        Changement::VignetteCoin(t) | Changement::Theme(t) => t.clone(),
        Changement::ModeleNom(_) => return "reglage change: modele_nom".into(),
    };
    format!("reglage change: {} {valeur}", c.ligne())
}

/// Refus de la dernière sortie active (texte du contrat, une ligne).
pub const REFUS_AUCUNE_SORTIE: &str = "Gardez au moins la copie, l'enregistrement ou la vignette";

/// Le texte d'un refus de `reglages::valider`.
fn refus_validation(ligne: &str, e: &ErreurEcriture, n: &Reglages) -> Refus {
    match e {
        ErreurEcriture::AucuneSortie => Refus::nouveau(ligne, REFUS_AUCUNE_SORTIE, "aucune_sortie"),
        ErreurEcriture::ValeurInvalide {
            champ: "modele_nom",
        } if n.modele_nom.trim().is_empty() => Refus::nouveau(ligne, "Nom vide", "nom_vide"),
        ErreurEcriture::ValeurInvalide {
            champ: "modele_nom",
        } => Refus::nouveau(ligne, "Nom trop long", "nom_trop_long"),
        _ => Refus::nouveau(ligne, "", "invalide"),
    }
}

/// Que faire du fichier relu avant un changement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relecture {
    /// Rien n'a changé dehors, ou le fichier n'a pas pu être lu (valeurs protégées, qui ne
    /// disent rien de lui) : la mémoire reste la base, `derniere_lecture` et les raccourcis
    /// ne bougent pas.
    Garder,
    /// Le fichier a été changé dehors : il devient la base.
    Adopter,
}

/// Décide de la relecture. Fonction pure.
pub fn relecture(relu: &Reglages, derniere: Option<&Reglages>) -> Relecture {
    if relu.protege() || derniere == Some(relu) {
        Relecture::Garder
    } else {
        Relecture::Adopter
    }
}

/// Un texte venu de la page, rendu sûr pour le journal : ASCII imprimable seulement (le
/// reste devient « ? »), 24 caractères au plus. Fonction pure.
pub fn code_pour_journal(texte: &str) -> String {
    texte
        .chars()
        .take(24)
        .map(|c| if c.is_ascii_graphic() { c } else { '?' })
        .collect()
}

/// Applique un changement à une copie des réglages et la valide par le module `reglages`.
/// Fonction pure.
pub fn appliquer(r: &Reglages, c: &Changement) -> Result<Reglages, Refus> {
    let mut n = r.clone();
    let invalide = || Refus::nouveau(c.ligne(), "", "invalide");
    match c {
        Changement::Copier(b) => n.copier_apres_capture = *b,
        Changement::Enregistrer(b) => n.enregistrer_apres_capture = *b,
        Changement::VignetteAfficher(b) => n.vignette.afficher = *b,
        Changement::VignetteDuree(d) => n.vignette.duree_s = *d,
        Changement::VignetteCoin(t) => {
            n.vignette.coin = Coin::depuis_texte(t).ok_or_else(invalide)?
        }
        Changement::ModeleNom(t) => n.modele_nom = t.clone(),
        Changement::Theme(t) => n.theme = Theme::depuis_texte(t).ok_or_else(invalide)?,
        Changement::Demarrage(b) => n.demarrer_avec_systeme = *b,
        Changement::Retardateur(s) if *s > reglages::RETARDATEUR_MAX_S => return Err(invalide()),
        Changement::Retardateur(s) => n.capture.retardateur_s = *s,
        // La loupe reste dans `selection` (compatibilité du fichier, version 3).
        Changement::Loupe(b) => n.selection.loupe = *b,
        Changement::Pointeur(b) => n.capture.pointeur = *b,
    }
    reglages::valider(&n).map_err(|e| refus_validation(c.ligne(), &e, &n))?;
    Ok(n)
}

/// Le thème des pages pour un réglage (`None` : celui du système).
pub fn theme_page(theme: Theme) -> Option<ThemePage> {
    match theme {
        Theme::Systeme => None,
        Theme::Clair => Some(ThemePage::Clair),
        Theme::Sombre => Some(ThemePage::Sombre),
    }
}

/// Libellé d'une action dans la rubrique Raccourcis.
pub fn libelle_action(action: &str) -> &'static str {
    match action {
        "zone" => "Zone",
        "plein_ecran" => "Plein écran",
        "fenetre" => "Fenêtre",
        "derniere_zone" => "Refaire la dernière zone",
        _ => "",
    }
}

/// Une touche tapée dans la page, telle que le navigateur la donne.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Saisie {
    pub ctrl: bool,
    pub maj: bool,
    pub alt: bool,
    pub win: bool,
    /// `KeyboardEvent.key` (dépend de la disposition du clavier).
    pub cle: String,
    /// `KeyboardEvent.code` (position physique).
    pub code: String,
}

const TOUCHE_NON_PRISE: &str = "Touche non prise en charge";

/// Noms de touches par `KeyboardEvent.code`, vers la notation des réglages.
const CODES_NOMMES: [(&str, &str); 14] = [
    ("PrintScreen", "ImprEcran"),
    ("Space", "Espace"),
    ("Enter", "Entree"),
    ("Tab", "Tab"),
    ("Insert", "Inser"),
    ("Delete", "Suppr"),
    ("Home", "Debut"),
    ("End", "Fin"),
    ("PageUp", "PagePrec"),
    ("PageDown", "PageSuiv"),
    ("ArrowUp", "Haut"),
    ("ArrowDown", "Bas"),
    ("ArrowLeft", "Gauche"),
    ("ArrowRight", "Droite"),
];

/// La touche (sans modificateurs) dans la notation des réglages. Les lettres se lisent
/// par `cle` (la touche marquée A d'un clavier AZERTY donne `A`, comme pour le système),
/// les chiffres de la rangée et les autres touches par `code` (`Digit1` donne `1`, que Maj
/// soit tenue ou non). `None` : touche hors de la notation (pavé numérique, ponctuation…).
fn touche_saisie(s: &Saisie) -> Option<String> {
    let mut lettres = s.cle.chars();
    if let (Some(c), None) = (lettres.next(), lettres.next()) {
        if c.is_ascii_alphabetic() {
            return Some(c.to_ascii_uppercase().to_string());
        }
    }
    if let Some(n) = s.code.strip_prefix("Digit") {
        if n.len() == 1 && n.as_bytes()[0].is_ascii_digit() {
            return Some(n.to_string());
        }
    }
    if let Some(n) = s.code.strip_prefix('F') {
        if let Ok(v) = n.parse::<u8>() {
            if (1..=24).contains(&v) && !n.starts_with('0') {
                return Some(format!("F{v}"));
            }
        }
    }
    CODES_NOMMES
        .iter()
        .find(|(code, _)| *code == s.code)
        .map(|(_, nom)| nom.to_string())
}

/// Traduit une touche tapée en notation canonique (`Ctrl+Maj+1`), vérifiée par
/// `raccourcis::analyser` (mêmes règles que le fichier). `Err` : le texte de la ligne.
pub fn notation_saisie(s: &Saisie) -> Result<String, Refus> {
    let refus = |texte: &str, code| Refus::nouveau("raccourci", texte, code);
    let touche =
        touche_saisie(s).ok_or_else(|| refus(TOUCHE_NON_PRISE, "touche non prise en charge"))?;
    let mut morceaux: Vec<&str> = Vec::new();
    if s.ctrl {
        morceaux.push("Ctrl");
    }
    if s.maj {
        morceaux.push("Maj");
    }
    if s.alt {
        morceaux.push("Alt");
    }
    if s.win {
        morceaux.push("Win");
    }
    morceaux.push(&touche);
    let notation = morceaux.join("+");
    match raccourcis::analyser(&notation) {
        Ok(r) => Ok(r.canonique()),
        Err(e) => Err(refus(&texte_refus(&Raison::Invalide(e)), "invalide")),
    }
}

/// Le texte d'un candidat écarté, une ligne sous le raccourci.
pub fn texte_refus(raison: &Raison) -> String {
    match raison {
        Raison::Invalide(
            ErreurNotation::ToucheSeuleInterdite(_) | ErreurNotation::MajSeuleInterdite(_),
        ) => "Ajoutez Ctrl ou Alt".into(),
        Raison::Invalide(ErreurNotation::ReserveAuSysteme(_)) => "Réservé par Windows".into(),
        Raison::Invalide(_) => TOUCHE_NON_PRISE.into(),
        Raison::ReserveParLeSysteme => "Impr. écran est réservée par Windows".into(),
        Raison::DejaPrisParUneAutreAction(a) => {
            format!("Déjà pris par « {} »", libelle_action(a))
        }
        Raison::RefuseParLeSysteme => "Pris par une autre application".into(),
    }
}

/// Le dossier à écrire pour un dossier choisi : `Ok(None)` s'il est le dossier par défaut
/// (Images › BreezyShot), `Err` (code du journal) s'il est vide ou relatif. Fonction pure.
pub fn dossier_choisi(
    choisi: &Path,
    par_defaut: Option<&Path>,
) -> Result<Option<PathBuf>, &'static str> {
    if choisi.as_os_str().is_empty() || !choisi.is_absolute() {
        return Err("invalide");
    }
    if par_defaut == Some(choisi) {
        return Ok(None);
    }
    Ok(Some(choisi.to_path_buf()))
}

/// L'aperçu du nom d'un fichier enregistré (« BreezyShot 2026-11-12 à 10.42.08.png »),
/// vide pour un modèle vide. Fonction pure.
pub fn apercu_nom(modele: &str, maintenant: DateTime<Local>) -> String {
    if modele.trim().is_empty() {
        return String::new();
    }
    format!("{}.png", sortie::nom_fichier(modele, maintenant))
}

/// Les candidats d'une action connue, à écrire dans les réglages.
pub fn fixer_candidats(r: &mut Raccourcis, action: &str, candidats: Vec<String>) -> bool {
    let cible = match action {
        "zone" => &mut r.zone,
        "plein_ecran" => &mut r.plein_ecran,
        "fenetre" => &mut r.fenetre,
        "derniere_zone" => &mut r.derniere_zone,
        _ => return false,
    };
    *cible = candidats;
    true
}

/// La vignette vue par la page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VueVignette {
    pub afficher: bool,
    pub duree_s: u32,
    pub coin: &'static str,
}

/// La rubrique Capture vue par la page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VueCapture {
    /// Une valeur du fichier hors de `retardateurs` (7) s'affiche sans segment coché.
    pub retardateur_s: u32,
    pub loupe: bool,
    pub pointeur: bool,
}

/// Une ligne de la rubrique Raccourcis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VueRaccourci {
    pub action: String,
    pub libelle: &'static str,
    /// Le raccourci tenu, en morceaux à afficher (`["Ctrl", "Maj", "1"]`), vide sans
    /// raccourci.
    pub touches: Vec<String>,
    /// Vrai si la liste diffère de celle par défaut (le bouton « Rétablir » se montre).
    pub modifie: bool,
    /// Une ligne sous le raccourci, ou rien.
    pub refus: Option<String>,
}

/// Tout ce que la page affiche, et rien de plus (aucun chemin complet).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VueReglages {
    pub lignes: Vec<&'static str>,
    pub capture: VueCapture,
    /// Les segments du retardateur, dans l'ordre de la page (`0` = aucun).
    pub retardateurs: [u32; 4],
    pub copier: bool,
    pub enregistrer: bool,
    pub vignette: VueVignette,
    pub durees: [u32; 4],
    /// Nom affiché du dossier d'enregistrement (« Images › BreezyShot »).
    pub dossier: String,
    pub modele_nom: String,
    pub apercu_nom: String,
    pub theme: &'static str,
    pub demarrage: &'static str,
    pub raccourcis: Vec<VueRaccourci>,
    pub version: &'static str,
    pub alerte: Option<String>,
    /// Vrai pendant la saisie d'un raccourci : faux, la page sort de « Appuyez… » (filet
    /// des 15 s, fenêtre refermée).
    pub saisie_en_cours: bool,
}

/// Ce que la vue prend ailleurs que dans les réglages.
pub struct Contexte<'a> {
    pub vignette_construite: bool,
    /// Le Gel existe sur ce système (`gel::disponible`) : la rubrique Capture a un effet.
    pub capture_disponible: bool,
    /// La loupe existe sur ce système (`gel::loupe_disponible`).
    pub loupe_disponible: bool,
    pub actions: &'a [&'a str],
    pub bilan: &'a raccourcis::Bilan,
    pub defaut: &'a Raccourcis,
    /// Le refus à dire sous chaque raccourci (dernier changement raté), par action.
    pub refus_raccourcis: &'a std::collections::BTreeMap<String, String>,
    pub dossier_affiche: String,
    pub demarrage: EtatDemarrage,
    pub version: &'static str,
    pub alerte: Option<String>,
    pub maintenant: DateTime<Local>,
    pub saisie_en_cours: bool,
}

/// L'état de « Lancer au démarrage » pour la page.
pub fn nom_demarrage(e: EtatDemarrage) -> &'static str {
    match e {
        EtatDemarrage::Actif => "oui",
        // Désactivé dans le gestionnaire des tâches : l'app ne démarre pas, l'interrupteur
        // est éteint ; BreezyShot ne touche pas à ce choix.
        EtatDemarrage::Inactif | EtatDemarrage::DesactiveParWindows => "non",
        EtatDemarrage::Indisponible => "indisponible",
    }
}

/// La vue complète. Fonction pure.
pub fn vue(r: &Reglages, c: &Contexte<'_>) -> VueReglages {
    let mut lignes = lignes_montrees(
        c.vignette_construite,
        c.capture_disponible,
        c.loupe_disponible,
    );
    if c.demarrage == EtatDemarrage::Indisponible {
        lignes.retain(|l| *l != "demarrage");
    }
    let raccourcis = c
        .actions
        .iter()
        .map(|a| {
            let retenu = c.bilan.retenu(a);
            let touches: Vec<String> = retenu
                .map(|x| x.affichage().split(" + ").map(str::to_string).collect())
                .unwrap_or_default();
            let refus = c
                .refus_raccourcis
                .get(*a)
                .cloned()
                .or_else(|| retenu.is_none().then(|| AUCUN_RACCOURCI.to_string()));
            VueRaccourci {
                action: a.to_string(),
                libelle: libelle_action(a),
                touches,
                modifie: r.raccourcis.candidats(a) != c.defaut.candidats(a),
                refus,
            }
        })
        .collect();
    VueReglages {
        lignes,
        capture: VueCapture {
            retardateur_s: r.capture.retardateur_s,
            loupe: r.selection.loupe,
            pointeur: r.capture.pointeur,
        },
        retardateurs: RETARDATEURS_PROPOSES,
        copier: r.copier_apres_capture,
        enregistrer: r.enregistrer_apres_capture,
        vignette: VueVignette {
            afficher: r.vignette.afficher,
            duree_s: r.vignette.duree_s,
            coin: r.vignette.coin.texte(),
        },
        durees: DUREES_PROPOSEES,
        dossier: c.dossier_affiche.clone(),
        modele_nom: r.modele_nom.clone(),
        apercu_nom: apercu_nom(&r.modele_nom, c.maintenant),
        theme: r.theme.texte(),
        demarrage: nom_demarrage(c.demarrage),
        raccourcis,
        version: c.version,
        alerte: c.alerte.clone(),
        saisie_en_cours: c.saisie_en_cours,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raccourcis::choisir;
    use crate::reglages::Plateforme;
    use chrono::TimeZone;
    use std::collections::BTreeMap;

    fn defaut() -> Reglages {
        Reglages::par_defaut(Plateforme::Windows)
    }

    fn saisie(ctrl: bool, maj: bool, alt: bool, cle: &str, code: &str) -> Saisie {
        Saisie {
            ctrl,
            maj,
            alt,
            win: false,
            cle: cle.into(),
            code: code.into(),
        }
    }

    #[test]
    fn lignes_selon_la_vignette() {
        let sans = lignes_montrees(false, false, false);
        assert_eq!(
            sans,
            [
                "copier",
                "dossier",
                "modele_nom",
                "theme",
                "demarrage",
                "version"
            ]
        );
        let avec = lignes_montrees(true, false, false);
        assert_eq!(
            avec,
            [
                "copier",
                "vignette",
                "vignette_duree",
                "vignette_coin",
                "dossier",
                "enregistrer",
                "modele_nom",
                "theme",
                "demarrage",
                "version"
            ]
        );
        // Sans Gel, la rubrique Capture n'a aucun effet : aucune de ses lignes.
        for absent in ["loupe", "retardateur", "pointeur"] {
            assert!(!avec.contains(&absent));
        }
    }

    #[test]
    fn lignes_de_la_capture() {
        // En tête, dans l'ordre de la page ; la loupe seulement si elle existe.
        let tout = lignes_montrees(true, true, true);
        assert_eq!(tout[..4], ["retardateur", "loupe", "pointeur", "copier"]);
        assert_eq!(tout.len(), 13);
        let sans_loupe = lignes_montrees(true, true, false);
        assert_eq!(sans_loupe[..3], ["retardateur", "pointeur", "copier"]);
        // La loupe ne vient jamais sans le Gel.
        assert_eq!(
            lignes_montrees(true, false, true),
            lignes_montrees(true, false, false)
        );
        // Rien de ce qui n'a pas d'effet en 0.4.
        for absent in ["barre", "format", "mise_a_jour"] {
            assert!(!tout.contains(&absent));
        }
    }

    #[test]
    fn changements_lus_depuis_la_page() {
        let c: Changement = serde_json::from_str(r#"{"cle":"copier","valeur":false}"#).unwrap();
        assert_eq!(c, Changement::Copier(false));
        let c: Changement = serde_json::from_str(r#"{"cle":"vignette_duree","valeur":0}"#).unwrap();
        assert_eq!(c, Changement::VignetteDuree(0));
        let c: Changement = serde_json::from_str(r#"{"cle":"retardateur","valeur":5}"#).unwrap();
        assert_eq!(c, Changement::Retardateur(5));
        let c: Changement = serde_json::from_str(r#"{"cle":"loupe","valeur":true}"#).unwrap();
        assert_eq!(c, Changement::Loupe(true));
        let c: Changement = serde_json::from_str(r#"{"cle":"pointeur","valeur":true}"#).unwrap();
        assert_eq!(c, Changement::Pointeur(true));
        for faux in [
            r#"{"cle":"barre","valeur":true}"#,
            r#"{"cle":"retardateur","valeur":-1}"#,
            r#"{"cle":"retardateur","valeur":"3"}"#,
            r#"{"cle":"pointeur","valeur":1}"#,
            r#"{"cle":"copier","valeur":"oui"}"#,
            r#"{"cle":"dossier_enregistrement","valeur":"C:\\x"}"#,
            r#"{"cle":"vignette_duree","valeur":-1}"#,
        ] {
            assert!(serde_json::from_str::<Changement>(faux).is_err(), "{faux}");
        }
    }

    #[test]
    fn appliquer_et_valider() {
        let r = defaut();
        let n = appliquer(&r, &Changement::Copier(false)).unwrap();
        assert!(!n.copier_apres_capture);
        let n = appliquer(&r, &Changement::VignetteDuree(0)).unwrap();
        assert_eq!(n.vignette.duree_s, 0);
        let n = appliquer(&r, &Changement::Theme("sombre".into())).unwrap();
        assert_eq!(n.theme, Theme::Sombre);
        let n = appliquer(&r, &Changement::VignetteCoin("haut_droite".into())).unwrap();
        assert_eq!(n.vignette.coin, Coin::HautDroite);
        // Refus.
        let e = appliquer(&r, &Changement::ModeleNom("  ".into())).unwrap_err();
        assert_eq!(
            (e.ligne.as_str(), e.texte.as_str()),
            ("modele_nom", "Nom vide")
        );
        let e = appliquer(&r, &Changement::VignetteDuree(61)).unwrap_err();
        assert_eq!((e.texte.as_str(), e.code), ("", "invalide"));
        let e = appliquer(&r, &Changement::Theme("rose".into())).unwrap_err();
        assert_eq!(e.ligne, "theme");
        let e = appliquer(&r, &Changement::VignetteCoin("milieu".into())).unwrap_err();
        assert_eq!(e.ligne, "vignette_coin");
        // Au moins une sortie.
        let mut seule_copie = r.clone();
        seule_copie.vignette.afficher = false;
        let e = appliquer(&seule_copie, &Changement::Copier(false)).unwrap_err();
        assert_eq!(e.ligne, "copier");
        assert_eq!(e.code, "aucune_sortie");
        assert_eq!(e.texte, REFUS_AUCUNE_SORTIE);
        // Nom vide, nom trop long.
        let e = appliquer(&r, &Changement::ModeleNom("  ".into())).unwrap_err();
        assert_eq!((e.texte.as_str(), e.code), ("Nom vide", "nom_vide"));
        let long = "a".repeat(reglages::MODELE_NOM_MAX + 1);
        let e = appliquer(&r, &Changement::ModeleNom(long)).unwrap_err();
        assert_eq!(
            (e.texte.as_str(), e.code),
            ("Nom trop long", "nom_trop_long")
        );
        let juste = "a".repeat(reglages::MODELE_NOM_MAX);
        assert!(appliquer(&r, &Changement::ModeleNom(juste)).is_ok());
    }

    #[test]
    fn appliquer_la_capture() {
        let r = defaut();
        for s in [0, 3, 5, 7, 10] {
            let n = appliquer(&r, &Changement::Retardateur(s)).unwrap();
            assert_eq!(n.capture.retardateur_s, s);
        }
        for s in [11, 60, u32::MAX] {
            let e = appliquer(&r, &Changement::Retardateur(s)).unwrap_err();
            assert_eq!(
                (e.ligne.as_str(), e.texte.as_str(), e.code),
                ("retardateur", "", "invalide")
            );
        }
        let n = appliquer(&r, &Changement::Loupe(true)).unwrap();
        assert!(n.selection.loupe);
        let n = appliquer(&r, &Changement::Pointeur(true)).unwrap();
        assert!(n.capture.pointeur);
        // Rien d'autre ne bouge.
        let mut attendu = r.clone();
        attendu.capture.pointeur = true;
        assert_eq!(n, attendu);
    }

    #[test]
    fn relecture_du_fichier() {
        let derniere = defaut();
        // Relu sain, égal : rien.
        assert_eq!(relecture(&derniere, Some(&derniere)), Relecture::Garder);
        // Relu sain, différent : adopté.
        let mut change = derniere.clone();
        change.modele_nom = "Externe {date}".into();
        assert_eq!(relecture(&change, Some(&derniere)), Relecture::Adopter);
        assert_eq!(relecture(&change, None), Relecture::Adopter);
        // Relu protégé (fichier verrouillé un instant) : la mémoire est gardée.
        let dossier = tempfile::tempdir().unwrap();
        let chemin = dossier.path().join("reglages.json");
        std::fs::create_dir_all(&chemin).unwrap(); // un dossier à la place : illisible
        let (protege, _) = reglages::charger(dossier.path(), Plateforme::Windows);
        assert!(protege.protege());
        assert_eq!(relecture(&protege, Some(&derniere)), Relecture::Garder);
        assert_eq!(relecture(&protege, None), Relecture::Garder);
    }

    #[test]
    fn code_sur_pour_le_journal() {
        assert_eq!(code_pour_journal("KeyA"), "KeyA");
        assert_eq!(code_pour_journal("a\nfaux"), "a?faux");
        assert_eq!(code_pour_journal("é"), "?");
        assert_eq!(code_pour_journal(&"x".repeat(100)).len(), 24);
    }

    #[test]
    fn lignes_de_journal_sans_texte_de_l_utilisateur() {
        assert_eq!(
            ligne_changement(&Changement::Copier(false)),
            "reglage change: copier non"
        );
        assert_eq!(
            ligne_changement(&Changement::VignetteDuree(0)),
            "reglage change: vignette_duree 0"
        );
        assert_eq!(
            ligne_changement(&Changement::ModeleNom("Lea {date}".into())),
            "reglage change: modele_nom"
        );
        assert_eq!(
            ligne_changement(&Changement::Demarrage(true)),
            "reglage change: demarrage oui"
        );
        assert_eq!(
            ligne_changement(&Changement::Retardateur(5)),
            "reglage change: retardateur 5"
        );
        assert_eq!(
            ligne_changement(&Changement::Loupe(true)),
            "reglage change: loupe oui"
        );
        assert_eq!(
            ligne_changement(&Changement::Pointeur(false)),
            "reglage change: pointeur non"
        );
    }

    #[test]
    fn saisie_vers_notation() {
        let ok = |s: Saisie| notation_saisie(&s).unwrap();
        assert_eq!(ok(saisie(true, true, false, "!", "Digit1")), "Ctrl+Maj+1");
        // AZERTY : la rangée des chiffres sans Maj donne « & », la position fait foi.
        assert_eq!(ok(saisie(true, false, true, "&", "Digit1")), "Ctrl+Alt+1");
        // AZERTY : la touche marquée A est à la place de Q.
        assert_eq!(ok(saisie(true, true, false, "A", "KeyQ")), "Ctrl+Maj+A");
        assert_eq!(ok(saisie(true, false, false, "m", "Semicolon")), "Ctrl+M");
        assert_eq!(
            ok(saisie(false, false, false, "PrintScreen", "PrintScreen")),
            "ImprEcran"
        );
        assert_eq!(ok(saisie(false, false, false, "F9", "F9")), "F9");
        assert_eq!(
            ok(saisie(true, true, false, "ArrowUp", "ArrowUp")),
            "Ctrl+Maj+Haut"
        );
        // Réservés pour plus tard : acceptés comme les autres s'ils sont tapés.
        assert_eq!(ok(saisie(true, true, false, "$", "Digit4")), "Ctrl+Maj+4");
        assert_eq!(ok(saisie(true, true, false, "T", "KeyT")), "Ctrl+Maj+T");
        let refus = |s: Saisie| notation_saisie(&s).unwrap_err().texte;
        assert_eq!(
            refus(saisie(true, false, false, "1", "Numpad1")),
            "Touche non prise en charge"
        );
        assert_eq!(
            refus(saisie(true, true, false, ",", "Comma")),
            "Touche non prise en charge"
        );
        assert_eq!(
            refus(saisie(true, true, false, "€", "KeyE")),
            "Touche non prise en charge"
        );
        assert_eq!(
            refus(saisie(false, false, false, "a", "KeyA")),
            "Ajoutez Ctrl ou Alt"
        );
        assert_eq!(
            refus(saisie(false, true, false, "A", "KeyA")),
            "Ajoutez Ctrl ou Alt"
        );
        assert_eq!(
            refus(saisie(true, false, false, "c", "KeyC")),
            "Réservé par Windows"
        );
        assert_eq!(
            refus(saisie(false, false, true, "F4", "F4")),
            "Réservé par Windows"
        );
        assert_eq!(
            refus(saisie(true, false, false, "x", "KeyX")),
            "Réservé par Windows"
        );
    }

    #[test]
    fn textes_des_refus() {
        assert_eq!(
            texte_refus(&Raison::RefuseParLeSysteme),
            "Pris par une autre application"
        );
        assert_eq!(
            texte_refus(&Raison::DejaPrisParUneAutreAction("fenetre".into())),
            "Déjà pris par « Fenêtre »"
        );
        assert_eq!(
            texte_refus(&Raison::ReserveParLeSysteme),
            "Impr. écran est réservée par Windows"
        );
        for r in [Raison::RefuseParLeSysteme, Raison::ReserveParLeSysteme] {
            assert!(!texte_refus(&r).contains('\n'));
        }
    }

    #[test]
    fn dossiers_choisis() {
        let racine = std::env::temp_dir();
        let defaut = racine.join("Images").join("BreezyShot");
        assert_eq!(dossier_choisi(&defaut, Some(&defaut)), Ok(None));
        let autre = racine.join("Captures");
        assert_eq!(
            dossier_choisi(&autre, Some(&defaut)),
            Ok(Some(autre.clone()))
        );
        assert_eq!(dossier_choisi(&autre, None), Ok(Some(autre)));
        assert_eq!(dossier_choisi(Path::new("relatif"), None), Err("invalide"));
        assert_eq!(dossier_choisi(Path::new(""), None), Err("invalide"));
    }

    #[test]
    fn apercu() {
        let t = Local.with_ymd_and_hms(2026, 11, 12, 10, 42, 8).unwrap();
        assert_eq!(
            apercu_nom("BreezyShot {date} à {heure}", t),
            "BreezyShot 2026-11-12 à 10.42.08.png"
        );
        assert_eq!(apercu_nom("a:b", t), "a-b.png");
        assert_eq!(apercu_nom(" ", t), "");
    }

    #[test]
    fn vue_complete() {
        let r = defaut();
        let mut table = BTreeMap::new();
        for a in ["zone", "plein_ecran", "fenetre"] {
            table.insert(a.to_string(), r.raccourcis.candidats(a).unwrap().to_vec());
        }
        // Impr. écran prise par Windows, Ctrl+Maj+3 pris par une autre application.
        let bilan = choisir(&table, true, |x| x.canonique() != "Ctrl+Maj+3");
        let refus = BTreeMap::new();
        let defaut_r = Raccourcis::par_defaut(Plateforme::Windows);
        let mut r2 = r.clone();
        r2.raccourcis.plein_ecran = vec!["Ctrl+Maj+7".into()];
        let c = Contexte {
            vignette_construite: false,
            capture_disponible: true,
            loupe_disponible: false,
            actions: &["zone", "plein_ecran", "fenetre"],
            bilan: &bilan,
            defaut: &defaut_r,
            refus_raccourcis: &refus,
            dossier_affiche: "Images › BreezyShot".into(),
            demarrage: EtatDemarrage::DesactiveParWindows,
            version: "9.9.9",
            alerte: None,
            maintenant: Local.with_ymd_and_hms(2026, 11, 12, 10, 42, 8).unwrap(),
            saisie_en_cours: false,
        };
        let v = vue(&r2, &c);
        assert_eq!(v.version, "9.9.9");
        assert_eq!(v.demarrage, "non");
        assert_eq!(v.raccourcis[0].touches, ["Ctrl", "Maj", "1"]);
        assert_eq!(
            v.raccourcis[0].refus, None,
            "un repli retenu n'est pas un refus"
        );
        assert!(!v.raccourcis[0].modifie);
        assert!(v.raccourcis[1].modifie);
        assert_eq!(v.raccourcis[2].touches, Vec::<String>::new());
        assert_eq!(v.raccourcis[2].refus.as_deref(), Some(AUCUN_RACCOURCI));
        assert_eq!(v.raccourcis[1].libelle, "Plein écran");
        assert_eq!(v.apercu_nom, "BreezyShot 2026-11-12 à 10.42.08.png");
        let json = serde_json::to_string(&v).unwrap();
        assert!(!json.contains("Users"), "aucun chemin");
        let ailleurs = Contexte {
            demarrage: EtatDemarrage::Indisponible,
            ..c
        };
        assert!(!vue(&r, &ailleurs).lignes.contains(&"demarrage"));
        // Rubrique Capture : valeurs du fichier, loupe lue dans `selection`.
        assert_eq!(v.retardateurs, [0, 3, 5, 10]);
        assert_eq!(v.lignes[..2], ["retardateur", "pointeur"]);
        let mut r3 = r.clone();
        r3.capture.retardateur_s = 7;
        r3.capture.pointeur = true;
        r3.selection.loupe = true;
        let v3 = vue(&r3, &ailleurs);
        assert_eq!(
            v3.capture,
            VueCapture {
                retardateur_s: 7,
                loupe: true,
                pointeur: true
            }
        );
        let sans_gel = Contexte {
            capture_disponible: false,
            loupe_disponible: true,
            ..ailleurs
        };
        let l = vue(&r, &sans_gel).lignes;
        assert!(!l
            .iter()
            .any(|x| ["retardateur", "loupe", "pointeur"].contains(x)));
    }

    #[test]
    fn candidats_fixes() {
        let mut r = Raccourcis::par_defaut(Plateforme::Windows);
        assert!(fixer_candidats(&mut r, "fenetre", vec!["F9".into()]));
        assert_eq!(r.fenetre, ["F9"]);
        assert!(fixer_candidats(
            &mut r,
            "derniere_zone",
            vec!["Ctrl+Maj+9".into()]
        ));
        assert_eq!(r.derniere_zone, ["Ctrl+Maj+9"]);
        assert_eq!(libelle_action("derniere_zone"), "Refaire la dernière zone");
        assert!(!fixer_candidats(&mut r, "annoter", vec![]));
    }

    #[test]
    fn themes() {
        assert_eq!(theme_page(Theme::Systeme), None);
        assert_eq!(theme_page(Theme::Clair), Some(ThemePage::Clair));
        assert_eq!(theme_page(Theme::Sombre), Some(ThemePage::Sombre));
    }
}

/// La vue réelle de l'app assemblée, que l'essai de page charge (`essais/pages/
/// vue-reglages.json`) : la page est essayée sur les lignes que le cœur montre vraiment,
/// et sur le vrai texte du refus de la dernière sortie. Fichier tenu à jour par ce test
/// (`BREEZYSHOT_ECRIRE_VUE=1 cargo test vue_de_l_essai_de_page` le réécrit).
#[cfg(test)]
mod essai_de_page {
    use super::*;
    use crate::raccourcis::choisir;
    use crate::reglages::Plateforme;
    use chrono::TimeZone;
    use std::collections::BTreeMap;

    fn fixture() -> String {
        let r = Reglages::par_defaut(Plateforme::Windows);
        let mut table = BTreeMap::new();
        for a in crate::application::ACTIONS_CONSTRUITES {
            table.insert(a.to_string(), r.raccourcis.candidats(a).unwrap().to_vec());
        }
        let bilan = choisir(&table, true, |_| true);
        let refus = BTreeMap::new();
        let defaut = Raccourcis::par_defaut(Plateforme::Windows);
        let c = Contexte {
            vignette_construite: crate::application::VIGNETTE_CONSTRUITE,
            // La vue de Windows, loupe comprise (chantier C) : l'essai de page retire
            // lui-même « loupe » pour le cas sans loupe. Jamais `gel::disponible()` ici, qui
            // changerait le fichier selon le système de l'essai.
            capture_disponible: true,
            loupe_disponible: true,
            actions: &crate::application::ACTIONS_CONSTRUITES,
            bilan: &bilan,
            defaut: &defaut,
            refus_raccourcis: &refus,
            dossier_affiche: "Images › BreezyShot".into(),
            demarrage: EtatDemarrage::Inactif,
            // Jamais la vraie version : l'essai ne la suppose pas.
            version: "7.8.9-essai",
            alerte: None,
            maintenant: Local.with_ymd_and_hms(2026, 11, 12, 10, 42, 8).unwrap(),
            saisie_en_cours: false,
        };
        // Le refus de la dernière sortie, tel que le cœur le rend : copie seule active.
        let mut seule_copie = r.clone();
        seule_copie.vignette.afficher = false;
        seule_copie.enregistrer_apres_capture = false;
        let refus_copier = appliquer(&seule_copie, &Changement::Copier(false)).unwrap_err();
        let tout = serde_json::json!({ "vue": vue(&r, &c), "refus_copier": refus_copier });
        serde_json::to_string_pretty(&tout).unwrap() + "\n"
    }

    #[test]
    fn vue_de_l_essai_de_page_a_jour() {
        let chemin = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../essais/pages/vue-reglages.json");
        let attendu = fixture();
        if std::env::var_os("BREEZYSHOT_ECRIRE_VUE").is_some() {
            std::fs::write(&chemin, &attendu).unwrap();
        }
        let lu = std::fs::read_to_string(&chemin).unwrap_or_default();
        assert_eq!(
            lu.replace("\r\n", "\n"),
            attendu,
            "essais/pages/vue-reglages.json n'est pas à jour : \
             BREEZYSHOT_ECRIRE_VUE=1 cargo test vue_de_l_essai_de_page"
        );
    }
}
