use super::*;
use crate::systeme::texte::MotBrut;

fn image(largeur: u32, hauteur: u32, rgba: [u8; 4]) -> Pixels {
    Pixels {
        largeur,
        hauteur,
        rgba: rgba.repeat((largeur * hauteur) as usize),
    }
}

/// Le pixel `(x, y)` d'une image BGRA.
fn pixel(p: &Preparee, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * p.largeur + x) * 4) as usize;
    p.bgra[i..i + 4].try_into().unwrap()
}

fn mot(texte: &str, x: f32, y: f32, largeur: f32, hauteur: f32) -> MotBrut {
    MotBrut {
        texte: texte.into(),
        x,
        y,
        largeur,
        hauteur,
    }
}

fn ligne(texte: &str, mots: &[&str]) -> LigneBrute {
    LigneBrute {
        texte: texte.into(),
        mots: mots
            .iter()
            .enumerate()
            .map(|(i, m)| mot(m, i as f32 * 10.0, 0.0, 8.0, 8.0))
            .collect(),
    }
}

/// Trois lignes de l'essai Windows, la deuxième vide.
fn lu_exemple() -> Lu {
    let r = Reconnu {
        langue: "en-US".into(),
        lignes: vec![
            ligne(
                "BreezyShot lit ce texte",
                &["BreezyShot", "lit", "ce", "texte"],
            ),
            ligne("", &[]),
            ligne("Version 0.5", &["Version", "0.5"]),
            ligne("0123456789", &["0123456789"]),
        ],
    };
    depuis_reconnu(r, 1.0, 1000, 1000)
}

// ---------- preparer ----------

#[test]
fn petite_image_agrandie_deux_fois_au_plus_proche() {
    let mut p = image(2, 1, [0, 0, 0, 255]);
    p.rgba[4..8].copy_from_slice(&[10, 20, 30, 255]);
    let prep = preparer(&p, 10000).expect("image");
    assert_eq!((prep.largeur, prep.hauteur, prep.facteur), (4, 2, 2.0));
    assert_eq!(prep.bgra.len(), 4 * 2 * 4);
    for y in 0..2 {
        assert_eq!(pixel(&prep, 0, y), [0, 0, 0, 255]);
        assert_eq!(pixel(&prep, 1, y), [0, 0, 0, 255]);
        // BGRA : bleu d'abord.
        assert_eq!(pixel(&prep, 2, y), [30, 20, 10, 255]);
        assert_eq!(pixel(&prep, 3, y), [30, 20, 10, 255]);
    }
}

#[test]
fn agrandie_jusqu_a_un_million_de_pixels_compris() {
    let prep = preparer(&image(1000, 1000, [0, 0, 0, 255]), 10000).unwrap();
    assert_eq!(
        (prep.largeur, prep.hauteur, prep.facteur),
        (2000, 2000, 2.0)
    );
    let prep = preparer(&image(1001, 1000, [0, 0, 0, 255]), 10000).unwrap();
    assert_eq!(
        (prep.largeur, prep.hauteur, prep.facteur),
        (1001, 1000, 1.0)
    );
}

#[test]
fn pas_agrandie_si_le_double_depasse_la_dimension_max() {
    let prep = preparer(&image(600, 10, [0, 0, 0, 255]), 1000).unwrap();
    assert_eq!((prep.largeur, prep.hauteur, prep.facteur), (600, 10, 1.0));
    let prep = preparer(&image(500, 10, [0, 0, 0, 255]), 1000).unwrap();
    assert_eq!((prep.largeur, prep.hauteur, prep.facteur), (1000, 20, 2.0));
}

#[test]
fn reduite_au_dela_de_la_dimension_max() {
    // 10000 mesuré sur la machine d'essai : une image de 12000 × 120 est réduite.
    let prep = preparer(&image(12000, 120, [0, 0, 0, 255]), 10000).unwrap();
    assert_eq!((prep.largeur, prep.hauteur), (10000, 100));
    assert!((prep.facteur - 10000.0 / 12000.0).abs() < 1e-6);
    assert_eq!(prep.bgra.len(), 10000 * 100 * 4);

    // Moyenne des pixels couverts : colonnes noire et blanche alternées → gris.
    let mut p = image(8, 2, [0, 0, 0, 255]);
    for x in (1..8).step_by(2) {
        for y in 0..2 {
            let i = ((y * 8 + x) * 4) as usize;
            p.rgba[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
        }
    }
    let prep = preparer(&p, 4).unwrap();
    assert_eq!((prep.largeur, prep.hauteur, prep.facteur), (4, 1, 0.5));
    for x in 0..4 {
        assert_eq!(pixel(&prep, x, 0), [128, 128, 128, 255]);
    }
}

#[test]
fn reduite_garde_au_moins_un_pixel() {
    let prep = preparer(&image(100, 1, [0, 0, 0, 255]), 10).unwrap();
    assert_eq!((prep.largeur, prep.hauteur), (10, 1));
}

#[test]
fn alpha_compose_sur_blanc() {
    let mut p = image(3, 1, [255, 0, 0, 0]);
    p.rgba[4..8].copy_from_slice(&[255, 0, 0, 128]);
    p.rgba[8..12].copy_from_slice(&[0, 0, 255, 255]);
    let prep = preparer(&p, 10000).unwrap();
    // Transparent : blanc ; à moitié : rouge pâle ; opaque : tel quel (BGRA).
    assert_eq!(pixel(&prep, 0, 0), [255, 255, 255, 255]);
    assert_eq!(pixel(&prep, 2, 0), [127, 127, 255, 255]);
    assert_eq!(pixel(&prep, 4, 0), [255, 0, 0, 255]);
    assert!(prep.bgra.chunks_exact(4).all(|p| p[3] == 255));
}

#[test]
fn image_vide_ou_fausse_refusee() {
    assert_eq!(preparer(&image(0, 5, [0; 4]), 10000), None);
    assert_eq!(preparer(&image(5, 0, [0; 4]), 10000), None);
    let mut p = image(2, 2, [0; 4]);
    p.rgba.pop();
    assert_eq!(preparer(&p, 10000), None);
    assert_eq!(preparer(&image(2, 2, [0; 4]), 0), None);
}

// ---------- depuis_reconnu ----------

#[test]
fn boites_divisees_par_le_facteur() {
    let r = Reconnu {
        langue: "en-US".into(),
        lignes: vec![LigneBrute {
            texte: "Bonjour".into(),
            mots: vec![mot("Bonjour", 21.0, 9.0, 40.0, 17.0)],
        }],
    };
    let lu = depuis_reconnu(r, 2.0, 100, 100);
    assert_eq!(lu.langue, "en-US");
    // 10,5 → 10 ; 30,5 → 31 ; 4,5 → 4 ; 13 → 13.
    assert_eq!(
        lu.mots,
        vec![Mot {
            x: 10,
            y: 4,
            largeur: 21,
            hauteur: 9,
            ligne: 0
        }]
    );
}

#[test]
fn boites_bornees_a_l_image() {
    let r = Reconnu {
        langue: "en-US".into(),
        lignes: vec![LigneBrute {
            texte: "a b c".into(),
            mots: vec![
                mot("a", -5.0, -3.0, 10.0, 10.0),
                mot("b", 90.0, 45.0, 30.0, 30.0),
                mot("c", f32::NAN, 200.0, f32::NAN, 5.0),
            ],
        }],
    };
    let lu = depuis_reconnu(r, 1.0, 100, 50);
    assert_eq!((lu.mots[0].x, lu.mots[0].y), (0, 0));
    assert_eq!((lu.mots[0].largeur, lu.mots[0].hauteur), (5, 7));
    assert_eq!((lu.mots[1].x, lu.mots[1].y), (90, 45));
    assert_eq!((lu.mots[1].largeur, lu.mots[1].hauteur), (10, 5));
    assert_eq!((lu.mots[2].x, lu.mots[2].y), (0, 50));
    assert_eq!((lu.mots[2].largeur, lu.mots[2].hauteur), (0, 0));
    for m in &lu.mots {
        assert!(m.x + m.largeur <= 100 && m.y + m.hauteur <= 50);
    }
}

#[test]
fn facteur_faux_pris_pour_un() {
    let r = Reconnu {
        langue: String::new(),
        lignes: vec![ligne("a", &["a"])],
    };
    for f in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let lu = depuis_reconnu(r.clone(), f, 100, 100);
        assert_eq!((lu.mots[0].x, lu.mots[0].largeur), (0, 8));
    }
}

#[test]
fn lignes_vides_sautees_et_ordre_garde() {
    let lu = lu_exemple();
    assert_eq!(lu.mots.len(), 7);
    let lignes: Vec<u32> = lu.mots.iter().map(|m| m.ligne).collect();
    assert_eq!(lignes, vec![0, 0, 0, 0, 1, 1, 2]);
}

// ---------- separateur ----------

#[test]
fn separateur_latin_et_japonais() {
    assert_eq!(separateur(&ligne("Version 0.5", &["Version", "0.5"])), " ");
    assert_eq!(
        separateur(&ligne("日本語の文章", &["日本", "語の", "文章"])),
        ""
    );
    // Le moteur peut mettre des espaces entre des idéogrammes : pas de séparateur.
    assert_eq!(separateur(&ligne("日 本 語", &["日", "本", "語"])), "");
    // Coréen : des espaces entre les mots.
    assert_eq!(separateur(&ligne("한국어 문장", &["한국어", "문장"])), " ");
    // Mélange : des espaces.
    assert_eq!(
        separateur(&ligne("Windows 日本", &["Windows", "日本"])),
        " "
    );
}

// ---------- assembler, extrait ----------

#[test]
fn assembler_avec_la_fin_de_ligne_donnee() {
    let lu = lu_exemple();
    assert_eq!(
        assembler(&lu, "\r\n"),
        "BreezyShot lit ce texte\r\nVersion 0.5\r\n0123456789"
    );
    assert_eq!(
        assembler(&lu, "\n"),
        "BreezyShot lit ce texte\nVersion 0.5\n0123456789"
    );
    let vide = depuis_reconnu(
        Reconnu {
            langue: String::new(),
            lignes: vec![ligne("", &[])],
        },
        1.0,
        10,
        10,
    );
    assert_eq!(assembler(&vide, "\r\n"), "");
}

#[test]
fn assembler_japonais_sans_espace() {
    let lu = depuis_reconnu(
        Reconnu {
            langue: "ja".into(),
            lignes: vec![
                ligne("日 本 語", &["日", "本", "語"]),
                ligne("文章", &["文章"]),
            ],
        },
        1.0,
        100,
        100,
    );
    assert_eq!(assembler(&lu, "\n"), "日本語\n文章");
}

#[test]
fn extrait_d_une_ligne_et_a_cheval() {
    let lu = lu_exemple();
    assert_eq!(
        extrait(&lu, 0, 2, "\r\n").as_deref(),
        Some("BreezyShot lit ce")
    );
    assert_eq!(extrait(&lu, 3, 3, "\r\n").as_deref(), Some("texte"));
    assert_eq!(
        extrait(&lu, 2, 4, "\r\n").as_deref(),
        Some("ce texte\r\nVersion")
    );
    assert_eq!(extrait(&lu, 0, 6, "\r\n"), Some(assembler(&lu, "\r\n")));
}

#[test]
fn extrait_bornes_fausses() {
    let lu = lu_exemple();
    assert_eq!(extrait(&lu, 3, 2, "\n"), None);
    assert_eq!(extrait(&lu, 0, 7, "\n"), None);
    assert_eq!(extrait(&lu, 7, 7, "\n"), None);
    assert_eq!(extrait(&lu, usize::MAX, usize::MAX, "\n"), None);
    let vide = depuis_reconnu(
        Reconnu {
            langue: String::new(),
            lignes: Vec::new(),
        },
        1.0,
        1,
        1,
    );
    assert_eq!(extrait(&vide, 0, 0, "\n"), None);
}

// ---------- lire ----------

#[test]
fn issue_du_moteur() {
    let rien = Reconnu {
        langue: "en-US".into(),
        lignes: vec![ligne("", &[])],
    };
    assert_eq!(issue(Ok(rien), 1.0, 10, 10), Issue::Vide);
    let blancs = Reconnu {
        langue: "en-US".into(),
        lignes: vec![ligne("  ", &[" ", ""])],
    };
    assert_eq!(issue(Ok(blancs), 1.0, 10, 10), Issue::Vide);
    let un = Reconnu {
        langue: "en-US".into(),
        lignes: vec![ligne("a", &["a"])],
    };
    assert!(matches!(issue(Ok(un), 1.0, 10, 10), Issue::Texte(lu) if lu.mots.len() == 1));
    assert_eq!(
        issue(Err(ErreurMoteur::AucuneLangue), 1.0, 10, 10),
        Issue::AucuneLangue
    );
    assert_eq!(
        issue(Err(ErreurMoteur::Moteur("echeance".into())), 1.0, 10, 10),
        Issue::Echec("echeance".into())
    );
}

#[test]
fn delai_de_lecture_egal_a_l_attente_du_moteur() {
    assert_eq!(DELAI_LECTURE, moteur::ATTENTE_MAX);
}

#[test]
fn lire_prepare_appelle_le_moteur_et_ramene_les_boites() {
    let p = image(100, 20, [255, 255, 255, 128]);
    let issue = lire_avec(&p, Ok(10000), |bgra, l, h| {
        // Le moteur reçoit l'image agrandie ×2, opaque.
        assert_eq!((l, h), (200, 40));
        assert_eq!(bgra.len(), 200 * 40 * 4);
        assert!(bgra.chunks_exact(4).all(|p| p == [255, 255, 255, 255]));
        Ok(Reconnu {
            langue: "en-US".into(),
            lignes: vec![LigneBrute {
                texte: "Bonjour".into(),
                mots: vec![mot("Bonjour", 20.0, 10.0, 100.0, 20.0)],
            }],
        })
    });
    let Issue::Texte(lu) = issue else {
        panic!("texte attendu : {issue:?}");
    };
    assert_eq!((lu.mots[0].x, lu.mots[0].y), (10, 5));
    assert_eq!((lu.mots[0].largeur, lu.mots[0].hauteur), (50, 10));
    assert_eq!(assembler(&lu, "\n"), "Bonjour");
}

#[test]
fn lire_sans_moteur_ou_image_vide() {
    let jamais =
        |_: &[u8], _: u32, _: u32| -> Result<Reconnu, ErreurMoteur> { panic!("moteur appelé") };
    let p = image(10, 10, [0, 0, 0, 255]);
    let sans_moteur = Err(ErreurMoteur::AucuneLangue);
    assert_eq!(lire_avec(&p, sans_moteur, jamais), Issue::AucuneLangue);
    assert_eq!(
        lire_avec(&image(0, 10, [0; 4]), Ok(10000), jamais),
        Issue::Vide
    );
    let refus = lire_avec(&p, Ok(10000), |_, _, _| Err(ErreurMoteur::AucuneLangue));
    assert_eq!(refus, Issue::AucuneLangue);
}

#[test]
fn dimension_illisible_est_un_echec_du_moteur() {
    let jamais =
        |_: &[u8], _: u32, _: u32| -> Result<Reconnu, ErreurMoteur> { panic!("moteur appelé") };
    let p = image(10, 10, [0, 0, 0, 255]);
    let panne = Err(ErreurMoteur::Moteur("dimension 0x80004005".into()));
    assert_eq!(
        lire_avec(&p, panne, jamais),
        Issue::Echec("dimension 0x80004005".into())
    );
    assert_eq!(
        lire_avec(&p, Ok(0), jamais),
        Issue::Echec("dimension 0x00000000".into())
    );
}

#[test]
fn debug_sans_le_texte_lu() {
    let lu = depuis_reconnu(
        Reconnu {
            langue: "fr-FR".into(),
            lignes: vec![
                LigneBrute {
                    texte: "SECRET un".into(),
                    mots: vec![
                        mot("SECRET", 0.0, 0.0, 10.0, 5.0),
                        mot("un", 12.0, 0.0, 4.0, 5.0),
                    ],
                },
                LigneBrute {
                    texte: "SECRET".into(),
                    mots: vec![mot("SECRET", 0.0, 8.0, 10.0, 5.0)],
                },
            ],
        },
        1.0,
        100,
        100,
    );
    let issue = Issue::Texte(lu.clone());
    for ecrit in [
        format!("{lu:?}"),
        format!("{issue:?}"),
        format!("{issue:#?}"),
    ] {
        assert!(!ecrit.contains("SECRET"), "{ecrit}");
        assert!(
            ecrit.contains("mots: 3") && ecrit.contains("lignes: 2"),
            "{ecrit}"
        );
    }
    assert_eq!(
        format!("{:?}", Issue::Echec("echeance".into())),
        "Echec(\"echeance\")"
    );
    assert_eq!(format!("{:?}", Issue::Vide), "Vide");
}
