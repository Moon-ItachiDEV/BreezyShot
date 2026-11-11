//! Du contenu du fichier aux réglages : fonctions pures, sans disque.

use std::path::PathBuf;

use serde_json::{json, Map, Value};

use super::constat::{Constat, Raison};
use super::modele::{
    vers_json, Animations, Coin, Plateforme, Raccourcis, Reglages, Selection, Theme, Vignette,
    DUREE_MAX_S, DUREE_MIN_S, VERSION_ACTUELLE,
};

/// Une mise à niveau d'une version à la suivante. Elle complète l'objet sans rien effacer ;
/// le numéro de version est tenu par `interpreter`, pas par elle.
pub(super) type MiseANiveau = fn(&mut Map<String, Value>, Plateforme);

/// `MISES_A_NIVEAU[n]` fait passer de la version `n` à `n + 1`. Pour une version 2 un jour :
/// ajouter `v1_vers_v2` à la fin, et `VERSION_ACTUELLE` passe à 2 tout seul.
pub(super) const MISES_A_NIVEAU: &[MiseANiveau] = &[v0_vers_v1];

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
    let demarrer_avec_systeme = lire(
        &mut m,
        "",
        "demarrer_avec_systeme",
        false,
        Value::as_bool,
        c,
    );
    let dossier_enregistrement = lire(&mut m, "", "dossier_enregistrement", None, lire_dossier, c);
    let modele_nom = lire(&mut m, "", "modele_nom", d.modele_nom, texte_non_vide, c);
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

/// Un nombre entier de secondes entre `DUREE_MIN_S` et `DUREE_MAX_S` (`30.0` vaut 30, `6.5`
/// est refusé).
pub(super) fn duree_valide(v: &Value) -> Option<u32> {
    entier_sature(v).filter(|n| (DUREE_MIN_S..=DUREE_MAX_S).contains(n))
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
