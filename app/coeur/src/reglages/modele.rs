//! Les types des réglages, leurs valeurs par défaut et leur forme JSON.

use std::path::PathBuf;

use serde_json::{Map, Value};

/// Version du format que cette app sait écrire. Elle découle de la liste des mises à niveau
/// (`lecture::MISES_A_NIVEAU`) : une fonction de plus, une version de plus.
pub const VERSION_ACTUELLE: u32 = super::lecture::MISES_A_NIVEAU.len() as u32;

/// Durée d'affichage de la vignette permise, en secondes (bornes comprises).
pub const DUREE_MIN_S: u32 = 1;
pub const DUREE_MAX_S: u32 = 60;
/// `vignette.duree_s = 0` : la vignette reste affichée (version 2 du fichier).
pub const DUREE_TOUJOURS: u32 = 0;

/// Le système pour lequel on choisit les valeurs par défaut. Le module ne le devine pas
/// lui-même (aucun `cfg` hors de `systeme/`) : l'Application le lui donne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plateforme {
    Windows,
    Mac,
    /// Linux et le reste, pour le développement : mêmes valeurs que Windows.
    Autre,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coin {
    BasGauche,
    BasDroite,
    HautGauche,
    HautDroite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Animations {
    Systeme,
    Normales,
    Reduites,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Systeme,
    Clair,
    Sombre,
}

/// Fait le lien entre une énumération et son texte dans le fichier.
macro_rules! texte_enum {
    ($type:ident { $($variante:ident => $texte:literal),+ $(,)? }) => {
        impl $type {
            /// Le texte écrit dans le fichier.
            pub fn texte(self) -> &'static str {
                match self { $($type::$variante => $texte),+ }
            }
            /// L'inverse de `texte` ; `None` pour une valeur inconnue.
            pub fn depuis_texte(texte: &str) -> Option<Self> {
                match texte { $($texte => Some($type::$variante),)+ _ => None }
            }
        }
    };
}

texte_enum!(Coin {
    BasGauche => "bas_gauche",
    BasDroite => "bas_droite",
    HautGauche => "haut_gauche",
    HautDroite => "haut_droite",
});
texte_enum!(Animations { Systeme => "systeme", Normales => "normales", Reduites => "reduites" });
texte_enum!(Theme { Systeme => "systeme", Clair => "clair", Sombre => "sombre" });

/// Raccourcis rangés par nom d'action : pour chacune, les candidats par ordre de préférence.
/// Les chaînes ne sont pas interprétées ici (c'est le travail du module `raccourcis`).
#[derive(Debug, Clone, PartialEq)]
pub struct Raccourcis {
    pub zone: Vec<String>,
    pub plein_ecran: Vec<String>,
    pub fenetre: Vec<String>,
    /// Actions que cette version ne connaît pas, gardées telles quelles pour la réécriture.
    pub(in crate::reglages) autres: Map<String, Value>,
}

impl Raccourcis {
    /// Les noms d'action connus, dans l'ordre où ils sont écrits.
    pub const ACTIONS: [&'static str; 3] = ["zone", "plein_ecran", "fenetre"];

    /// Les candidats d'une action connue, `None` pour une action inconnue.
    pub fn candidats(&self, action: &str) -> Option<&[String]> {
        match action {
            "zone" => Some(&self.zone),
            "plein_ecran" => Some(&self.plein_ecran),
            "fenetre" => Some(&self.fenetre),
            _ => None,
        }
    }

    pub(in crate::reglages) fn candidats_mut(&mut self, action: &str) -> Option<&mut Vec<String>> {
        match action {
            "zone" => Some(&mut self.zone),
            "plein_ecran" => Some(&mut self.plein_ecran),
            "fenetre" => Some(&mut self.fenetre),
            _ => None,
        }
    }

    /// Valeurs par défaut : `ImprEcran` n'existe pas sur Mac.
    pub fn par_defaut(plateforme: Plateforme) -> Self {
        let liste = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        match plateforme {
            Plateforme::Windows | Plateforme::Autre => Raccourcis {
                zone: liste(&["ImprEcran", "Ctrl+Maj+1"]),
                plein_ecran: liste(&["Ctrl+Maj+2"]),
                fenetre: liste(&["Ctrl+Maj+3"]),
                autres: Map::new(),
            },
            // Provisoire : les raccourcis du Mac restent à choisir (Cmd+Maj+3, 4 et 5 sont
            // ceux de macOS lui-même, on les évite).
            Plateforme::Mac => Raccourcis {
                zone: liste(&["Cmd+Maj+1"]),
                plein_ecran: liste(&["Cmd+Maj+2"]),
                fenetre: liste(&["Cmd+Maj+6"]),
                autres: Map::new(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Vignette {
    pub afficher: bool,
    /// Entre `DUREE_MIN_S` et `DUREE_MAX_S`, en secondes entières, ou `DUREE_TOUJOURS`.
    pub duree_s: u32,
    pub coin: Coin,
    pub(in crate::reglages) inconnus: Map<String, Value>,
}

impl Vignette {
    pub fn par_defaut() -> Self {
        Vignette {
            afficher: true,
            duree_s: 6,
            coin: Coin::BasGauche,
            inconnus: Map::new(),
        }
    }
}

/// Réglages de la sélection de zone (étape 5).
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub loupe: bool,
    pub(in crate::reglages) inconnus: Map<String, Value>,
}

impl Selection {
    pub fn par_defaut() -> Self {
        Selection {
            loupe: false,
            inconnus: Map::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Reglages {
    pub raccourcis: Raccourcis,
    pub copier_apres_capture: bool,
    pub enregistrer_apres_capture: bool,
    pub vignette: Vignette,
    pub selection: Selection,
    pub demarrer_avec_systeme: bool,
    /// `None` = Images › BreezyShot (résolu par `systeme`).
    pub dossier_enregistrement: Option<PathBuf>,
    pub modele_nom: String,
    pub animations: Animations,
    pub theme: Theme,
    /// Champs inconnus du fichier, gardés tels quels pour la réécriture.
    pub(in crate::reglages) inconnus: Map<String, Value>,
    /// Version du fichier lu (`VERSION_ACTUELLE` sauf pour un fichier plus récent).
    pub(in crate::reglages) version_fichier: u32,
    /// Vrai quand le fichier n'a pas pu être lu ou mis de côté : on ne l'écrase jamais.
    pub(in crate::reglages) protege: bool,
}

impl Reglages {
    /// Valeurs par défaut. Seuls les raccourcis dépendent du système, d'où le paramètre.
    pub fn par_defaut(plateforme: Plateforme) -> Self {
        Reglages {
            raccourcis: Raccourcis::par_defaut(plateforme),
            copier_apres_capture: true,
            enregistrer_apres_capture: false,
            vignette: Vignette::par_defaut(),
            selection: Selection::par_defaut(),
            demarrer_avec_systeme: false,
            dossier_enregistrement: None,
            modele_nom: "BreezyShot {date} à {heure}".to_string(),
            animations: Animations::Systeme,
            theme: Theme::Systeme,
            inconnus: Map::new(),
            version_fichier: VERSION_ACTUELLE,
            protege: false,
        }
    }

    /// Vrai si au moins une sortie (copie, enregistrement, vignette) est active.
    pub fn une_sortie_active(&self) -> bool {
        self.copier_apres_capture || self.enregistrer_apres_capture || self.vignette.afficher
    }

    /// Version du fichier d'où viennent ces réglages.
    pub fn version_fichier(&self) -> u32 {
        self.version_fichier
    }

    /// Faux si `ecrire` refusera ces réglages quoi qu'on y change : fichier d'une version plus
    /// récente, ou fichier qu'on n'a pas pu lire ou mettre de côté. Vrai ne garantit pas
    /// l'écriture : `ecrire` refuse aussi, par exemple, un fichier passé en lecture seule ou
    /// devenu illisible depuis, une valeur invalide ou une combinaison sans sortie active.
    pub fn enregistrable(&self) -> bool {
        !self.protege && self.version_fichier <= VERSION_ACTUELLE
    }

    /// Vrai si ces réglages sont les valeurs par défaut mises à la place d'un fichier qu'on
    /// n'a pas pu lire ou mettre de côté (verrou, droits) : ils ne disent rien du fichier.
    pub fn protege(&self) -> bool {
        self.protege
    }

    /// Valeurs par défaut marquées « protégées » (voir `enregistrable`).
    pub(in crate::reglages) fn par_defaut_protege(plateforme: Plateforme) -> Self {
        Reglages {
            protege: true,
            ..Reglages::par_defaut(plateforme)
        }
    }
}

/// Forme JSON des réglages : champs connus dans l'ordre du contrat, puis les champs inconnus
/// dans leur ordre d'origine. Fonction pure. `Err(champ)` si un chemin n'est pas en UTF-8.
pub(in crate::reglages) fn vers_json(r: &Reglages) -> Result<Value, &'static str> {
    let liste = |l: &[String]| Value::Array(l.iter().cloned().map(Value::String).collect());

    let mut raccourcis = Map::new();
    for action in Raccourcis::ACTIONS {
        if let Some(c) = r.raccourcis.candidats(action) {
            raccourcis.insert(action.to_string(), liste(c));
        }
    }
    for (cle, valeur) in &r.raccourcis.autres {
        raccourcis.insert(cle.clone(), valeur.clone());
    }

    let mut vignette = Map::new();
    vignette.insert("afficher".into(), Value::Bool(r.vignette.afficher));
    vignette.insert("duree_s".into(), Value::from(r.vignette.duree_s));
    vignette.insert("coin".into(), Value::from(r.vignette.coin.texte()));
    for (cle, valeur) in &r.vignette.inconnus {
        vignette.insert(cle.clone(), valeur.clone());
    }

    let mut selection = Map::new();
    selection.insert("loupe".into(), Value::Bool(r.selection.loupe));
    for (cle, valeur) in &r.selection.inconnus {
        selection.insert(cle.clone(), valeur.clone());
    }

    let dossier = match &r.dossier_enregistrement {
        None => Value::Null,
        Some(chemin) => match chemin.to_str() {
            Some(texte) => Value::from(texte),
            None => return Err("dossier_enregistrement"),
        },
    };

    let mut m = Map::new();
    m.insert("version".into(), Value::from(r.version_fichier));
    m.insert("raccourcis".into(), Value::Object(raccourcis));
    m.insert(
        "copier_apres_capture".into(),
        Value::Bool(r.copier_apres_capture),
    );
    m.insert(
        "enregistrer_apres_capture".into(),
        Value::Bool(r.enregistrer_apres_capture),
    );
    m.insert("vignette".into(), Value::Object(vignette));
    m.insert("selection".into(), Value::Object(selection));
    m.insert(
        "demarrer_avec_systeme".into(),
        Value::Bool(r.demarrer_avec_systeme),
    );
    m.insert("dossier_enregistrement".into(), dossier);
    m.insert("modele_nom".into(), Value::from(r.modele_nom.as_str()));
    m.insert("animations".into(), Value::from(r.animations.texte()));
    m.insert("theme".into(), Value::from(r.theme.texte()));
    for (cle, valeur) in &r.inconnus {
        m.insert(cle.clone(), valeur.clone());
    }
    Ok(Value::Object(m))
}
