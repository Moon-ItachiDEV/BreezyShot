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
/// Retardateur permis, en secondes entières : 0 (aucun) à `RETARDATEUR_MAX_S` (version 3).
pub const RETARDATEUR_MAX_S: u32 = 10;

/// Fluidités d'une vidéo permises, en images par seconde (version 7).
pub const FLUIDITES: [u32; 2] = [30, 60];
/// Nombres de captures gardées dans l'historique permis (version 7) ; `0` : aucune
/// (seule la dernière capture reste, le temps de la vignette).
pub const HISTORIQUE_CHOIX: [u32; 5] = [0, 25, 50, 100, 200];
/// Nombre de captures gardées par défaut, et sans licence.
pub const HISTORIQUE_DEFAUT: u32 = 50;

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

/// Le format du fichier enregistré après une capture (version 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatImage {
    Png,
    Jpg,
}

/// Ce que lance la touche Impr. écran quand BreezyShot la tient (version 8). L'Outil Capture
/// de Windows n'y figure pas : rendre la touche à Windows, c'est le registre
/// (`systeme::donner_impr_ecran`), pas ce fichier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CibleImprEcran {
    /// La barre de capture (ce n'est pas une action : aucune ligne dans Raccourcis).
    Barre,
    Zone,
    PleinEcran,
    Fenetre,
    DerniereZone,
    Pipette,
    Defilante,
    Texte,
    Video,
    Gif,
    Historique,
}

impl CibleImprEcran {
    /// Les cibles dans l'ordre de la liste de la page.
    pub const TOUTES: [CibleImprEcran; 11] = [
        CibleImprEcran::Barre,
        CibleImprEcran::Zone,
        CibleImprEcran::PleinEcran,
        CibleImprEcran::Fenetre,
        CibleImprEcran::DerniereZone,
        CibleImprEcran::Pipette,
        CibleImprEcran::Defilante,
        CibleImprEcran::Texte,
        CibleImprEcran::Video,
        CibleImprEcran::Gif,
        CibleImprEcran::Historique,
    ];

    /// Le nom de l'action lancée, `None` pour la barre (qui n'est pas une action).
    pub fn action(self) -> Option<&'static str> {
        match self {
            CibleImprEcran::Barre => None,
            CibleImprEcran::Zone => Some("zone"),
            CibleImprEcran::PleinEcran => Some("plein_ecran"),
            CibleImprEcran::Fenetre => Some("fenetre"),
            CibleImprEcran::DerniereZone => Some("derniere_zone"),
            CibleImprEcran::Pipette => Some("pipette"),
            CibleImprEcran::Defilante => Some("defilante"),
            CibleImprEcran::Texte => Some("texte"),
            CibleImprEcran::Video => Some("video"),
            CibleImprEcran::Gif => Some("gif"),
            CibleImprEcran::Historique => Some("historique"),
        }
    }

    /// L'inverse de `action`.
    pub fn depuis_action(action: &str) -> Option<Self> {
        Self::TOUTES
            .into_iter()
            .find(|c| c.action() == Some(action))
    }
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
texte_enum!(FormatImage { Png => "png", Jpg => "jpg" });
texte_enum!(Animations { Systeme => "systeme", Normales => "normales", Reduites => "reduites" });
texte_enum!(CibleImprEcran {
    Barre => "barre",
    Zone => "zone",
    PleinEcran => "plein_ecran",
    Fenetre => "fenetre",
    DerniereZone => "derniere_zone",
    Pipette => "pipette",
    Defilante => "defilante",
    Texte => "texte",
    Video => "video",
    Gif => "gif",
    Historique => "historique",
});
texte_enum!(Theme { Systeme => "systeme", Clair => "clair", Sombre => "sombre" });

/// Raccourcis rangés par nom d'action : pour chacune, les candidats par ordre de préférence.
/// Les chaînes ne sont pas interprétées ici (c'est le travail du module `raccourcis`).
#[derive(Debug, Clone, PartialEq)]
pub struct Raccourcis {
    pub zone: Vec<String>,
    pub plein_ecran: Vec<String>,
    pub fenetre: Vec<String>,
    /// Refaire la dernière zone (version 3).
    pub derniere_zone: Vec<String>,
    /// Ouvrir l'historique (version 4).
    pub historique: Vec<String>,
    /// Copier le texte (version 4).
    pub texte: Vec<String>,
    /// La capture défilante (version 5).
    pub defilante: Vec<String>,
    /// La vidéo (version 6).
    pub video: Vec<String>,
    /// Le GIF (version 6).
    pub gif: Vec<String>,
    /// La pipette de couleur (version 11).
    pub pipette: Vec<String>,
    /// Actions que cette version ne connaît pas, gardées telles quelles pour la réécriture.
    pub(in crate::reglages) autres: Map<String, Value>,
}

impl Raccourcis {
    /// Les noms d'action connus, dans l'ordre où ils sont écrits.
    pub const ACTIONS: [&'static str; 10] = [
        "zone",
        "plein_ecran",
        "fenetre",
        "derniere_zone",
        "historique",
        "texte",
        "defilante",
        "video",
        "gif",
        "pipette",
    ];

    /// Les candidats d'une action connue, `None` pour une action inconnue.
    pub fn candidats(&self, action: &str) -> Option<&[String]> {
        match action {
            "zone" => Some(&self.zone),
            "plein_ecran" => Some(&self.plein_ecran),
            "fenetre" => Some(&self.fenetre),
            "derniere_zone" => Some(&self.derniere_zone),
            "historique" => Some(&self.historique),
            "texte" => Some(&self.texte),
            "defilante" => Some(&self.defilante),
            "video" => Some(&self.video),
            "gif" => Some(&self.gif),
            "pipette" => Some(&self.pipette),
            _ => None,
        }
    }

    pub(in crate::reglages) fn candidats_mut(&mut self, action: &str) -> Option<&mut Vec<String>> {
        match action {
            "zone" => Some(&mut self.zone),
            "plein_ecran" => Some(&mut self.plein_ecran),
            "fenetre" => Some(&mut self.fenetre),
            "derniere_zone" => Some(&mut self.derniere_zone),
            "historique" => Some(&mut self.historique),
            "texte" => Some(&mut self.texte),
            "defilante" => Some(&mut self.defilante),
            "video" => Some(&mut self.video),
            "gif" => Some(&mut self.gif),
            "pipette" => Some(&mut self.pipette),
            _ => None,
        }
    }

    /// Valeurs par défaut. `ImprEcran` n'est dans aucune liste (version 8) : c'est la cible
    /// `Reglages::impr_ecran_ouvre` qui la tient ; elle n'existe pas sur Mac.
    pub fn par_defaut(plateforme: Plateforme) -> Self {
        let liste = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        match plateforme {
            Plateforme::Windows | Plateforme::Autre => Raccourcis {
                zone: liste(&["Ctrl+Maj+1"]),
                plein_ecran: liste(&["Ctrl+Maj+2"]),
                fenetre: liste(&["Ctrl+Maj+3"]),
                derniere_zone: liste(&["Ctrl+Maj+4"]),
                historique: liste(&["Ctrl+Maj+H"]),
                texte: liste(&["Ctrl+Maj+T"]),
                defilante: liste(&["Ctrl+Maj+5"]),
                video: liste(&["Ctrl+Maj+6"]),
                gif: liste(&["Ctrl+Maj+7"]),
                pipette: liste(&["Ctrl+Maj+C"]),
                autres: Map::new(),
            },
            // Provisoire : les raccourcis du Mac restent à choisir (Cmd+Maj+3, 4 et 5 sont
            // ceux de macOS lui-même, on les évite ; Cmd+Maj+0 pour la défilante, contrat 0.6
            // §7.1). Vidéo et GIF : aucun (la fonction est absente du Mac, et ses chiffres
            // libres sont pris ; contrat 0.7, §7.1).
            Plateforme::Mac => Raccourcis {
                zone: liste(&["Cmd+Maj+1"]),
                plein_ecran: liste(&["Cmd+Maj+2"]),
                fenetre: liste(&["Cmd+Maj+6"]),
                derniere_zone: liste(&["Cmd+Maj+7"]),
                historique: liste(&["Cmd+Maj+8"]),
                texte: liste(&["Cmd+Maj+9"]),
                defilante: liste(&["Cmd+Maj+0"]),
                video: Vec::new(),
                gif: Vec::new(),
                pipette: liste(&["Cmd+Maj+C"]),
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

/// Réglages de la capture (version 3) : retardateur, pointeur et icônes du bureau.
#[derive(Debug, Clone, PartialEq)]
pub struct Capture {
    /// 0 (aucun) ou 1 à `RETARDATEUR_MAX_S` secondes entières.
    pub retardateur_s: u32,
    /// Le pointeur est dessiné dans l'image.
    pub pointeur: bool,
    /// Masquer temporairement les icônes du bureau pendant un plein écran ou un film.
    pub masquer_icones_bureau: bool,
    pub(in crate::reglages) inconnus: Map<String, Value>,
}

impl Capture {
    pub fn par_defaut() -> Self {
        Capture {
            retardateur_s: 0,
            pointeur: false,
            masquer_icones_bureau: false,
            inconnus: Map::new(),
        }
    }
}

/// Réglages des films, vidéo et GIF (version 7, Pro).
#[derive(Debug, Clone, PartialEq)]
pub struct Film {
    /// Images par seconde d'une vidéo : une valeur de `FLUIDITES`.
    pub fluidite: u32,
    /// Le son de l'ordinateur est mêlé à la vidéo.
    pub son_systeme: bool,
    /// Un anneau est dessiné à chaque clic.
    pub clics_visibles: bool,
    /// `None` = le dossier d'enregistrement des images.
    pub dossier: Option<PathBuf>,
    pub(in crate::reglages) inconnus: Map<String, Value>,
}

impl Film {
    pub fn par_defaut() -> Self {
        Film {
            fluidite: 30,
            son_systeme: false,
            clics_visibles: false,
            dossier: None,
            inconnus: Map::new(),
        }
    }
}

/// Réglages de l'historique (version 7, Pro).
#[derive(Debug, Clone, PartialEq)]
pub struct Historique {
    /// Une valeur de `HISTORIQUE_CHOIX`.
    pub gardes: u32,
    pub(in crate::reglages) inconnus: Map<String, Value>,
}

impl Historique {
    pub fn par_defaut() -> Self {
        Historique {
            gardes: HISTORIQUE_DEFAUT,
            inconnus: Map::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Reglages {
    pub raccourcis: Raccourcis,
    /// Ce que lance Impr. écran quand BreezyShot la tient (version 8). Sans effet tant que
    /// Windows garde la touche pour l'Outil Capture (le registre, pas ce fichier).
    pub impr_ecran_ouvre: CibleImprEcran,
    pub copier_apres_capture: bool,
    pub enregistrer_apres_capture: bool,
    /// Le format du fichier enregistré (version 7) ; « Récents » reste en PNG.
    pub format_image: FormatImage,
    /// Ouvrir l'éditeur après chaque capture (version 7).
    pub editeur_apres_capture: bool,
    /// Jouer le son « Goutte » à chaque capture d'image (version 9).
    pub son_capture: bool,
    pub vignette: Vignette,
    pub selection: Selection,
    pub capture: Capture,
    pub film: Film,
    pub historique: Historique,
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
            impr_ecran_ouvre: CibleImprEcran::Zone,
            copier_apres_capture: true,
            enregistrer_apres_capture: false,
            format_image: FormatImage::Png,
            editeur_apres_capture: false,
            son_capture: true,
            vignette: Vignette::par_defaut(),
            selection: Selection::par_defaut(),
            capture: Capture::par_defaut(),
            film: Film::par_defaut(),
            historique: Historique::par_defaut(),
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

    let mut capture = Map::new();
    capture.insert("retardateur_s".into(), Value::from(r.capture.retardateur_s));
    capture.insert("pointeur".into(), Value::Bool(r.capture.pointeur));
    capture.insert(
        "masquer_icones_bureau".into(),
        Value::Bool(r.capture.masquer_icones_bureau),
    );
    for (cle, valeur) in &r.capture.inconnus {
        capture.insert(cle.clone(), valeur.clone());
    }

    let dossier = match &r.dossier_enregistrement {
        None => Value::Null,
        Some(chemin) => match chemin.to_str() {
            Some(texte) => Value::from(texte),
            None => return Err("dossier_enregistrement"),
        },
    };

    let mut film = Map::new();
    film.insert("fluidite".into(), Value::from(r.film.fluidite));
    film.insert("son_systeme".into(), Value::Bool(r.film.son_systeme));
    film.insert("clics_visibles".into(), Value::Bool(r.film.clics_visibles));
    film.insert(
        "dossier".into(),
        match &r.film.dossier {
            None => Value::Null,
            Some(chemin) => match chemin.to_str() {
                Some(texte) => Value::from(texte),
                None => return Err("film.dossier"),
            },
        },
    );
    for (cle, valeur) in &r.film.inconnus {
        film.insert(cle.clone(), valeur.clone());
    }

    let mut historique = Map::new();
    historique.insert("gardes".into(), Value::from(r.historique.gardes));
    for (cle, valeur) in &r.historique.inconnus {
        historique.insert(cle.clone(), valeur.clone());
    }

    let mut m = Map::new();
    m.insert("version".into(), Value::from(r.version_fichier));
    m.insert("raccourcis".into(), Value::Object(raccourcis));
    m.insert(
        "impr_ecran_ouvre".into(),
        Value::from(r.impr_ecran_ouvre.texte()),
    );
    m.insert(
        "copier_apres_capture".into(),
        Value::Bool(r.copier_apres_capture),
    );
    m.insert(
        "enregistrer_apres_capture".into(),
        Value::Bool(r.enregistrer_apres_capture),
    );
    m.insert("format_image".into(), Value::from(r.format_image.texte()));
    m.insert(
        "editeur_apres_capture".into(),
        Value::Bool(r.editeur_apres_capture),
    );
    m.insert("son_capture".into(), Value::Bool(r.son_capture));
    m.insert("vignette".into(), Value::Object(vignette));
    m.insert("selection".into(), Value::Object(selection));
    m.insert("capture".into(), Value::Object(capture));
    m.insert("film".into(), Value::Object(film));
    m.insert("historique".into(), Value::Object(historique));
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
