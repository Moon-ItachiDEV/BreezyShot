//! Du contenu du fichier aux réglages : fonctions pures, sans disque.

use std::path::PathBuf;

use serde_json::{json, Map, Value};

use super::constat::{Constat, Raison};
use super::modele::{
    vers_json, Animations, Capture, Coin, Plateforme, Raccourcis, Reglages, Selection, Theme,
    Vignette, DUREE_MAX_S, DUREE_MIN_S, DUREE_TOUJOURS, RETARDATEUR_MAX_S, VERSION_ACTUELLE,
};

/// Une mise à niveau d'une version à la suivante. Elle complète l'objet sans rien effacer ;
/// le numéro de version est tenu par `interpreter`, pas par elle.
pub(super) type MiseANiveau = fn(&mut Map<String, Value>, Plateforme);

/// `MISES_A_NIVEAU[n]` fait passer de la version `n` à `n + 1`. Pour une version 6 un jour :
/// ajouter `v5_vers_v6` à la fin, et `VERSION_ACTUELLE` passe à 6 tout seul.
pub(super) const MISES_A_NIVEAU: &[MiseANiveau] =
    &[v0_vers_v1, v1_vers_v2, v2_vers_v3, v3_vers_v4, v4_vers_v5];

/// Version 0 (aucun champ `version`) vers 1 : ajoute les champs de la version 1 qui manquent,
/// y compris dans les sous-objets, sans toucher à ceux qui sont là (même invalides : la
/// validation s'en charge ensuite, champ par champ).
fn v0_vers_v1(objet: &mut Map<String, Value>, plateforme: Plateforme) {
    let r = Raccourcis::par_defaut(plateforme);
    let modele = json!({
        "raccourcis": { "zone": r.zone, "plein_ecran": r.plein_ecran, "fenetre": r.fenetre },
        "copier_apres_capture": true,
        "enregistrer_apres_capture": false,
        "vignette": { "afficher": true, "duree_s": 6, "coin": "bas_gauche" },
        "selection": { "loupe": false },
        "demarrer_avec_systeme": false,
        "dossier_enregistrement": null,
        "modele_nom": "BreezyShot {date} à {heure}",
        "animations": "systeme",
        "theme": "systeme"
    });
    if let Value::Object(modele) = modele {
        completer(objet, &modele);
    }
}

/// Version 1 vers 2 (étape 10) : même forme. Seul le sens de `vignette.duree_s` s'élargit
/// (`0` = toujours affichée). Le numéro change pour qu'une app plus ancienne, qui
/// remplacerait 0 par sa valeur par défaut et réécrirait le fichier, le laisse intact.
fn v1_vers_v2(_objet: &mut Map<String, Value>, _plateforme: Plateforme) {}

/// Version 2 vers 3 (0.4) : le raccourci de la dernière zone (celui de la plateforme) et
/// l'objet `capture` (retardateur, pointeur), sans rien effacer ni remplacer. Un raccourci
/// `derniere_zone` déjà là est gardé ; un objet `raccourcis` absent n'est pas créé (la
/// validation le dira absent, comme en version 2) ; un `capture` qui n'est pas un objet
/// reste tel quel (la validation le remplacera, avec son constat).
fn v2_vers_v3(objet: &mut Map<String, Value>, plateforme: Plateforme) {
    if let Some(Value::Object(raccourcis)) = objet.get_mut("raccourcis") {
        if !raccourcis.contains_key("derniere_zone") {
            let r = Raccourcis::par_defaut(plateforme);
            raccourcis.insert("derniere_zone".into(), json!(r.derniere_zone));
        }
    }
    let modele = json!({ "capture": { "retardateur_s": 0, "pointeur": false } });
    if let Value::Object(modele) = modele {
        completer(objet, &modele);
    }
}

/// Version 3 vers 4 (0.5) : les raccourcis de l'historique et de Copier le texte. Chacun
/// reçoit le défaut de la plateforme seulement si ce raccourci n'est pas déjà donné à une
/// autre action (l'utilisateur a pu taper `Ctrl+Maj+T` en 0.4), sinon une liste vide. Une
/// action déjà là est gardée ; un objet `raccourcis` absent n'est pas créé.
fn v3_vers_v4(objet: &mut Map<String, Value>, plateforme: Plateforme) {
    let Some(Value::Object(raccourcis)) = objet.get_mut("raccourcis") else {
        return;
    };
    let r = Raccourcis::par_defaut(plateforme);
    for (action, defaut) in [("historique", &r.historique), ("texte", &r.texte)] {
        ajouter_action(raccourcis, action, defaut);
    }
}

/// Version 4 vers 5 (0.6) : le raccourci de la capture défilante, avec la même règle que
/// `v3_vers_v4` (défaut seulement s'il n'est pas déjà donné, action déjà là gardée, objet
/// `raccourcis` absent non créé). Écrit aussi sans licence : invisible.
fn v4_vers_v5(objet: &mut Map<String, Value>, plateforme: Plateforme) {
    let Some(Value::Object(raccourcis)) = objet.get_mut("raccourcis") else {
        return;
    };
    let r = Raccourcis::par_defaut(plateforme);
    ajouter_action(raccourcis, "defilante", &r.defilante);
}

/// Ajoute `action` si elle manque : son défaut, ou une liste vide si l'un de ses candidats
/// est déjà donné à une autre action.
fn ajouter_action(raccourcis: &mut Map<String, Value>, action: &str, defaut: &[String]) {
    if raccourcis.contains_key(action) {
        return;
    }
    let pris = defaut.iter().any(|d| deja_donne(raccourcis, d));
    let liste = if pris { Vec::new() } else { defaut.to_vec() };
    raccourcis.insert(action.into(), json!(liste));
}

/// Vrai si une action de `raccourcis` (connue ou non) a `notation` parmi ses candidats.
fn deja_donne(raccourcis: &Map<String, Value>, notation: &str) -> bool {
    let voulu = forme_simple(notation);
    raccourcis.values().any(|liste| {
        liste.as_array().is_some_and(|l| {
            l.iter()
                .any(|c| c.as_str().map(forme_simple) == Some(voulu.clone()))
        })
    })
}

/// Une notation ramenée à une forme comparable sans le module `raccourcis` (qui n'est pas
/// connu d'ici), avec la même règle que son `simplifier` : sans espaces ni points, en
/// minuscules, sans accents (`Impr. écran` vaut `ImprEcran`), `Option` vaut `Alt` et `Cmd`
/// vaut `Win` ; morceaux triés (`Maj + ctrl+T` vaut `Ctrl+Maj+T`).
pub(super) fn forme_simple(notation: &str) -> Vec<String> {
    let mut morceaux: Vec<String> = notation.split('+').map(morceau_simple).collect();
    morceaux.sort();
    morceaux
}

/// Un morceau d'une notation, comme `raccourcis::notation::simplifier` puis les synonymes
/// des modificateurs.
fn morceau_simple(morceau: &str) -> String {
    let minuscules: String = morceau
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '.')
        .flat_map(char::to_lowercase)
        .collect();
    let simple: String = if minuscules.chars().count() <= 1 {
        minuscules
    } else {
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
    };
    match simple.as_str() {
        "option" => "alt".into(),
        "cmd" => "win".into(),
        _ => simple,
    }
}

/// Ajoute à `cible` les clés de `modele` qui lui manquent, en descendant dans les sous-objets
/// présents des deux côtés. Rien n'est effacé ni remplacé.
fn completer(cible: &mut Map<String, Value>, modele: &Map<String, Value>) {
    for (cle, valeur) in modele {
        match cible.get_mut(cle) {
            None => {
                cible.insert(cle.clone(), valeur.clone());
            }
            Some(Value::Object(sous_cible)) => {
                if let Value::Object(sous_modele) = valeur {
                    completer(sous_cible, sous_modele);
                }
            }
            Some(_) => {}
        }
    }
}

/// Au-delà de cette taille, le fichier est traité comme abîmé sans être analysé.
pub(super) const TAILLE_MAX: usize = 1024 * 1024;

// Les très grands nombres des champs inconnus perdent en précision à la réécriture (pas de
// `arbitrary_precision` dans serde_json, qui toucherait tout le programme) : c'est accepté.

/// Lit les octets du fichier : `Ok` si c'est un objet JSON (un BOM UTF-8 en tête est toléré,
/// le Bloc-notes en ajoute parfois), sinon la raison en clair.
pub(super) fn analyser(octets: &[u8]) -> Result<Map<String, Value>, String> {
    if octets.len() > TAILLE_MAX {
        return Err("trop gros".to_string());
    }
    let octets = octets.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(octets);
    match serde_json::from_slice::<Value>(octets) {
        Err(e) => Err(format!("pas du JSON : {e}")),
        Ok(Value::Object(objet)) => Ok(objet),
        Ok(_) => Err("pas un objet JSON".to_string()),
    }
}

/// Résultat de l'interprétation d'un fichier lisible.
#[derive(Debug)]
pub(super) struct Lecture {
    pub reglages: Reglages,
    pub constats: Vec<Constat>,
    /// Le fichier doit être réécrit (mise à niveau ou correction), et peut l'être (version
    /// connue).
    pub a_reecrire: bool,
}

/// Transforme un objet JSON en réglages : version, mises à niveau, validation champ par champ,
/// règle « au moins une sortie active ». Ne panique jamais.
pub(super) fn interpreter(objet: Map<String, Value>, plateforme: Plateforme) -> Lecture {
    let original = Value::Object(objet.clone());
    let mut m = objet;
    let mut constats = Vec::new();

    let mut version = match m.shift_remove("version") {
        None => 0,
        // Un nombre trop grand pour u32 (4294967296, 1e30) vaut u32::MAX : plus récent.
        Some(v) => match entier_sature(&v) {
            Some(n) => n,
            None => {
                constats.push(remplacee("version", Raison::Invalide));
                0
            }
        },
    };
    if version < VERSION_ACTUELLE {
        for mise_a_niveau in MISES_A_NIVEAU.iter().skip(version as usize) {
            mise_a_niveau(&mut m, plateforme);
        }
        constats.push(Constat::MisANiveau {
            de: version,
            a: VERSION_ACTUELLE,
        });
        version = VERSION_ACTUELLE;
    }
    let plus_recente = version > VERSION_ACTUELLE;

    let d = Reglages::par_defaut(plateforme);
    let c = &mut constats;
    let raccourcis = lire_raccourcis(&mut m, d.raccourcis, c);
    let copier_apres_capture = lire(&mut m, "", "copier_apres_capture", true, Value::as_bool, c);
    let enregistrer_apres_capture = lire(
        &mut m,
        "",
        "enregistrer_apres_capture",
        false,
        Value::as_bool,
        c,
    );
    let vignette = lire_vignette(&mut m, c);
    let selection = lire_selection(&mut m, c);
    let capture = lire_capture(&mut m, c);
    let demarrer_avec_systeme = lire(
        &mut m,
        "",
        "demarrer_avec_systeme",
        false,
        Value::as_bool,
        c,
    );
    let dossier_enregistrement = lire(&mut m, "", "dossier_enregistrement", None, lire_dossier, c);
    let modele_nom = lire(&mut m, "", "modele_nom", d.modele_nom, modele_valide, c);
    let animations = lire(
        &mut m,
        "",
        "animations",
        d.animations,
        |v| v.as_str().and_then(Animations::depuis_texte),
        c,
    );
    let theme = lire(
        &mut m,
        "",
        "theme",
        d.theme,
        |v| v.as_str().and_then(Theme::depuis_texte),
        c,
    );

    let mut reglages = Reglages {
        raccourcis,
        copier_apres_capture,
        enregistrer_apres_capture,
        vignette,
        selection,
        capture,
        demarrer_avec_systeme,
        dossier_enregistrement,
        modele_nom,
        animations,
        theme,
        inconnus: m,
        version_fichier: version,
        protege: false,
    };

    if !reglages.une_sortie_active() {
        reglages.copier_apres_capture = true;
        constats.push(Constat::CopieRallumee);
    }
    if plus_recente {
        constats.push(Constat::VersionPlusRecente {
            version,
            connue: VERSION_ACTUELLE,
        });
    }
    // On compare des valeurs JSON (l'ordre des clés et la mise en forme ne comptent pas) :
    // un fichier déjà correct n'est jamais réécrit.
    let a_reecrire = !plus_recente && vers_json(&reglages).ok().as_ref() != Some(&original);
    Lecture {
        reglages,
        constats,
        a_reecrire,
    }
}

fn remplacee(champ: &str, raison: Raison) -> Constat {
    Constat::ValeurRemplacee {
        champ: champ.to_string(),
        raison,
    }
}

/// Retire `cle` de `m` et la convertit ; la valeur par défaut la remplace si elle manque ou
/// est invalide, avec un constat.
fn lire<T>(
    m: &mut Map<String, Value>,
    prefixe: &str,
    cle: &str,
    defaut: T,
    convertir: impl FnOnce(&Value) -> Option<T>,
    constats: &mut Vec<Constat>,
) -> T {
    match m.shift_remove(cle) {
        None => {
            constats.push(remplacee(&format!("{prefixe}{cle}"), Raison::Absente));
            defaut
        }
        Some(v) => match convertir(&v) {
            Some(x) => x,
            None => {
                constats.push(remplacee(&format!("{prefixe}{cle}"), Raison::Invalide));
                defaut
            }
        },
    }
}

fn texte_non_vide(v: &Value) -> Option<String> {
    v.as_str()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
}

/// Modèle de nom : non vide, et pas plus long que `MODELE_NOM_MAX` (même règle que
/// `valider`).
fn modele_valide(v: &Value) -> Option<String> {
    texte_non_vide(v).filter(|s| s.chars().count() <= super::fichier::MODELE_NOM_MAX)
}

/// `null` = dossier par défaut ; un texte vide (ou fait d'espaces) est invalide.
fn lire_dossier(v: &Value) -> Option<Option<PathBuf>> {
    match v {
        Value::Null => Some(None),
        _ => texte_non_vide(v).map(|s| Some(PathBuf::from(s))),
    }
}

/// Une liste de textes (vide permise : l'action n'a alors aucun raccourci).
fn liste_de_textes(v: &Value) -> Option<Vec<String>> {
    v.as_array()?
        .iter()
        .map(|e| e.as_str().map(str::to_string))
        .collect()
}

fn lire_raccourcis(
    m: &mut Map<String, Value>,
    defaut: Raccourcis,
    constats: &mut Vec<Constat>,
) -> Raccourcis {
    let mut objet = match m.shift_remove("raccourcis") {
        None => {
            constats.push(remplacee("raccourcis", Raison::Absente));
            return defaut;
        }
        Some(Value::Object(objet)) => objet,
        Some(_) => {
            constats.push(remplacee("raccourcis", Raison::Invalide));
            return defaut;
        }
    };
    let mut r = defaut;
    for action in Raccourcis::ACTIONS {
        if let Some(cible) = r.candidats_mut(action) {
            let defaut_action = std::mem::take(cible);
            *cible = lire(
                &mut objet,
                "raccourcis.",
                action,
                defaut_action,
                liste_de_textes,
                constats,
            );
        }
    }
    // Les actions inconnues restent telles quelles, sans jugement : elles ne sont pas à nous.
    r.autres = objet;
    r
}

/// Retire le sous-objet `cle` ; `None` (avec un constat) s'il manque ou n'est pas un objet.
fn sous_objet(
    m: &mut Map<String, Value>,
    cle: &str,
    constats: &mut Vec<Constat>,
) -> Option<Map<String, Value>> {
    match m.shift_remove(cle) {
        None => {
            constats.push(remplacee(cle, Raison::Absente));
            None
        }
        Some(Value::Object(objet)) => Some(objet),
        Some(_) => {
            constats.push(remplacee(cle, Raison::Invalide));
            None
        }
    }
}

fn lire_selection(m: &mut Map<String, Value>, constats: &mut Vec<Constat>) -> Selection {
    let d = Selection::par_defaut();
    let Some(mut objet) = sous_objet(m, "selection", constats) else {
        return d;
    };
    let loupe = lire(
        &mut objet,
        "selection.",
        "loupe",
        d.loupe,
        Value::as_bool,
        constats,
    );
    Selection {
        loupe,
        inconnus: objet,
    }
}

fn lire_capture(m: &mut Map<String, Value>, constats: &mut Vec<Constat>) -> Capture {
    let d = Capture::par_defaut();
    let Some(mut objet) = sous_objet(m, "capture", constats) else {
        return d;
    };
    let p = "capture.";
    let retardateur_s = lire(
        &mut objet,
        p,
        "retardateur_s",
        d.retardateur_s,
        retardateur_valide,
        constats,
    );
    let pointeur = lire(
        &mut objet,
        p,
        "pointeur",
        d.pointeur,
        Value::as_bool,
        constats,
    );
    Capture {
        retardateur_s,
        pointeur,
        inconnus: objet,
    }
}

/// Un nombre entier de secondes de 0 à `RETARDATEUR_MAX_S` (`3.0` vaut 3, `"3"` est refusé).
pub(super) fn retardateur_valide(v: &Value) -> Option<u32> {
    entier_sature(v).filter(|n| *n <= RETARDATEUR_MAX_S)
}

fn lire_vignette(m: &mut Map<String, Value>, constats: &mut Vec<Constat>) -> Vignette {
    let d = Vignette::par_defaut();
    let Some(mut objet) = sous_objet(m, "vignette", constats) else {
        return d;
    };
    let p = "vignette.";
    let afficher = lire(
        &mut objet,
        p,
        "afficher",
        d.afficher,
        Value::as_bool,
        constats,
    );
    let duree_s = lire(&mut objet, p, "duree_s", d.duree_s, duree_valide, constats);
    let coin = lire(
        &mut objet,
        p,
        "coin",
        d.coin,
        |v| v.as_str().and_then(Coin::depuis_texte),
        constats,
    );
    Vignette {
        afficher,
        duree_s,
        coin,
        inconnus: objet,
    }
}

/// Un nombre entier de secondes entre `DUREE_MIN_S` et `DUREE_MAX_S`, ou `DUREE_TOUJOURS`
/// (`30.0` vaut 30, `6.5` est refusé).
pub(super) fn duree_valide(v: &Value) -> Option<u32> {
    entier_sature(v).filter(|n| *n == DUREE_TOUJOURS || (DUREE_MIN_S..=DUREE_MAX_S).contains(n))
}

/// Un entier positif ou nul, écrit `6` ou `6.0`, ramené à `u32::MAX` s'il ne tient pas.
/// `None` pour un texte, un négatif ou un décimal avec partie fractionnaire.
fn entier_sature(v: &Value) -> Option<u32> {
    if let Some(n) = v.as_u64() {
        return Some(u32::try_from(n).unwrap_or(u32::MAX));
    }
    let f = v.as_f64()?;
    if !f.is_finite() || f < 0.0 || f.fract() != 0.0 {
        return None;
    }
    if f >= f64::from(u32::MAX) {
        Some(u32::MAX)
    } else {
        // Exact : f est entier, positif et plus petit que u32::MAX.
        Some(f as u32)
    }
}
