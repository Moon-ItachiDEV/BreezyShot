use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_compact::{KeyPair, Seed};

use super::*;

/// La licence d'essai du dépôt, telle que rangée (fin de ligne comprise).
const ESSAI: &str = include_str!("../../../../essais/licence-essai.txt");

/// Une paire faite d'une graine fixe : jamais de hasard.
fn paire() -> KeyPair {
    KeyPair::from_seed(Seed::new([7u8; 32]))
}

fn cle(paire: &KeyPair) -> [u8; 32] {
    *paire.pk
}

/// Une licence de `charge` signée par `paire`.
fn faire(charge: &str, paire: &KeyPair) -> String {
    let signature = paire.sk.sign(charge.as_bytes(), None);
    format!(
        "BS1.{}.{}",
        URL_SAFE_NO_PAD.encode(charge),
        URL_SAFE_NO_PAD.encode(*signature)
    )
}

fn parties(texte: &str) -> Vec<String> {
    texte.trim().split('.').map(String::from).collect()
}

/// Change un caractère base64 de la partie `n` sans sortir de l'alphabet ni toucher aux
/// bits de fin (le premier caractère porte six bits pleins).
fn abimer(texte: &str, n: usize) -> String {
    let mut p = parties(texte);
    let premier = if p[n].starts_with('A') { "B" } else { "A" };
    p[n].replace_range(0..1, premier);
    p.join(".")
}

#[test]
fn licence_d_essai_acceptee() {
    assert_eq!(
        verifier(ESSAI),
        Ok(Licence {
            id: "ESSAI-0001".into(),
            essai: true,
        })
    );
}

#[test]
fn octet_change_dans_la_charge_ou_la_signature() {
    assert_eq!(verifier(&abimer(ESSAI, 1)), Err(Refus::Signature));
    assert_eq!(verifier(&abimer(ESSAI, 2)), Err(Refus::Signature));
    // Un octet de la charge décodée, re-encodée proprement.
    let p = parties(ESSAI);
    let mut charge = URL_SAFE_NO_PAD.decode(&p[1]).unwrap();
    charge[7] ^= 1;
    let texte = format!("BS1.{}.{}", URL_SAFE_NO_PAD.encode(&charge), p[2]);
    assert_eq!(verifier(&texte), Err(Refus::Signature));
}

#[test]
fn prefixe_faux() {
    let p = parties(ESSAI);
    for prefixe in ["BS2", "bs1", "", "BS1 "] {
        let texte = format!("{prefixe}.{}.{}", p[1], p[2]);
        assert_eq!(verifier(&texte), Err(Refus::Forme), "{prefixe:?}");
    }
}

#[test]
fn deux_ou_quatre_parties() {
    let p = parties(ESSAI);
    assert_eq!(verifier(&format!("BS1.{}", p[1])), Err(Refus::Forme));
    assert_eq!(verifier(&format!("{}.", ESSAI.trim())), Err(Refus::Forme));
    assert_eq!(verifier(&format!("{}.x", ESSAI.trim())), Err(Refus::Forme));
    assert_eq!(verifier("BS1"), Err(Refus::Forme));
}

#[test]
fn base64_invalide() {
    let p = parties(ESSAI);
    assert_eq!(p[1].len(), 56);
    assert!(p[2].contains('_'));
    let cas = [
        // Remplissage, alphabet standard, caractère hors alphabet, espace au milieu.
        format!("BS1.{}=.{}", p[1], p[2]),
        format!("BS1.{}.{}==", p[1], p[2]),
        format!("BS1.+{}.{}", &p[1][1..], p[2]),
        format!("BS1.{}.{}", p[1], p[2].replace('_', "/")),
        format!("BS1.{}!.{}", p[1], p[2]),
        format!("BS1.{} .{}", p[1], p[2]),
        // Longueur impossible : 57 caractères ne font aucun nombre d'octets.
        format!("BS1.{}A.{}", p[1], p[2]),
        // Bits de fin non nuls : `Q` (0x10) termine la signature, `R` ajoute un bit perdu.
        format!("BS1.{}.{}R", p[1], &p[2][..p[2].len() - 1]),
    ];
    for texte in &cas {
        assert_eq!(verifier(texte), Err(Refus::Forme), "{texte}");
    }
}

#[test]
fn signature_de_mauvaise_taille() {
    let p = parties(ESSAI);
    let courte = URL_SAFE_NO_PAD.encode([0u8; 63]);
    assert_eq!(
        verifier(&format!("BS1.{}.{courte}", p[1])),
        Err(Refus::Forme)
    );
    assert_eq!(verifier(&format!("BS1.{}.", p[1])), Err(Refus::Forme));
}

#[test]
fn edition_inconnue() {
    let paire = paire();
    let cles = [cle(&paire)];
    for e in ["essai", "Pro", ""] {
        let texte = faire(&format!(r#"{{"e":"{e}","id":"X-1"}}"#), &paire);
        assert_eq!(verifier_avec(&texte, &cles), Err(Refus::Edition), "{e}");
    }
}

#[test]
fn id_vide_trop_long_ou_non_imprimable() {
    let paire = paire();
    let cles = [cle(&paire)];
    let long = "A".repeat(41);
    for id in [
        "",
        long.as_str(),
        "a\\nb",
        "a\\u0000",
        "\\u00e9t\\u00e9",
        "\\u007f",
    ] {
        let texte = faire(&format!(r#"{{"e":"pro","id":"{id}"}}"#), &paire);
        assert_eq!(verifier_avec(&texte, &cles), Err(Refus::Charge), "{id}");
    }
    let juste = "~".repeat(40);
    let texte = faire(&format!(r#"{{"e":"pro","id":"{juste}"}}"#), &paire);
    assert_eq!(
        verifier_avec(&texte, &cles).map(|l| l.id),
        Ok(juste.clone())
    );
}

#[test]
fn charge_illisible() {
    let paire = paire();
    let cles = [cle(&paire)];
    let cas = [
        "",
        "pas du json",
        "[]",
        r#"{"id":"X"}"#,
        r#"{"e":"pro"}"#,
        r#"{"e":"pro","id":7}"#,
        r#"{"e":"pro","id":"X","essai":"oui"}"#,
        r#"{"e":"pro","id":"X"} reste"#,
    ];
    for charge in cas {
        let texte = faire(charge, &paire);
        assert_eq!(verifier_avec(&texte, &cles), Err(Refus::Charge), "{charge}");
    }
}

#[test]
fn champ_inconnu_accepte_essai_absent_faux() {
    let paire = paire();
    let cles = [cle(&paire)];
    let texte = faire(
        r#"{"e":"pro","id":"VENTE-42","appareils":3,"x":{"y":[]}}"#,
        &paire,
    );
    assert_eq!(
        verifier_avec(&texte, &cles),
        Ok(Licence {
            id: "VENTE-42".into(),
            essai: false,
        })
    );
}

#[test]
fn autre_cle_refusee_par_verifier_acceptee_par_verifier_avec() {
    let paire = paire();
    let texte = faire(r#"{"e":"pro","id":"AUTRE-1","essai":true}"#, &paire);
    assert_eq!(verifier(&texte), Err(Refus::Signature));
    assert_eq!(
        verifier_avec(&texte, &[cle(&paire)]),
        Ok(Licence {
            id: "AUTRE-1".into(),
            essai: true,
        })
    );
    // Une liste de plusieurs clés : une seule suffit ; la licence d'essai n'y est plus.
    assert!(verifier_avec(&texte, &[CLE_ESSAI, cle(&paire)]).is_ok());
    assert_eq!(verifier_avec(ESSAI, &[cle(&paire)]), Err(Refus::Signature));
    assert_eq!(verifier_avec(ESSAI, &[]), Err(Refus::Signature));
}

#[test]
fn signature_verifiee_avant_la_charge() {
    // Charge illisible et édition fausse, mal signées : c'est la signature qui refuse.
    let paire = paire();
    for charge in ["pas du json", r#"{"e":"essai","id":"X"}"#] {
        let texte = faire(charge, &paire);
        assert_eq!(verifier(&texte), Err(Refus::Signature), "{charge}");
    }
}

#[test]
fn espaces_et_fins_de_ligne_autour() {
    let nue = ESSAI.trim_end_matches('\n');
    for texte in [
        format!("  {nue}  "),
        format!("\t{nue}\r\n"),
        format!("\r\n\n {nue}\n\t \r\n"),
    ] {
        assert!(verifier(&texte).is_ok(), "{texte:?}");
    }
    // Au milieu, ce n'est plus la même licence.
    let p = parties(ESSAI);
    let texte = format!("BS1.{}.\n{}", p[1], p[2]);
    assert_eq!(verifier(&texte), Err(Refus::Forme));
    // Un autre blanc que ceux-là n'est pas retiré.
    assert_eq!(verifier(&format!("\u{a0}{nue}")), Err(Refus::Forme));
}

#[test]
fn bom_devant_refuse_forme() {
    // Un `licence.txt` enregistré « UTF-8 avec BOM » : le BOM n'est pas un blanc retiré.
    let nue = ESSAI.trim_end_matches('\n');
    assert!(verifier(nue).is_ok());
    assert_eq!(verifier(&format!("\u{feff}{nue}")), Err(Refus::Forme));
    assert_eq!(verifier(&format!("\u{feff}{ESSAI}")), Err(Refus::Forme));
    assert_eq!(
        nettoyer(&format!("\u{feff}{nue}")),
        format!("\u{feff}{nue}")
    );
}

#[test]
fn nettoyer_retire_les_seuls_blancs_des_bouts() {
    let nue = ESSAI.trim_end_matches('\n');
    assert_eq!(nettoyer(ESSAI), nue);
    assert_eq!(nettoyer(&format!("{}{nue}\r\n\t", " ".repeat(70_000))), nue);
    assert_eq!(nettoyer(" \r\n\t "), "");
    assert_eq!(nettoyer(" a b "), "a b");
    assert_eq!(nettoyer("\u{a0}a"), "\u{a0}a");
}

#[test]
fn vide_et_trop_longue() {
    for texte in ["", "   ", "\r\n\t "] {
        assert_eq!(verifier(texte), Err(Refus::Vide), "{texte:?}");
    }
    let juste = format!("BS1.{}", "A".repeat(LONGUEUR_MAX - 4));
    assert_eq!(juste.chars().count(), LONGUEUR_MAX);
    assert_eq!(verifier(&juste), Err(Refus::Forme));
    let trop = format!("{juste}A");
    assert_eq!(verifier(&trop), Err(Refus::TropLongue));
    // Bornée en caractères, avant tout décodage, même énorme ou non ASCII.
    assert_eq!(verifier(&"é".repeat(1001)), Err(Refus::TropLongue));
    assert_eq!(verifier(&"A".repeat(1_000_000)), Err(Refus::TropLongue));
    // Les blancs des bouts ne comptent pas.
    assert!(verifier(&format!("{}{ESSAI}", " ".repeat(2000))).is_ok());
}

#[test]
fn codes_des_refus() {
    let codes: Vec<_> = [
        Refus::Vide,
        Refus::TropLongue,
        Refus::Forme,
        Refus::Charge,
        Refus::Edition,
        Refus::Signature,
    ]
    .iter()
    .map(Refus::code)
    .collect();
    assert_eq!(
        codes,
        [
            "vide",
            "trop_longue",
            "forme",
            "charge",
            "edition",
            "signature"
        ]
    );
}
