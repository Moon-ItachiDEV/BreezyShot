//! Notation des raccourcis telle qu'on l'écrit dans les réglages.
//!
//! Exemples : `Ctrl+Maj+1`, `ImprEcran`, `win + maj + s`. La casse, les espaces et les
//! accents des noms de touches (`Entrée`, `Début`) ne comptent pas. Les modificateurs
//! viennent avant la touche, chacun une seule fois. Une touche seule n'est acceptée que
//! pour `ImprEcran` et `F1` à `F24`.

use std::fmt;
use std::str::FromStr;

/// Les modificateurs d'un raccourci. `Win` et `Cmd` sont la même touche (SUPER),
/// `Alt` et `Option` aussi.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modificateurs {
    pub ctrl: bool,
    pub maj: bool,
    pub alt: bool,
    pub win: bool,
}

impl Modificateurs {
    pub fn aucun(&self) -> bool {
        !(self.ctrl || self.maj || self.alt || self.win)
    }
}

/// Une touche que la notation sait nommer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Touche {
    /// `A` à `Z`, toujours rangée en majuscule.
    Lettre(char),
    /// `0` à `9`.
    Chiffre(u8),
    /// `F1` à `F24`.
    F(u8),
    ImprEcran,
    Espace,
    Entree,
    Tab,
    Echap,
    Suppr,
    Inser,
    Debut,
    Fin,
    PagePrec,
    PageSuiv,
    Haut,
    Bas,
    Gauche,
    Droite,
}

/// Touches nommées : (forme canonique, forme pour l'utilisateur, touche).
const TOUCHES_NOMMEES: &[(&str, &str, Touche)] = &[
    ("ImprEcran", "Impr. écran", Touche::ImprEcran),
    ("Espace", "Espace", Touche::Espace),
    ("Entree", "Entrée", Touche::Entree),
    ("Tab", "Tab", Touche::Tab),
    ("Echap", "Échap", Touche::Echap),
    ("Suppr", "Suppr", Touche::Suppr),
    ("Inser", "Inser", Touche::Inser),
    ("Debut", "Début", Touche::Debut),
    ("Fin", "Fin", Touche::Fin),
    ("PagePrec", "Page préc.", Touche::PagePrec),
    ("PageSuiv", "Page suiv.", Touche::PageSuiv),
    ("Haut", "Haut", Touche::Haut),
    ("Bas", "Bas", Touche::Bas),
    ("Gauche", "Gauche", Touche::Gauche),
    ("Droite", "Droite", Touche::Droite),
];

impl Touche {
    /// Vrai si la touche peut servir seule, sans modificateur.
    pub fn accepte_seule(&self) -> bool {
        matches!(self, Touche::ImprEcran | Touche::F(_))
    }

    fn ecrire(&self, pour_utilisateur: bool) -> String {
        match self {
            Touche::Lettre(c) => c.to_string(),
            Touche::Chiffre(n) => n.to_string(),
            Touche::F(n) => format!("F{n}"),
            nommee => TOUCHES_NOMMEES
                .iter()
                .find(|(_, _, t)| t == nommee)
                .map(|(canon, aff, _)| if pour_utilisateur { *aff } else { *canon })
                .unwrap_or("?")
                .to_string(),
        }
    }

    /// Lit un nom de touche déjà mis en minuscules, sans espaces ni accents.
    fn lire(simplifie: &str) -> Option<Touche> {
        let mut caracteres = simplifie.chars();
        if let (Some(c), None) = (caracteres.next(), caracteres.next()) {
            if c.is_ascii_lowercase() {
                return Some(Touche::Lettre(c.to_ascii_uppercase()));
            }
            if let Some(n) = c.to_digit(10) {
                return Some(Touche::Chiffre(n as u8));
            }
        }
        if let Some(reste) = simplifie.strip_prefix('f') {
            // « F01 » ou « F+1 » ne sont pas acceptés : chiffres simples seulement.
            if !reste.is_empty() && !reste.starts_with('0') && reste.len() <= 2 {
                if let Ok(n) = reste.parse::<u8>() {
                    if (1..=24).contains(&n) {
                        return Some(Touche::F(n));
                    }
                }
            }
        }
        TOUCHES_NOMMEES
            .iter()
            .find(|(canon, _, _)| canon.to_ascii_lowercase() == simplifie)
            .map(|(_, _, t)| *t)
    }
}

/// Un raccourci : des modificateurs puis une touche.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Raccourci {
    pub modificateurs: Modificateurs,
    pub touche: Touche,
}

/// Pourquoi une notation n'a pas pu être lue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErreurNotation {
    /// Chaîne vide ou faite d'espaces.
    Vide,
    /// Un morceau vide entre deux `+` (`Ctrl++1`, `Ctrl+`).
    MorceauVide,
    /// Un nom qui n'est ni un modificateur ni une touche connue.
    ToucheInconnue(String),
    /// Le même modificateur deux fois (`Ctrl+Ctrl+1`, `Win+Cmd+1`).
    ModificateurEnDouble(String),
    /// Seulement des modificateurs (`Ctrl+Maj`).
    SansTouche,
    /// Une touche ailleurs qu'en dernière position (`1+Ctrl`, `A+B`).
    ToucheAvantLaFin(String),
    /// Une touche seule qui n'a pas le droit de l'être (`A`, `Espace`).
    ToucheSeuleInterdite(String),
    /// `Maj` comme seul modificateur, hors `F1`–`F24` et `ImprEcran` (`Maj+A` gênerait la
    /// saisie des majuscules). Porte la forme canonique.
    MajSeuleInterdite(String),
    /// Un raccourci que le système ou toutes les applications utilisent déjà
    /// (`RESERVES_AU_SYSTEME`). Porte la forme canonique.
    ReserveAuSysteme(String),
}

/// Raccourcis jamais acceptés, en forme canonique : copier, coller, annuler, fermer,
/// verrouiller… BreezyShot ne doit jamais les prendre aux autres applications.
pub const RESERVES_AU_SYSTEME: [&str; 11] = [
    "Ctrl+C",
    "Ctrl+V",
    "Ctrl+X",
    "Ctrl+Z",
    "Ctrl+A",
    "Ctrl+S",
    "Alt+F4",
    "Alt+Tab",
    "Alt+Espace",
    "Ctrl+Alt+Suppr",
    "Win+L",
];

impl fmt::Display for ErreurNotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErreurNotation::Vide => write!(f, "raccourci vide"),
            ErreurNotation::MorceauVide => write!(f, "un « + » de trop"),
            ErreurNotation::ToucheInconnue(nom) => write!(f, "touche inconnue « {nom} »"),
            ErreurNotation::ModificateurEnDouble(nom) => {
                write!(f, "modificateur « {nom} » en double")
            }
            ErreurNotation::SansTouche => write!(f, "aucune touche après les modificateurs"),
            ErreurNotation::ToucheAvantLaFin(nom) => {
                write!(f, "la touche « {nom} » doit être en dernier")
            }
            ErreurNotation::ToucheSeuleInterdite(nom) => {
                write!(f, "la touche « {nom} » ne peut pas servir seule")
            }
            ErreurNotation::MajSeuleInterdite(nom) => {
                write!(f, "« {nom} » : Maj seule ne suffit pas comme modificateur")
            }
            ErreurNotation::ReserveAuSysteme(nom) => {
                write!(f, "« {nom} » est réservé au système")
            }
        }
    }
}

impl std::error::Error for ErreurNotation {}

/// Minuscules, sans espaces, sans points ni accents (pour comparer les noms) : la forme
/// d'affichage (`Impr. écran`, `Page préc.`) se relit donc aussi.
/// Une lettre accentuée seule (`é`) n'est pas repliée : ce n'est pas la touche `E`.
fn simplifier(morceau: &str) -> String {
    let minuscules: String = morceau
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '.')
        .flat_map(char::to_lowercase)
        .collect();
    if minuscules.chars().count() <= 1 {
        return minuscules;
    }
    minuscules
        .chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'à' | 'â' => 'a',
            'î' | 'ï' => 'i',
            'ô' => 'o',
            'û' | 'ù' => 'u',
            'ç' => 'c',
            autre => autre,
        })
        .collect()
}

/// Rend `Some(true)` pour un modificateur déjà rangé, `Some(false)` pour un nouveau,
/// `None` si le nom n'est pas un modificateur.
fn lire_modificateur(simplifie: &str, m: &mut Modificateurs) -> Option<bool> {
    let case = match simplifie {
        "ctrl" => &mut m.ctrl,
        "maj" => &mut m.maj,
        "alt" | "option" => &mut m.alt,
        "win" | "cmd" => &mut m.win,
        _ => return None,
    };
    let deja = *case;
    *case = true;
    Some(deja)
}

/// Lit une notation de raccourci.
pub fn analyser(texte: &str) -> Result<Raccourci, ErreurNotation> {
    if texte.trim().is_empty() {
        return Err(ErreurNotation::Vide);
    }
    let morceaux: Vec<&str> = texte.split('+').collect();
    let dernier = morceaux.len() - 1;
    let mut modificateurs = Modificateurs::default();
    let mut touche = None;
    for (rang, brut) in morceaux.iter().enumerate() {
        let simple = simplifier(brut);
        if simple.is_empty() {
            return Err(ErreurNotation::MorceauVide);
        }
        if let Some(deja) = lire_modificateur(&simple, &mut modificateurs) {
            if deja {
                return Err(ErreurNotation::ModificateurEnDouble(
                    brut.trim().to_string(),
                ));
            }
            if rang == dernier {
                return Err(ErreurNotation::SansTouche);
            }
            continue;
        }
        match Touche::lire(&simple) {
            None => return Err(ErreurNotation::ToucheInconnue(brut.trim().to_string())),
            Some(_) if rang != dernier => {
                return Err(ErreurNotation::ToucheAvantLaFin(brut.trim().to_string()))
            }
            Some(t) => touche = Some(t),
        }
    }
    let touche = touche.ok_or(ErreurNotation::SansTouche)?;
    if modificateurs.aucun() && !touche.accepte_seule() {
        return Err(ErreurNotation::ToucheSeuleInterdite(touche.ecrire(false)));
    }
    let raccourci = Raccourci {
        modificateurs,
        touche,
    };
    let seule_maj = Modificateurs {
        maj: true,
        ..Modificateurs::default()
    };
    if modificateurs == seule_maj && !touche.accepte_seule() {
        return Err(ErreurNotation::MajSeuleInterdite(raccourci.canonique()));
    }
    let canon = raccourci.canonique();
    if RESERVES_AU_SYSTEME.contains(&canon.as_str()) {
        return Err(ErreurNotation::ReserveAuSysteme(canon));
    }
    Ok(raccourci)
}

impl Raccourci {
    fn assembler(&self, pour_utilisateur: bool) -> String {
        let m = &self.modificateurs;
        let mut morceaux: Vec<String> = [
            (m.ctrl, "Ctrl"),
            (m.maj, "Maj"),
            (m.alt, "Alt"),
            (m.win, "Win"),
        ]
        .iter()
        .filter(|(present, _)| *present)
        .map(|(_, nom)| nom.to_string())
        .collect();
        morceaux.push(self.touche.ecrire(pour_utilisateur));
        morceaux.join(if pour_utilisateur { " + " } else { "+" })
    }

    /// Forme canonique, celle qu'on réécrit dans les réglages : `Ctrl+Maj+1`, `ImprEcran`.
    pub fn canonique(&self) -> String {
        self.assembler(false)
    }

    /// Forme montrée à l'utilisateur : `Ctrl + Maj + 1`, `Impr. écran`.
    pub fn affichage(&self) -> String {
        self.assembler(true)
    }

    /// Vrai pour la touche Impr. écran seule, celle que Windows peut réserver.
    pub fn est_impr_ecran_seule(&self) -> bool {
        self.touche == Touche::ImprEcran && self.modificateurs.aucun()
    }
}

/// `to_string()` donne la forme canonique.
impl fmt::Display for Raccourci {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonique())
    }
}

impl FromStr for Raccourci {
    type Err = ErreurNotation;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        analyser(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canon(texte: &str) -> String {
        analyser(texte).unwrap().canonique()
    }

    #[test]
    fn formes_canonique_et_affichage() {
        let r = analyser("Ctrl+Maj+1").unwrap();
        assert_eq!(r.canonique(), "Ctrl+Maj+1");
        assert_eq!(r.affichage(), "Ctrl + Maj + 1");
        assert_eq!(r.to_string(), "Ctrl+Maj+1");
        let r = analyser("ImprEcran").unwrap();
        assert_eq!(r.canonique(), "ImprEcran");
        assert_eq!(r.affichage(), "Impr. écran");
        assert!(r.est_impr_ecran_seule());
        assert_eq!(canon("Alt+Maj+PagePrec"), "Maj+Alt+PagePrec");
        assert_eq!(
            analyser("Ctrl+Entree").unwrap().affichage(),
            "Ctrl + Entrée"
        );
    }

    #[test]
    fn casse_espaces_et_accents_ignores() {
        for variante in [
            "ctrl+maj+1",
            "CTRL + MAJ + 1",
            "  Ctrl +Maj+ 1 ",
            "Maj+Ctrl+1",
            "c t r l+maj+1",
        ] {
            assert_eq!(canon(variante), "Ctrl+Maj+1", "{variante}");
        }
        assert_eq!(canon("impr ecran"), "ImprEcran");
        assert_eq!(canon("ImprÉcran"), "ImprEcran");
        assert_eq!(canon("ctrl+entrée"), "Ctrl+Entree");
        assert_eq!(canon("alt+début"), "Alt+Debut");
        assert_eq!(canon("ctrl+b"), "Ctrl+B");
        assert_eq!(canon("f12"), "F12");
        assert_eq!(canon("ctrl+F24"), "Ctrl+F24");
    }

    #[test]
    fn modificateurs_du_mac() {
        assert_eq!(analyser("Cmd+Maj+3"), analyser("Win+Maj+3"));
        assert_eq!(analyser("Option+A"), analyser("Alt+A"));
        assert_eq!(canon("cmd+option+ctrl+maj+z"), "Ctrl+Maj+Alt+Win+Z");
    }

    #[test]
    fn aller_retour_sur_toutes_les_touches() {
        let mut textes: Vec<String> = Vec::new();
        textes.extend(('A'..='Z').map(|c| format!("Ctrl+Alt+{c}")));
        textes.extend((0..=9).map(|n| format!("Maj+Alt+{n}")));
        textes.extend((1..=24).map(|n| format!("F{n}")));
        textes.extend(
            TOUCHES_NOMMEES
                .iter()
                .map(|(c, _, _)| format!("Ctrl+Win+{c}")),
        );
        textes.push("ImprEcran".into());
        textes.push("Ctrl+Maj+Alt+Win+ImprEcran".into());
        for t in textes {
            let r = analyser(&t).unwrap();
            assert_eq!(r.canonique(), t);
            assert_eq!(analyser(&r.canonique()).unwrap(), r);
            assert_eq!(analyser(&r.affichage()).unwrap(), r, "affichage relu : {t}");
        }
    }

    #[test]
    fn touche_seule() {
        assert!(analyser("ImprEcran").is_ok());
        assert!(analyser("F1").is_ok());
        assert!(analyser("F24").is_ok());
        assert_eq!(
            analyser("A"),
            Err(ErreurNotation::ToucheSeuleInterdite("A".into()))
        );
        assert_eq!(
            analyser("1"),
            Err(ErreurNotation::ToucheSeuleInterdite("1".into()))
        );
        assert_eq!(
            analyser("espace"),
            Err(ErreurNotation::ToucheSeuleInterdite("Espace".into()))
        );
        assert!(matches!(
            analyser("Echap"),
            Err(ErreurNotation::ToucheSeuleInterdite(_))
        ));
    }

    #[test]
    fn erreurs() {
        assert_eq!(analyser(""), Err(ErreurNotation::Vide));
        assert_eq!(analyser("   "), Err(ErreurNotation::Vide));
        assert_eq!(analyser("Ctrl++1"), Err(ErreurNotation::MorceauVide));
        assert_eq!(analyser("Ctrl+"), Err(ErreurNotation::MorceauVide));
        assert_eq!(analyser("+1"), Err(ErreurNotation::MorceauVide));
        assert_eq!(analyser("Ctrl+Maj"), Err(ErreurNotation::SansTouche));
        assert_eq!(analyser("Ctrl"), Err(ErreurNotation::SansTouche));
        assert_eq!(
            analyser("Ctrl+Ctrl+1"),
            Err(ErreurNotation::ModificateurEnDouble("Ctrl".into()))
        );
        assert_eq!(
            analyser("Win+Cmd+1"),
            Err(ErreurNotation::ModificateurEnDouble("Cmd".into()))
        );
        assert_eq!(
            analyser("1+Ctrl"),
            Err(ErreurNotation::ToucheAvantLaFin("1".into()))
        );
        assert_eq!(
            analyser("Ctrl+A+B"),
            Err(ErreurNotation::ToucheAvantLaFin("A".into()))
        );
        for inconnu in [
            "Ctrl+F25",
            "Ctrl+F0",
            "Ctrl+F01",
            "Shift+1",
            "Ctrl+é",
            "Ctrl+Maj+PrintScreen",
            "Ctrl+12",
        ] {
            assert!(
                matches!(analyser(inconnu), Err(ErreurNotation::ToucheInconnue(_))),
                "{inconnu}"
            );
        }
        assert_eq!(
            analyser("Ctrl + Machin"),
            Err(ErreurNotation::ToucheInconnue("Machin".into()))
        );
    }

    #[test]
    fn maj_seule() {
        assert_eq!(
            analyser("maj + a"),
            Err(ErreurNotation::MajSeuleInterdite("Maj+A".into()))
        );
        for t in ["Maj+1", "Maj+Espace", "Maj+Entree", "Maj+Suppr", "Maj+Haut"] {
            assert!(
                matches!(analyser(t), Err(ErreurNotation::MajSeuleInterdite(_))),
                "{t}"
            );
        }
        for t in [
            "Maj+F1",
            "Maj+F24",
            "Maj+ImprEcran",
            "Ctrl+Maj+A",
            "Maj+Alt+1",
            "Maj+Win+S",
        ] {
            assert!(analyser(t).is_ok(), "{t}");
        }
    }

    #[test]
    fn reserves_au_systeme() {
        for canon in RESERVES_AU_SYSTEME {
            assert_eq!(
                analyser(canon),
                Err(ErreurNotation::ReserveAuSysteme(canon.to_string())),
                "{canon}"
            );
        }
        // Les autres écritures du même raccourci aussi.
        for (t, canon) in [
            ("ctrl + c", "Ctrl+C"),
            ("Option+F4", "Alt+F4"),
            ("Cmd+L", "Win+L"),
            ("Alt+Ctrl+Suppr", "Ctrl+Alt+Suppr"),
            ("alt+espace", "Alt+Espace"),
        ] {
            assert_eq!(
                analyser(t),
                Err(ErreurNotation::ReserveAuSysteme(canon.into()))
            );
        }
        // Un voisin n'est pas visé.
        for t in [
            "Ctrl+Maj+C",
            "Ctrl+B",
            "Alt+F5",
            "Win+K",
            "Ctrl+Maj+Alt+Suppr",
        ] {
            assert!(analyser(t).is_ok(), "{t}");
        }
    }

    #[test]
    fn from_str() {
        let r: Raccourci = "ctrl+maj+2".parse().unwrap();
        assert_eq!(r.canonique(), "Ctrl+Maj+2");
    }
}
