//! Tests du module : une règle du contrat (ARCHITECTURE.md §4) par test au moins.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Local, TimeZone};
use serde_json::{json, Value};
use tempfile::TempDir;

use super::fichier::{
    charger_a, cle_sauvegarde, nom_sauvegarde, sauvegardes_a_supprimer, texte_du_fichier,
};
use super::*;

const W: Plateforme = Plateforme::Windows;

fn dossier() -> TempDir {
    match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => panic!("dossier temporaire : {e}"),
    }
}

fn chemin(d: &TempDir) -> PathBuf {
    d.path().join(NOM_FICHIER)
}

fn poser(d: &TempDir, octets: impl AsRef<[u8]>) {
    fs::write(chemin(d), octets).unwrap();
}

fn poser_json(d: &TempDir, v: Value) {
    poser(d, serde_json::to_string_pretty(&v).unwrap());
}

fn octets(d: &TempDir) -> Vec<u8> {
    fs::read(chemin(d)).unwrap()
}

fn json_du_fichier(d: &TempDir) -> Value {
    serde_json::from_slice(&octets(d)).unwrap()
}

fn noms(d: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(d)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    v.sort();
    v
}

/// Le fichier par défaut complet, en JSON, modifié par `f`.
fn defaut_json(f: impl FnOnce(&mut serde_json::Map<String, Value>)) -> Value {
    let texte = texte_du_fichier(&Reglages::par_defaut(W)).unwrap();
    let mut v: Value = serde_json::from_str(&texte).unwrap();
    if let Value::Object(m) = &mut v {
        f(m);
    }
    v
}

fn remplacee(champ: &str, raison: Raison) -> Constat {
    Constat::ValeurRemplacee {
        champ: champ.to_string(),
        raison,
    }
}

/// Un second chargement ne doit rien réécrire ni rien signaler.
fn assert_stable(d: &TempDir) {
    let avant = octets(d);
    let (_, constats) = charger(d.path(), W);
    assert_eq!(constats, vec![], "second chargement");
    assert_eq!(octets(d), avant, "fichier réécrit au second chargement");
}

// ---------- Valeurs par défaut ----------

#[test]
fn par_defaut_windows_suit_le_contrat() {
    let texte = texte_du_fichier(&Reglages::par_defaut(W)).unwrap();
    let attendu = json!({
        "version": 2,
        "raccourcis": {
            "zone": ["ImprEcran", "Ctrl+Maj+1"],
            "plein_ecran": ["Ctrl+Maj+2"],
            "fenetre": ["Ctrl+Maj+3"]
        },
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
    // Même texte, même ordre des clés que l'exemple du contrat.
    assert_eq!(
        texte,
        format!("{}\n", serde_json::to_string_pretty(&attendu).unwrap())
    );
}

#[test]
fn par_defaut_mac_sans_impr_ecran() {
    let r = Reglages::par_defaut(Plateforme::Mac);
    for action in Raccourcis::ACTIONS {
        let c = r.raccourcis.candidats(action).unwrap();
        assert!(!c.is_empty());
        assert!(c
            .iter()
            .all(|s| !s.contains("ImprEcran") && !s.contains("Ctrl")));
    }
    assert_eq!(
        Reglages::par_defaut(Plateforme::Autre),
        Reglages::par_defaut(W)
    );
    assert_eq!(r.raccourcis.candidats("inconnue"), None);
}

#[test]
fn textes_des_enumerations_aller_retour() {
    for c in [
        Coin::BasGauche,
        Coin::BasDroite,
        Coin::HautGauche,
        Coin::HautDroite,
    ] {
        assert_eq!(Coin::depuis_texte(c.texte()), Some(c));
    }
    for a in [
        Animations::Systeme,
        Animations::Normales,
        Animations::Reduites,
    ] {
        assert_eq!(Animations::depuis_texte(a.texte()), Some(a));
    }
    for t in [Theme::Systeme, Theme::Clair, Theme::Sombre] {
        assert_eq!(Theme::depuis_texte(t.texte()), Some(t));
    }
    assert_eq!(Theme::depuis_texte("Sombre"), None);
}

// ---------- Fichier absent ----------

#[test]
fn absent_cree_par_defaut_puis_stable() {
    let d = dossier();
    let (r, constats) = charger(d.path(), W);
    assert_eq!(r, Reglages::par_defaut(W));
    assert_eq!(constats, vec![Constat::Cree { chemin: chemin(&d) }]);
    let texte = String::from_utf8(octets(&d)).unwrap();
    assert!(texte.ends_with("}\n"));
    assert!(texte.contains("\n  \"version\": 2,"), "indenté");
    assert!(
        texte.contains("BreezyShot {date} à {heure}"),
        "UTF-8 brut, pas \\u00e0"
    );
    assert_eq!(
        noms(d.path()),
        vec![NOM_FICHIER.to_string()],
        "pas de .tmp laissé"
    );
    assert_stable(&d);
}

#[test]
fn absent_dans_un_dossier_qui_n_existe_pas() {
    let d = dossier();
    let sous = d.path().join("a").join("b");
    let (_, constats) = charger(&sous, W);
    assert!(matches!(constats.as_slice(), [Constat::Cree { .. }]));
    assert!(sous.join(NOM_FICHIER).is_file());
}

#[test]
fn dossier_impossible_a_creer_rend_les_defauts() {
    let d = dossier();
    let fichier = d.path().join("pas_un_dossier");
    fs::write(&fichier, "x").unwrap();
    let (r, constats) = charger(&fichier.join("sous"), W);
    assert_eq!(r, Reglages::par_defaut(W));
    assert!(matches!(
        constats.as_slice(),
        [Constat::EcritureImpossible { .. }]
    ));
}

#[test]
fn lecture_impossible_ne_touche_a_rien() {
    let d = dossier();
    fs::create_dir(chemin(&d)).unwrap(); // un dossier à la place du fichier
    let (r, constats) = charger(d.path(), W);
    assert_eq!(r.theme, Reglages::par_defaut(W).theme);
    assert!(matches!(
        constats.as_slice(),
        [Constat::LectureImpossible { .. }]
    ));
    assert!(chemin(&d).is_dir());
    // Réglages protégés : `ecrire` ne remplacera pas un fichier qu'on n'a pas pu lire.
    assert!(!r.enregistrable());
    assert!(matches!(
        ecrire(d.path(), &r),
        Err(ErreurEcriture::FichierProtege)
    ));
    assert!(chemin(&d).is_dir());
}

#[test]
fn enregistrable_par_defaut_et_apres_lecture_normale() {
    let d = dossier();
    let (r, _) = charger(d.path(), W);
    assert!(r.enregistrable());
    assert!(Reglages::par_defaut(Plateforme::Mac).enregistrable());
}

// ---------- Fichier illisible ----------

fn assert_abime(contenu: &[u8]) {
    let d = dossier();
    poser(&d, contenu);
    let (r, constats) = charger(d.path(), W);
    assert_eq!(r, Reglages::par_defaut(W));
    let sauvegarde = match constats.as_slice() {
        [Constat::Abime { sauvegarde, .. }] => sauvegarde.clone(),
        autre => panic!("constats inattendus pour {contenu:?} : {autre:?}"),
    };
    let nom = sauvegarde
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        nom.starts_with("reglages.abime-") && nom.ends_with(".json"),
        "{nom}"
    );
    assert_eq!(
        fs::read(&sauvegarde).unwrap(),
        contenu,
        "contenu gardé tel quel"
    );
    assert_eq!(
        String::from_utf8(octets(&d)).unwrap(),
        texte_du_fichier(&Reglages::par_defaut(W)).unwrap()
    );
    assert_stable(&d);
}

#[test]
fn illisible_pas_du_json() {
    assert_abime(b"{ \"version\": 1, ");
}

#[test]
fn illisible_fichier_vide() {
    assert_abime(b"");
}

#[test]
fn illisible_null() {
    assert_abime(b"null");
}

#[test]
fn illisible_tableau() {
    assert_abime(b"[{\"version\": 1}]");
}

#[test]
fn illisible_nombre_ou_texte() {
    assert_abime(b"42");
    assert_abime(b"\"reglages\"");
}

#[test]
fn illisible_pas_de_l_utf8() {
    assert_abime(b"{\"modele_nom\": \"\xff\xfe\"}");
}

#[test]
fn bom_utf8_accepte_sans_reecriture() {
    let d = dossier();
    let mut contenu = b"\xEF\xBB\xBF".to_vec();
    contenu.extend(
        texte_du_fichier(&Reglages::par_defaut(W))
            .unwrap()
            .as_bytes(),
    );
    poser(&d, &contenu);
    let (r, constats) = charger(d.path(), W);
    assert_eq!(r, Reglages::par_defaut(W));
    assert_eq!(constats, vec![]);
    assert_eq!(octets(&d), contenu, "BOM laissé, fichier non réécrit");
}

#[test]
fn trois_fichiers_abimes_gardes_au_plus() {
    let d = dossier();
    let mut sauvegardes = Vec::new();
    for i in 0..5u32 {
        poser(&d, format!("abime {i}"));
        let quand = Local.with_ymd_and_hms(2026, 11, 11, 18, 40, i).unwrap();
        let (_, constats) = charger_a(d.path(), W, quand);
        for c in &constats {
            if let Constat::Abime { sauvegarde, .. } = c {
                sauvegardes.push(sauvegarde.clone());
            }
        }
        let supprimees = constats
            .iter()
            .filter(|c| matches!(c, Constat::AncienneSauvegardeSupprimee { .. }))
            .count();
        assert_eq!(supprimees, usize::from(i >= 3), "passage {i}");
    }
    let restants: Vec<String> = noms(d.path())
        .into_iter()
        .filter(|n| n.starts_with("reglages.abime-"))
        .collect();
    assert_eq!(restants.len(), 3);
    // Ce sont les trois plus récents.
    for (n, s) in sauvegardes.iter().enumerate() {
        assert_eq!(s.exists(), n >= 2, "{}", s.display());
    }
    assert_eq!(fs::read_to_string(&sauvegardes[4]).unwrap(), "abime 4");
}

#[test]
fn deux_abimes_au_meme_instant_ne_s_ecrasent_pas() {
    let d = dossier();
    let quand = Local.with_ymd_and_hms(2026, 11, 11, 18, 40, 0).unwrap();
    poser(&d, "un");
    charger_a(d.path(), W, quand);
    poser(&d, "deux");
    charger_a(d.path(), W, quand);
    let a = d.path().join(nom_sauvegarde(quand, 1));
    let b = d.path().join(nom_sauvegarde(quand, 2));
    assert_eq!(fs::read_to_string(a).unwrap(), "un");
    assert_eq!(fs::read_to_string(b).unwrap(), "deux");
}

#[test]
fn noms_de_sauvegarde_se_trient_dans_l_ordre_du_temps() {
    let t1 = Local.with_ymd_and_hms(2026, 11, 11, 9, 5, 0).unwrap();
    let t2 = Local.with_ymd_and_hms(2026, 11, 11, 18, 40, 12).unwrap();
    assert_eq!(
        nom_sauvegarde(t2, 1),
        "reglages.abime-20261111-184012-000.json"
    );
    let mut v = vec![
        nom_sauvegarde(t2, 2),
        nom_sauvegarde(t2, 1),
        nom_sauvegarde(t1, 1),
    ];
    v.sort();
    assert_eq!(
        v,
        vec![
            nom_sauvegarde(t1, 1),
            nom_sauvegarde(t2, 1),
            nom_sauvegarde(t2, 2)
        ]
    );
}

#[test]
fn choix_des_sauvegardes_a_supprimer() {
    let n = |s: &str| s.to_string();
    let noms = vec![
        n("reglages.json"),
        n("reglages.abime-20261111-100000-000.json"),
        n("reglages.abime-20261109-100000-000.json"),
        n("autre.json"),
        n("reglages.abime-20261111-100000-000_2.json"),
        n("reglages.abime-20261110-100000-000.json"),
        n("reglages.abime-20261108-100000-000.txt"),
    ];
    // La neuve (la plus récente ici) + les 2 plus récentes des autres.
    assert_eq!(
        sauvegardes_a_supprimer(noms.clone(), "reglages.abime-20261111-100000-000_2.json"),
        vec![n("reglages.abime-20261109-100000-000.json")]
    );
    // Horloge qui a reculé : la neuve est la plus ancienne par son nom, elle reste quand même.
    assert_eq!(
        sauvegardes_a_supprimer(noms, "reglages.abime-20261109-100000-000.json"),
        vec![n("reglages.abime-20261110-100000-000.json")]
    );
    assert_eq!(
        sauvegardes_a_supprimer(vec![n("reglages.json")], "x"),
        Vec::<String>::new()
    );
}

#[test]
fn format_exact_des_noms_de_sauvegarde() {
    assert_eq!(
        cle_sauvegarde("reglages.abime-20261111-184012-345.json"),
        Some(("20261111-184012-345", 1))
    );
    assert_eq!(
        cle_sauvegarde("reglages.abime-20261111-184012-345_10.json"),
        Some(("20261111-184012-345", 10))
    );
    for nom in [
        "reglages.abime-.json",
        "reglages.abime-0-ma-copie-perso.json",
        "reglages.abime-20261111-184012-345 - Copie.json",
        "reglages.abime-20261111-184012-345_.json",
        "reglages.abime-20261111-184012-345_2a.json",
        "reglages.abime-20261111-184012-345_-2.json",
        "reglages.abime-2026111a-184012-345.json",
        "reglages.abime-20261111_184012-345.json",
        "reglages.abime-20261111-184012-3456.json",
        "reglages.abime-20261111-184012-345.json.txt",
    ] {
        assert_eq!(cle_sauvegarde(nom), None, "{nom}");
    }
    // _10 se trie après _2 (nombres, pas texte).
    let base = "reglages.abime-20261111-100000-000";
    let noms: Vec<String> = ["", "_2", "_10", "_3"]
        .iter()
        .map(|s| format!("{base}{s}.json"))
        .collect();
    assert_eq!(
        sauvegardes_a_supprimer(noms, &format!("{base}_10.json")),
        vec![format!("{base}.json")]
    );
}

#[test]
fn horloge_qui_recule_garde_la_sauvegarde_neuve() {
    let d = dossier();
    for s in 0..3 {
        poser(&d, format!("ancien {s}"));
        charger_a(
            d.path(),
            W,
            Local.with_ymd_and_hms(2030, 1, 1, 0, 0, s).unwrap(),
        );
    }
    poser(&d, "PRECIEUX contenu utilisateur");
    let quand = Local.with_ymd_and_hms(2026, 11, 11, 18, 0, 0).unwrap();
    let (_, c) = charger_a(d.path(), W, quand);
    let neuve = d.path().join(nom_sauvegarde(quand, 1));
    assert_eq!(
        fs::read_to_string(&neuve).unwrap(),
        "PRECIEUX contenu utilisateur"
    );
    let supprimee = d.path().join(nom_sauvegarde(
        Local.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap(),
        1,
    ));
    assert!(c.contains(&Constat::AncienneSauvegardeSupprimee {
        chemin: supprimee.clone()
    }));
    assert!(!supprimee.exists());
    let restants = noms(d.path())
        .into_iter()
        .filter(|n| n.starts_with("reglages.abime-"))
        .count();
    assert_eq!(restants, 3);
}

#[test]
fn nettoyage_ignore_les_noms_proches() {
    let d = dossier();
    let a_moi = [
        "reglages.abime-0-ma-copie-perso.json",
        "reglages.abime-.json",
        "reglages.abime-20200101-000000-000 - Copie.json",
    ];
    for nom in a_moi {
        fs::write(d.path().join(nom), "a moi").unwrap();
    }
    for s in 0..4 {
        poser(&d, format!("abime {s}"));
        charger_a(
            d.path(),
            W,
            Local.with_ymd_and_hms(2026, 11, 11, 18, 0, s).unwrap(),
        );
    }
    let restants = noms(d.path());
    for nom in a_moi {
        assert!(restants.contains(&nom.to_string()), "{nom} supprime");
    }
    assert_eq!(restants.len(), a_moi.len() + 3 + 1);
}

#[test]
fn abime_non_sauvegarde_est_protege() {
    let d = dossier();
    let quand = Local.with_ymd_and_hms(2026, 11, 11, 18, 0, 0).unwrap();
    for n in 1..=99 {
        fs::write(d.path().join(nom_sauvegarde(quand, n)), "occupe").unwrap();
    }
    poser(&d, "{ \"theme\": \"sombre\", PRECIEUX abime");
    let avant = octets(&d);
    let (mut r, c) = charger_a(d.path(), W, quand);
    assert!(matches!(c.as_slice(), [Constat::AbimeNonSauvegarde { .. }]));
    assert!(!r.enregistrable());
    r.theme = Theme::Clair; // l'utilisateur change un réglage dans l'interface
    assert!(matches!(
        ecrire(d.path(), &r),
        Err(ErreurEcriture::FichierProtege)
    ));
    assert_eq!(octets(&d), avant, "contenu abime intact");
}

#[test]
fn abime_en_lecture_seule_ni_renomme_ni_remplace() {
    let d = dossier();
    poser(&d, "pas du json");
    let mut droits = fs::metadata(chemin(&d)).unwrap().permissions();
    droits.set_readonly(true);
    fs::set_permissions(chemin(&d), droits).unwrap();
    let (r, c) = charger(d.path(), W);
    assert!(matches!(c.as_slice(), [Constat::AbimeNonSauvegarde { .. }]));
    assert!(!r.enregistrable());
    assert_eq!(noms(d.path()), vec![NOM_FICHIER.to_string()]);
    assert_eq!(octets(&d), b"pas du json");
}

#[test]
fn fichier_trop_gros_mis_de_cote() {
    let d = dossier();
    let gros = defaut_json(|m| {
        m.insert("remplissage".into(), json!("x".repeat(1024 * 1024)));
    });
    let contenu = serde_json::to_string(&gros).unwrap();
    assert!(contenu.len() > 1024 * 1024);
    poser(&d, &contenu);
    let (r, c) = charger(d.path(), W);
    let sauvegarde = match c.as_slice() {
        [Constat::Abime { raison, sauvegarde }] => {
            assert_eq!(raison, "trop gros");
            sauvegarde.clone()
        }
        autre => panic!("{autre:?}"),
    };
    assert_eq!(fs::read_to_string(sauvegarde).unwrap(), contenu);
    assert_eq!(r, Reglages::par_defaut(W));
    assert_stable(&d);
}

#[test]
fn fichier_juste_sous_la_limite_lu() {
    let d = dossier();
    let mut gros = defaut_json(|m| {
        m.insert("remplissage".into(), json!(""));
    });
    let base = serde_json::to_string(&gros).unwrap().len();
    gros["remplissage"] = json!("x".repeat(1024 * 1024 - base));
    let contenu = serde_json::to_string(&gros).unwrap();
    assert_eq!(contenu.len(), 1024 * 1024);
    poser(&d, &contenu);
    let (_, c) = charger(d.path(), W);
    assert_eq!(c, vec![]);
}

// ---------- Valeurs invalides ----------

/// Pose `v`, charge, et renvoie réglages et constats ; vérifie aussi que le fichier réécrit
/// est stable.
fn charger_json(v: Value) -> (TempDir, Reglages, Vec<Constat>) {
    let d = dossier();
    poser_json(&d, v);
    let (r, c) = charger(d.path(), W);
    (d, r, c)
}

#[test]
fn valeur_inconnue_remplacee_seule() {
    let (d, r, c) = charger_json(defaut_json(|m| {
        m.insert("theme".into(), json!("violet"));
        m.insert("animations".into(), json!("reduites"));
        m.insert("enregistrer_apres_capture".into(), json!(true));
    }));
    assert_eq!(c, vec![remplacee("theme", Raison::Invalide)]);
    assert_eq!(r.theme, Theme::Systeme);
    assert_eq!(
        r.animations,
        Animations::Reduites,
        "les autres valeurs restent"
    );
    assert!(r.enregistrer_apres_capture);
    let f = json_du_fichier(&d);
    assert_eq!(f["theme"], json!("systeme"));
    assert_eq!(f["animations"], json!("reduites"));
    assert_stable(&d);
}

#[test]
fn mauvais_type_remplace() {
    let (d, r, c) = charger_json(defaut_json(|m| {
        m.insert("copier_apres_capture".into(), json!("oui"));
        m.insert("demarrer_avec_systeme".into(), json!(1));
    }));
    assert_eq!(
        c,
        vec![
            remplacee("copier_apres_capture", Raison::Invalide),
            remplacee("demarrer_avec_systeme", Raison::Invalide),
        ]
    );
    assert!(r.copier_apres_capture);
    assert!(!r.demarrer_avec_systeme);
    assert_stable(&d);
}

#[test]
fn champ_absent_dans_un_fichier_a_jour() {
    let (d, r, c) = charger_json(defaut_json(|m| {
        m.remove("modele_nom");
    }));
    assert_eq!(c, vec![remplacee("modele_nom", Raison::Absente)]);
    assert_eq!(r.modele_nom, "BreezyShot {date} à {heure}");
    assert_eq!(
        json_du_fichier(&d)["modele_nom"],
        json!("BreezyShot {date} à {heure}")
    );
    assert_stable(&d);
}

#[test]
fn duree_hors_limites_ou_pas_entiere() {
    for mauvaise in [
        json!(61),
        json!(1000),
        json!(6.5),
        json!(60.5),
        json!(1e30),
        json!(-1),
        json!("6"),
        json!(null),
    ] {
        let (d, r, c) = charger_json(defaut_json(|m| {
            m["vignette"]["duree_s"] = mauvaise.clone();
            m["vignette"]["coin"] = json!("haut_droite");
        }));
        assert_eq!(
            c,
            vec![remplacee("vignette.duree_s", Raison::Invalide)],
            "{mauvaise}"
        );
        assert_eq!(r.vignette.duree_s, 6);
        assert_eq!(r.vignette.coin, Coin::HautDroite);
        assert_stable(&d);
    }
    for bonne in [0u32, 1, 60] {
        let (_, r, c) = charger_json(defaut_json(|m| m["vignette"]["duree_s"] = json!(bonne)));
        assert_eq!(c, vec![]);
        assert_eq!(r.vignette.duree_s, bonne);
    }
}

#[test]
fn vignette_coin_invalide_et_champ_inconnu_garde() {
    let (d, r, c) = charger_json(defaut_json(|m| {
        m["vignette"]["coin"] = json!("milieu");
        m["vignette"]["opacite"] = json!({ "valeur": 0.8 });
    }));
    assert_eq!(c, vec![remplacee("vignette.coin", Raison::Invalide)]);
    assert_eq!(r.vignette.coin, Coin::BasGauche);
    let f = json_du_fichier(&d);
    assert_eq!(f["vignette"]["opacite"], json!({ "valeur": 0.8 }));
    assert_eq!(f["vignette"]["coin"], json!("bas_gauche"));
    assert_stable(&d);
}

#[test]
fn vignette_pas_un_objet() {
    let (d, r, c) = charger_json(defaut_json(|m| {
        m.insert("vignette".into(), json!(true));
    }));
    assert_eq!(c, vec![remplacee("vignette", Raison::Invalide)]);
    assert_eq!(r.vignette, Vignette::par_defaut());
    assert_stable(&d);
}

#[test]
fn dossier_enregistrement_vide_ou_rempli() {
    for vide in ["", "   "] {
        let (d, r, c) = charger_json(defaut_json(|m| {
            m.insert("dossier_enregistrement".into(), json!(vide));
        }));
        assert_eq!(
            c,
            vec![remplacee("dossier_enregistrement", Raison::Invalide)]
        );
        assert_eq!(r.dossier_enregistrement, None);
        assert_eq!(json_du_fichier(&d)["dossier_enregistrement"], Value::Null);
    }
    let (d, r, c) = charger_json(defaut_json(|m| {
        m.insert("dossier_enregistrement".into(), json!("D:\\Captures"));
    }));
    assert_eq!(c, vec![]);
    assert_eq!(
        r.dossier_enregistrement,
        Some(PathBuf::from("D:\\Captures"))
    );
    assert_stable(&d);
}

#[test]
fn modele_nom_vide_remplace() {
    let (_, r, c) = charger_json(defaut_json(|m| {
        m.insert("modele_nom".into(), json!(""));
    }));
    assert_eq!(c, vec![remplacee("modele_nom", Raison::Invalide)]);
    assert_eq!(r.modele_nom, "BreezyShot {date} à {heure}");
}

// ---------- Sélection ----------

#[test]
fn selection_loupe_lue_et_champ_inconnu_garde() {
    let (d, r, c) = charger_json(defaut_json(|m| {
        m["selection"]["loupe"] = json!(true);
        m["selection"]["grille"] = json!({ "pas": 8 });
        m["theme"] = json!(0); // force une réécriture
    }));
    assert_eq!(c, vec![remplacee("theme", Raison::Invalide)]);
    assert!(r.selection.loupe);
    let texte = String::from_utf8(octets(&d)).unwrap();
    let f: Value = serde_json::from_str(&texte).unwrap();
    assert_eq!(
        f["selection"],
        json!({ "loupe": true, "grille": { "pas": 8 } })
    );
    let pos = |s: &str| texte.find(s).unwrap();
    assert!(pos("\"vignette\"") < pos("\"selection\""));
    assert!(pos("\"selection\"") < pos("\"demarrer_avec_systeme\""));
    assert_stable(&d);
}

#[test]
fn selection_absente_en_v1() {
    let (d, r, c) = charger_json(defaut_json(|m| {
        m.shift_remove("selection");
    }));
    assert_eq!(c, vec![remplacee("selection", Raison::Absente)]);
    assert_eq!(r.selection, Selection::par_defaut());
    assert_eq!(json_du_fichier(&d)["selection"], json!({ "loupe": false }));
    assert_stable(&d);

    let (d, r, c) = charger_json(defaut_json(|m| m["selection"] = json!({ "autre": 1 })));
    assert_eq!(c, vec![remplacee("selection.loupe", Raison::Absente)]);
    assert!(!r.selection.loupe);
    assert_eq!(
        json_du_fichier(&d)["selection"],
        json!({ "loupe": false, "autre": 1 })
    );
    assert_stable(&d);
}

#[test]
fn selection_invalide() {
    for mauvaise in [json!("oui"), json!(1), json!(null)] {
        let (d, r, c) = charger_json(defaut_json(|m| m["selection"]["loupe"] = mauvaise.clone()));
        assert_eq!(
            c,
            vec![remplacee("selection.loupe", Raison::Invalide)],
            "{mauvaise}"
        );
        assert!(!r.selection.loupe);
        assert_stable(&d);
    }
    for mauvaise in [json!(true), json!([]), json!("loupe")] {
        let (d, r, c) = charger_json(defaut_json(|m| m["selection"] = mauvaise.clone()));
        assert_eq!(
            c,
            vec![remplacee("selection", Raison::Invalide)],
            "{mauvaise}"
        );
        assert_eq!(r.selection, Selection::par_defaut());
        assert_stable(&d);
    }
}

#[test]
fn selection_completee_par_la_mise_a_niveau() {
    let (d, r, c) = charger_json(json!({ "selection": { "zoom": 2 } }));
    assert_eq!(c, vec![Constat::MisANiveau { de: 0, a: 2 }]);
    assert!(!r.selection.loupe);
    assert_eq!(
        json_du_fichier(&d)["selection"],
        json!({ "zoom": 2, "loupe": false })
    );
    assert_stable(&d);
    let (_, r, c) = charger_json(json!({ "selection": { "loupe": true } }));
    assert_eq!(c, vec![Constat::MisANiveau { de: 0, a: 2 }]);
    assert!(r.selection.loupe);
}

#[test]
fn duree_decimale_entiere_acceptee() {
    let (d, r, c) = charger_json(defaut_json(|m| m["vignette"]["duree_s"] = json!(30.0)));
    assert_eq!(c, vec![]);
    assert_eq!(r.vignette.duree_s, 30);
    // Réécrite une fois sous la forme entière, puis stable.
    assert_eq!(json_du_fichier(&d)["vignette"]["duree_s"], json!(30));
    assert_stable(&d);
}

#[test]
fn ecrire_refuse_un_dossier_fait_d_espaces() {
    let d = dossier();
    let mut r = Reglages::par_defaut(W);
    r.dossier_enregistrement = Some(PathBuf::from("   "));
    assert!(matches!(
        ecrire(d.path(), &r),
        Err(ErreurEcriture::ValeurInvalide {
            champ: "dossier_enregistrement"
        })
    ));
    assert!(!chemin(&d).exists());
}

#[test]
fn ecrire_refuse_un_texte_trop_gros() {
    let d = dossier();
    charger(d.path(), W);
    let avant = octets(&d);
    let mut r = Reglages::par_defaut(W);
    // Le modèle de nom est borné (`valider`) : le texte trop gros vient d'un champ inconnu.
    r.modele_nom = "x".repeat(MODELE_NOM_MAX + 1);
    assert!(matches!(
        ecrire(d.path(), &r),
        Err(ErreurEcriture::ValeurInvalide {
            champ: "modele_nom"
        })
    ));
    r.modele_nom = "x".repeat(MODELE_NOM_MAX);
    r.inconnus.insert(
        "gros".into(),
        serde_json::Value::String("x".repeat(1_100_000)),
    );
    assert!(matches!(
        ecrire(d.path(), &r),
        Err(ErreurEcriture::TropGros { .. })
    ));
    assert_eq!(octets(&d), avant);
    assert_eq!(noms(d.path()), vec![NOM_FICHIER.to_string()]);
}

#[test]
fn ecrire_refuse_de_remplacer_un_fichier_illisible() {
    let d = dossier();
    fs::create_dir(chemin(&d)).unwrap(); // présent mais illisible comme fichier
    fs::write(chemin(&d).join("dedans"), "a moi").unwrap();
    assert!(matches!(
        ecrire(d.path(), &Reglages::par_defaut(W)),
        Err(ErreurEcriture::FichierProtege)
    ));
    assert!(chemin(&d).is_dir());
    assert_eq!(
        fs::read_to_string(chemin(&d).join("dedans")).unwrap(),
        "a moi"
    );
    assert_eq!(noms(d.path()), vec![NOM_FICHIER.to_string()], "pas de .tmp");
}

#[test]
fn ecrire_remplace_un_fichier_lisible_mais_abime() {
    let d = dossier();
    poser(&d, "pas du json");
    ecrire(d.path(), &Reglages::par_defaut(W)).unwrap();
    assert_eq!(
        String::from_utf8(octets(&d)).unwrap(),
        texte_du_fichier(&Reglages::par_defaut(W)).unwrap()
    );
}

// ---------- Raccourcis ----------

#[test]
fn raccourcis_liste_vide_acceptee() {
    let (d, r, c) = charger_json(defaut_json(|m| m["raccourcis"]["fenetre"] = json!([])));
    assert_eq!(c, vec![]);
    assert!(r.raccourcis.fenetre.is_empty());
    assert_stable(&d);
}

#[test]
fn raccourcis_elements_non_textuels() {
    let (d, r, c) = charger_json(defaut_json(|m| {
        m["raccourcis"]["zone"] = json!(["Ctrl+Maj+9", 4, null]);
        m["raccourcis"]["plein_ecran"] = json!("Ctrl+Maj+2");
        m["raccourcis"]["fenetre"] = json!(["Alt+F"]);
    }));
    assert_eq!(
        c,
        vec![
            remplacee("raccourcis.zone", Raison::Invalide),
            remplacee("raccourcis.plein_ecran", Raison::Invalide),
        ]
    );
    let d_w = Raccourcis::par_defaut(W);
    assert_eq!(r.raccourcis.zone, d_w.zone);
    assert_eq!(r.raccourcis.plein_ecran, d_w.plein_ecran);
    assert_eq!(r.raccourcis.fenetre, vec!["Alt+F".to_string()]);
    assert_stable(&d);
}

#[test]
fn raccourcis_action_inconnue_conservee() {
    let (d, r, c) = charger_json(defaut_json(|m| {
        m["raccourcis"]["texte"] = json!(["Ctrl+Maj+4"]);
        m["raccourcis"]["bizarre"] = json!({ "pas": "une liste" });
        m["theme"] = json!(7); // force une réécriture
    }));
    assert_eq!(c, vec![remplacee("theme", Raison::Invalide)]);
    assert_eq!(r.raccourcis.candidats("texte"), None);
    let f = json_du_fichier(&d);
    assert_eq!(f["raccourcis"]["texte"], json!(["Ctrl+Maj+4"]));
    assert_eq!(f["raccourcis"]["bizarre"], json!({ "pas": "une liste" }));
    assert_eq!(f["raccourcis"]["zone"], json!(["ImprEcran", "Ctrl+Maj+1"]));
    assert_stable(&d);
}

#[test]
fn raccourcis_pas_un_objet_ou_action_absente() {
    let (_, r, c) = charger_json(defaut_json(|m| {
        m.insert("raccourcis".into(), json!(["ImprEcran"]));
    }));
    assert_eq!(c, vec![remplacee("raccourcis", Raison::Invalide)]);
    assert_eq!(r.raccourcis, Raccourcis::par_defaut(W));

    let (_, r, c) = charger_json(defaut_json(|m| {
        if let Some(Value::Object(rc)) = m.get_mut("raccourcis") {
            rc.remove("plein_ecran");
        }
    }));
    assert_eq!(
        c,
        vec![remplacee("raccourcis.plein_ecran", Raison::Absente)]
    );
    assert_eq!(r.raccourcis.plein_ecran, vec!["Ctrl+Maj+2".to_string()]);
}

// ---------- Champs inconnus ----------

#[test]
fn champs_inconnus_conserves_a_la_reecriture() {
    let (d, _, c) = charger_json(defaut_json(|m| {
        m.insert("futur".into(), json!({ "a": [1, 2], "b": null }));
        m.insert("note".into(), json!("garde-moi"));
        m.insert("copier_apres_capture".into(), json!("faux")); // force une réécriture
    }));
    assert_eq!(c, vec![remplacee("copier_apres_capture", Raison::Invalide)]);
    let texte = String::from_utf8(octets(&d)).unwrap();
    let f: Value = serde_json::from_str(&texte).unwrap();
    assert_eq!(f["futur"], json!({ "a": [1, 2], "b": null }));
    assert_eq!(f["note"], json!("garde-moi"));
    // Les inconnus viennent après les champs connus, dans leur ordre d'origine.
    let pos = |s: &str| texte.find(s).unwrap();
    assert!(pos("\"theme\"") < pos("\"futur\"") && pos("\"futur\"") < pos("\"note\""));
    assert_stable(&d);
}

#[test]
fn champs_inconnus_gardes_par_ecrire() {
    let d = dossier();
    poser_json(
        &d,
        defaut_json(|m| {
            m.insert("futur".into(), json!(1));
        }),
    );
    let (mut r, c) = charger(d.path(), W);
    assert_eq!(c, vec![]);
    r.demarrer_avec_systeme = true;
    ecrire(d.path(), &r).unwrap();
    let f = json_du_fichier(&d);
    assert_eq!(f["futur"], json!(1));
    assert_eq!(f["demarrer_avec_systeme"], json!(true));
}

#[test]
fn fichier_correct_autrement_mis_en_forme_non_reecrit() {
    let d = dossier();
    let compact = serde_json::to_string(&defaut_json(|m| {
        let theme = m.shift_remove("theme").unwrap();
        m.insert("theme".into(), theme); // autre ordre
    }))
    .unwrap();
    poser(&d, &compact);
    let (_, c) = charger(d.path(), W);
    assert_eq!(c, vec![]);
    assert_eq!(octets(&d), compact.as_bytes());
}

// ---------- Versions ----------

#[test]
fn version_0_mise_a_niveau_sans_rien_effacer() {
    let (d, r, c) = charger_json(json!({
        "theme": "sombre",
        "vignette": { "duree_s": 10, "taille": "grande" },
        "raccourcis": { "zone": ["F9"] },
        "vieux": true
    }));
    assert_eq!(c, vec![Constat::MisANiveau { de: 0, a: 2 }]);
    assert_eq!(r.theme, Theme::Sombre);
    assert_eq!(r.vignette.duree_s, 10);
    assert!(r.vignette.afficher);
    assert_eq!(r.raccourcis.zone, vec!["F9".to_string()]);
    assert_eq!(r.raccourcis.fenetre, vec!["Ctrl+Maj+3".to_string()]);
    let f = json_du_fichier(&d);
    assert_eq!(f["version"], json!(2));
    assert_eq!(f["vieux"], json!(true));
    assert_eq!(f["vignette"]["taille"], json!("grande"));
    assert_eq!(f["vignette"]["coin"], json!("bas_gauche"));
    assert_stable(&d);
}

#[test]
fn version_0_objet_vide() {
    let (d, r, c) = charger_json(json!({}));
    assert_eq!(c, vec![Constat::MisANiveau { de: 0, a: 2 }]);
    assert_eq!(r, Reglages::par_defaut(W));
    assert_stable(&d);
}

#[test]
fn version_0_avec_valeur_invalide() {
    let (_, r, c) = charger_json(json!({ "theme": "rose" }));
    assert_eq!(
        c,
        vec![
            Constat::MisANiveau { de: 0, a: 2 },
            remplacee("theme", Raison::Invalide)
        ]
    );
    assert_eq!(r.theme, Theme::Systeme);
}

#[test]
fn version_invalide_traitee_comme_0() {
    for v in [json!("1"), json!(-1), json!(1.5)] {
        let (d, _, c) = charger_json(defaut_json(|m| m["version"] = v.clone()));
        assert_eq!(
            c,
            vec![
                remplacee("version", Raison::Invalide),
                Constat::MisANiveau { de: 0, a: 2 }
            ],
            "{v}"
        );
        assert_eq!(json_du_fichier(&d)["version"], json!(2));
        assert_stable(&d);
    }
}

#[test]
fn version_plus_recente_lue_sans_reecriture() {
    let d = dossier();
    let v = defaut_json(|m| {
        m["version"] = json!(3);
        m["theme"] = json!("contraste");
        m.insert("nouveau".into(), json!([1]));
        m["copier_apres_capture"] = json!(false);
        m["vignette"]["afficher"] = json!(false);
    });
    poser_json(&d, v);
    let avant = octets(&d);
    let (r, c) = charger(d.path(), W);
    assert_eq!(
        c,
        vec![
            remplacee("theme", Raison::Invalide),
            Constat::CopieRallumee,
            Constat::VersionPlusRecente {
                version: 3,
                connue: 2
            },
        ]
    );
    assert_eq!(r.version_fichier(), 3);
    assert!(r.copier_apres_capture, "copie rallumée en mémoire");
    assert_eq!(octets(&d), avant, "fichier non réécrit");
    assert!(matches!(
        ecrire(d.path(), &r),
        Err(ErreurEcriture::VersionPlusRecente { version: 3 })
    ));
    assert_eq!(octets(&d), avant);
}

#[test]
fn version_gigantesque_ou_decimale_plus_recente() {
    for (v, lue) in [
        (json!(4294967296u64), u32::MAX),
        (json!(1e30), u32::MAX),
        (json!(3.0), 3),
    ] {
        let d = dossier();
        poser_json(
            &d,
            defaut_json(|m| {
                m["version"] = v.clone();
                m.insert("futur".into(), json!("garde"));
            }),
        );
        let avant = octets(&d);
        let (r, c) = charger(d.path(), W);
        assert_eq!(
            c,
            vec![Constat::VersionPlusRecente {
                version: lue,
                connue: 2
            }],
            "{v}"
        );
        assert_eq!(r.version_fichier(), lue);
        assert!(!r.enregistrable());
        assert_eq!(octets(&d), avant, "{v} : fichier non reecrit");
    }
}

#[test]
fn version_2_0_lue_comme_2() {
    let (d, r, c) = charger_json(defaut_json(|m| m["version"] = json!(2.0)));
    assert_eq!(c, vec![]);
    assert_eq!(r.version_fichier(), 2);
    assert_eq!(json_du_fichier(&d)["version"], json!(2));
    assert_stable(&d);
}

// ---------- Au moins une sortie ----------

#[test]
fn aucune_sortie_copie_rallumee_a_la_lecture() {
    let (d, r, c) = charger_json(defaut_json(|m| {
        m["copier_apres_capture"] = json!(false);
        m["enregistrer_apres_capture"] = json!(false);
        m["vignette"]["afficher"] = json!(false);
    }));
    assert_eq!(c, vec![Constat::CopieRallumee]);
    assert!(r.copier_apres_capture);
    assert!(!r.vignette.afficher);
    assert_eq!(json_du_fichier(&d)["copier_apres_capture"], json!(true));
    assert_stable(&d);
}

#[test]
fn une_seule_sortie_suffit() {
    let (_, r, c) = charger_json(defaut_json(|m| {
        m["copier_apres_capture"] = json!(false);
        m["vignette"]["afficher"] = json!(false);
        m["enregistrer_apres_capture"] = json!(true);
    }));
    assert_eq!(c, vec![]);
    assert!(!r.copier_apres_capture);
}

#[test]
fn ecrire_refuse_aucune_sortie() {
    let d = dossier();
    charger(d.path(), W);
    let avant = octets(&d);
    let mut r = Reglages::par_defaut(W);
    r.copier_apres_capture = false;
    r.vignette.afficher = false;
    assert!(matches!(
        ecrire(d.path(), &r),
        Err(ErreurEcriture::AucuneSortie)
    ));
    assert_eq!(octets(&d), avant);
}

// ---------- Écriture ----------

#[test]
fn ecrire_puis_charger_aller_retour() {
    let d = dossier();
    let mut r = Reglages::par_defaut(W);
    r.vignette.coin = Coin::HautDroite;
    r.vignette.duree_s = 60;
    r.theme = Theme::Sombre;
    r.dossier_enregistrement = Some(PathBuf::from("/tmp/é"));
    r.raccourcis.zone = vec![];
    r.selection.loupe = true;
    ecrire(d.path(), &r).unwrap();
    assert_eq!(
        noms(d.path()),
        vec![NOM_FICHIER.to_string()],
        "pas de .tmp laissé"
    );
    let (lu, c) = charger(d.path(), W);
    assert_eq!(c, vec![]);
    assert_eq!(lu, r);
}

#[test]
fn ecrire_remplace_un_temporaire_oublie() {
    let d = dossier();
    fs::write(d.path().join("reglages.json.tmp"), "reste d'une coupure").unwrap();
    ecrire(d.path(), &Reglages::par_defaut(W)).unwrap();
    assert_eq!(noms(d.path()), vec![NOM_FICHIER.to_string()]);
}

#[test]
fn ecrire_refuse_les_valeurs_invalides() {
    let d = dossier();
    type Cas = (&'static str, fn(&mut Reglages));
    let cas: [Cas; 4] = [
        ("vignette.duree_s", |r| r.vignette.duree_s = 1000),
        ("vignette.duree_s", |r| r.vignette.duree_s = 61),
        ("modele_nom", |r| r.modele_nom = " ".into()),
        ("dossier_enregistrement", |r| {
            r.dossier_enregistrement = Some(PathBuf::new())
        }),
    ];
    for (champ_attendu, f) in cas {
        let mut r = Reglages::par_defaut(W);
        f(&mut r);
        match ecrire(d.path(), &r) {
            Err(ErreurEcriture::ValeurInvalide { champ }) => assert_eq!(champ, champ_attendu),
            autre => panic!("{champ_attendu} : {autre:?}"),
        }
    }
    assert!(!chemin(&d).exists());
}

#[test]
fn fichier_en_lecture_seule() {
    let d = dossier();
    poser_json(&d, defaut_json(|m| m["theme"] = json!("violet")));
    let mut droits = fs::metadata(chemin(&d)).unwrap().permissions();
    droits.set_readonly(true);
    fs::set_permissions(chemin(&d), droits).unwrap();
    let avant = octets(&d);

    let (r, c) = charger(d.path(), W);
    assert_eq!(r.theme, Theme::Systeme, "corrigé en mémoire");
    assert!(matches!(
        c.as_slice(),
        [
            Constat::ValeurRemplacee { .. },
            Constat::EcritureImpossible { .. }
        ]
    ));
    assert_eq!(octets(&d), avant, "fichier laissé tel quel");
    assert!(matches!(
        ecrire(d.path(), &r),
        Err(ErreurEcriture::LectureSeule { .. })
    ));
    assert_eq!(noms(d.path()), vec![NOM_FICHIER.to_string()]);
}

// ---------- Textes du journal ----------

#[test]
fn constats_lisibles_et_sans_accents() {
    let constats = [
        Constat::Cree {
            chemin: PathBuf::from("r.json"),
        },
        Constat::Abime {
            raison: "pas un objet JSON".into(),
            sauvegarde: PathBuf::from("a.json"),
        },
        Constat::AbimeNonSauvegarde {
            raison: "x".into(),
            erreur: "y".into(),
        },
        Constat::AncienneSauvegardeSupprimee {
            chemin: PathBuf::from("a.json"),
        },
        Constat::LectureImpossible { erreur: "e".into() },
        Constat::MisANiveau { de: 0, a: 2 },
        remplacee("vignette.duree_s", Raison::Invalide),
        remplacee("theme", Raison::Absente),
        Constat::CopieRallumee,
        Constat::VersionPlusRecente {
            version: 2,
            connue: 1,
        },
        Constat::EcritureImpossible {
            erreur: "disque plein".into(),
        },
    ];
    for c in &constats {
        let t = c.to_string();
        assert!(t.starts_with("reglages"), "{t}");
        assert!(t.is_ascii(), "{t}");
    }
    assert_eq!(
        constats[6].to_string(),
        "reglages : valeur invalide pour vignette.duree_s, remplacee par la valeur par defaut"
    );
    assert_eq!(
        constats[5].to_string(),
        "reglages mis a niveau de la version 0 a la version 2"
    );
    assert!(ErreurEcriture::AucuneSortie.to_string().is_ascii());
}

// ---------- Version 2 (étape 10) ----------

/// Un fichier de version 1 passe en version 2 sans rien perdre ni changer d'autre ; sa
/// durée « toujours » (0) n'existait pas en version 1 : elle y était invalide.
#[test]
fn version_1_mise_a_niveau_vers_2() {
    let (d, r, c) = charger_json(defaut_json(|m| {
        m["version"] = json!(1);
        m["theme"] = json!("sombre");
        m["vignette"]["duree_s"] = json!(10);
        m.insert("inconnu".into(), json!({ "a": 1 }));
    }));
    assert_eq!(c, vec![Constat::MisANiveau { de: 1, a: 2 }]);
    assert_eq!(r.theme, Theme::Sombre);
    assert_eq!(r.vignette.duree_s, 10);
    let f = json_du_fichier(&d);
    assert_eq!(f["version"], json!(2));
    assert_eq!(f["inconnu"], json!({ "a": 1 }));
    assert_stable(&d);
}

#[test]
fn duree_toujours_ecrite_et_relue() {
    let d = dossier();
    let mut r = Reglages::par_defaut(W);
    r.vignette.duree_s = DUREE_TOUJOURS;
    ecrire(d.path(), &r).unwrap();
    assert_eq!(json_du_fichier(&d)["vignette"]["duree_s"], json!(0));
    let (relu, c) = charger(d.path(), W);
    assert_eq!(c, vec![]);
    assert_eq!(relu.vignette.duree_s, DUREE_TOUJOURS);
}

#[test]
fn valider_sans_disque() {
    let r = Reglages::par_defaut(W);
    assert!(valider(&r).is_ok());
    let mut sans_sortie = r.clone();
    sans_sortie.copier_apres_capture = false;
    sans_sortie.vignette.afficher = false;
    assert!(matches!(
        valider(&sans_sortie),
        Err(ErreurEcriture::AucuneSortie)
    ));
    let mut vide = r.clone();
    vide.modele_nom = "  ".into();
    assert!(matches!(
        valider(&vide),
        Err(ErreurEcriture::ValeurInvalide {
            champ: "modele_nom"
        })
    ));
    // Des réglages protégés restent valides : seule l'écriture les refuse.
    assert!(valider(&Reglages::par_defaut_protege(W)).is_ok());
}

#[test]
fn codes_sans_chemin() {
    let e = ErreurEcriture::LectureSeule {
        chemin: PathBuf::from("/home/lea/reglages.json"),
    };
    assert_eq!(e.code(), "lecture_seule");
    let c = Constat::Cree {
        chemin: PathBuf::from("/home/lea/reglages.json"),
    };
    assert_eq!(c.code(), "cree");
    assert_eq!(Constat::MisANiveau { de: 1, a: 2 }.code(), "mis_a_niveau");
}

#[test]
fn modele_trop_long_remplace_a_la_lecture() {
    let d = dossier();
    let long = "a".repeat(MODELE_NOM_MAX + 1);
    fs::write(
        chemin(&d),
        json!({ "version": 2, "modele_nom": long }).to_string(),
    )
    .unwrap();
    let (r, _) = charger(d.path(), W);
    assert_eq!(r.modele_nom, Reglages::par_defaut(W).modele_nom);
    // Écrit par l'app juste après : jamais refusé à cause du nom lu.
    assert!(valider(&r).is_ok());
}
