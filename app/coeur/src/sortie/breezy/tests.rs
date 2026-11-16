//! Le fichier .breezy : aller-retour, archive faite par un autre outil, et fichiers hostiles.

use super::*;

fn image(l: u32, h: u32, alpha: u8) -> Pixels {
    let mut rgba = Vec::with_capacity((l * h * 4) as usize);
    for y in 0..h {
        for x in 0..l {
            rgba.extend_from_slice(&[(x * 7) as u8, (y * 11) as u8, 200, alpha]);
        }
    }
    Pixels {
        largeur: l,
        hauteur: h,
        rgba,
    }
}

const ANNOT: &str = r#"{"largeur":3,"hauteur":2,"objets":[],"recadrage":null,"suivant":1}"#;

fn fichier() -> Vec<u8> {
    ecrire(
        &image(3, 2, 255),
        false,
        ANNOT.as_bytes(),
        "0.7.5",
        "2026-11-16T10:00:00+01:00",
    )
    .expect("ecrire")
}

#[test]
fn aller_retour_garde_les_pixels_et_les_annotations_tels_quels() {
    let p = image(5, 4, 255);
    let octets = ecrire(&p, false, ANNOT.as_bytes(), "0.7.5", "x").unwrap();
    let lu = lire(&octets).unwrap();
    assert_eq!(lu.pixels, p);
    assert_eq!(lu.annotations, ANNOT);
    assert!(!lu.transparence);
}

#[test]
fn la_transparence_est_gardee_et_sinon_l_alpha_est_force() {
    let mut p = image(4, 4, 255);
    p.rgba[3] = 10;
    let avec = lire(&ecrire(&p, true, b"{}", "v", "c").unwrap()).unwrap();
    assert!(avec.transparence);
    assert_eq!(avec.pixels.rgba[3], 10);
    let sans = lire(&ecrire(&p, false, b"{}", "v", "c").unwrap()).unwrap();
    assert!(!sans.transparence);
    assert_eq!(sans.pixels.rgba[3], 255);
}

#[test]
fn le_fichier_est_une_archive_zip_reconnue_par_un_autre_outil() {
    // CRC-32 du ZIP : la valeur connue de « 123456789 ».
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    let octets = fichier();
    assert_eq!(&octets[..4], b"PK\x03\x04");
    assert_eq!(&octets[octets.len() - 22..octets.len() - 18], b"PK\x05\x06");
}

#[test]
fn une_archive_faite_par_python_se_lit_et_les_champs_inconnus_sont_ignores() {
    let lu = lire(include_bytes!("python-stocke.zip")).unwrap();
    assert_eq!((lu.pixels.largeur, lu.pixels.hauteur), (2, 1));
    assert_eq!(lu.pixels.rgba, [255, 0, 0, 255, 0, 0, 255, 128]);
    assert!(lu.transparence);
}

#[test]
fn une_version_plus_recente_a_sa_propre_cause() {
    let neuf = remanifeste(&fichier(), |m| {
        m["version"] = serde_json::json!(VERSION_FORMAT + 1)
    });
    assert_eq!(lire(&neuf), Err(ErreurBreezy::PlusRecent));
}

#[test]
fn manifeste_faux_ou_incoherent_est_refuse() {
    let faux = remanifeste(&fichier(), |m| m["format"] = serde_json::json!("autre"));
    assert_eq!(lire(&faux), Err(ErreurBreezy::Abime));
    let zero = remanifeste(&fichier(), |m| m["version"] = serde_json::json!(0));
    assert_eq!(lire(&zero), Err(ErreurBreezy::Abime));
    let taille = remanifeste(&fichier(), |m| m["largeur"] = serde_json::json!(99));
    assert_eq!(lire(&taille), Err(ErreurBreezy::Abime));
}

/// Refait l'archive avec un manifeste changé (mêmes autres entrées).
fn remanifeste(octets: &[u8], change: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
    let entrees = zip_lire(octets).unwrap();
    let get = |n: &str| entrees.iter().find(|(x, _)| x == n).unwrap().1;
    let mut m: serde_json::Value = serde_json::from_slice(get(NOM_MANIFESTE)).unwrap();
    change(&mut m);
    let m = m.to_string();
    zip_ecrire(&[
        (NOM_MANIFESTE, m.as_bytes()),
        (NOM_IMAGE, get(NOM_IMAGE)),
        (NOM_ANNOTATIONS, get(NOM_ANNOTATIONS)),
    ])
    .unwrap()
}

#[test]
fn entree_absente_ou_en_double_est_refusee() {
    let brut = fichier();
    let e = zip_lire(&brut).unwrap();
    let get = |n: &str| e.iter().find(|(x, _)| x == n).unwrap().1.to_vec();
    let (m, i, a) = (get(NOM_MANIFESTE), get(NOM_IMAGE), get(NOM_ANNOTATIONS));
    let sans_annotations = zip_ecrire(&[(NOM_MANIFESTE, &m), (NOM_IMAGE, &i)]).unwrap();
    assert_eq!(lire(&sans_annotations), Err(ErreurBreezy::Abime));
    let double = zip_ecrire(&[
        (NOM_MANIFESTE, &m),
        (NOM_IMAGE, &i),
        (NOM_ANNOTATIONS, &a),
        (NOM_IMAGE, &i),
    ])
    .unwrap();
    assert_eq!(lire(&double), Err(ErreurBreezy::Abime));
    // Une entrée en plus (inconnue) ne gêne pas.
    let plus = zip_ecrire(&[
        (NOM_MANIFESTE, &m),
        ("apercu.png", b"x"),
        (NOM_IMAGE, &i),
        (NOM_ANNOTATIONS, &a),
    ])
    .unwrap();
    assert!(lire(&plus).is_ok());
}

#[test]
fn annotations_qui_ne_sont_pas_un_objet_json_sont_refusees() {
    let brut = fichier();
    let e = zip_lire(&brut).unwrap();
    let get = |n: &str| e.iter().find(|(x, _)| x == n).unwrap().1.to_vec();
    for mauvais in [&b"[]"[..], b"12", b"{", b"", b"\xff\xfe"] {
        let z = zip_ecrire(&[
            (NOM_MANIFESTE, &get(NOM_MANIFESTE)),
            (NOM_IMAGE, &get(NOM_IMAGE)),
            (NOM_ANNOTATIONS, mauvais),
        ])
        .unwrap();
        assert_eq!(lire(&z), Err(ErreurBreezy::Abime), "{mauvais:?}");
    }
}

#[test]
fn tronque_abime_ou_vide_ne_panique_jamais() {
    let bon = fichier();
    // Tous les préfixes : une erreur, jamais une panique.
    for n in 0..bon.len() {
        assert!(lire(&bon[..n]).is_err(), "prefixe {n}");
    }
    // Un octet changé, partout : une erreur ou, si l'octet ne compte pas, la même lecture.
    for i in 0..bon.len() {
        let mut m = bon.clone();
        m[i] ^= 0xA5;
        let _ = lire(&m);
    }
    assert!(lire(&[]).is_err());
    assert!(lire(b"PK\x05\x06").is_err());
}

#[test]
fn octets_changes_dans_les_donnees_sont_vus_par_le_crc() {
    let mut m = fichier();
    // Le premier octet de données du manifeste (après l'en-tête local de 30 + nom).
    m[30 + NOM_MANIFESTE.len()] ^= 1;
    assert_eq!(lire(&m), Err(ErreurBreezy::Abime));
}

#[test]
fn entree_compressee_ou_chiffree_est_refusee() {
    let mut m = fichier();
    // Méthode 8 (deflate) dans l'en-tête central de la première entrée.
    let centrale = u32_a(&m, m.len() - 22 + 16).unwrap() as usize;
    m[centrale + 10] = 8;
    assert_eq!(lire(&m), Err(ErreurBreezy::Abime));
    let mut c = fichier();
    let centrale = u32_a(&c, c.len() - 22 + 16).unwrap() as usize;
    c[centrale + 8] |= 1;
    assert_eq!(lire(&c), Err(ErreurBreezy::Abime));
}

#[test]
fn decalage_ou_taille_hors_de_l_archive_est_refuse() {
    let mut m = fichier();
    let centrale = u32_a(&m, m.len() - 22 + 16).unwrap() as usize;
    // Décalage de la première entrée : au-delà de la fin.
    m[centrale + 42..centrale + 46].copy_from_slice(&0xFFFF_FF00u32.to_le_bytes());
    assert_eq!(lire(&m), Err(ErreurBreezy::Abime));
    let mut t = fichier();
    let centrale = u32_a(&t, t.len() - 22 + 16).unwrap() as usize;
    // Taille énorme (compressée = brute, donc cohérente) : jamais allouée, refusée.
    t[centrale + 20..centrale + 24].copy_from_slice(&0xFFFF_0000u32.to_le_bytes());
    t[centrale + 24..centrale + 28].copy_from_slice(&0xFFFF_0000u32.to_le_bytes());
    assert_eq!(lire(&t), Err(ErreurBreezy::Abime));
}

#[test]
fn une_image_qui_n_est_pas_du_rgba_8_bits_ou_trop_grande_est_refusee() {
    let brut = fichier();
    let e = zip_lire(&brut).unwrap();
    let get = |n: &str| e.iter().find(|(x, _)| x == n).unwrap().1.to_vec();
    // PNG en niveaux de gris 8 bits.
    let mut gris = Vec::new();
    {
        let mut enc = ::png::Encoder::new(&mut gris, 2, 2);
        enc.set_color(::png::ColorType::Grayscale);
        enc.set_depth(::png::BitDepth::Eight);
        let mut w = enc.write_header().unwrap();
        w.write_image_data(&[1, 2, 3, 4]).unwrap();
        w.finish().unwrap();
    }
    let z = zip_ecrire(&[
        (NOM_MANIFESTE, &get(NOM_MANIFESTE)),
        (NOM_IMAGE, &gris),
        (NOM_ANNOTATIONS, &get(NOM_ANNOTATIONS)),
    ])
    .unwrap();
    assert_eq!(lire(&z), Err(ErreurBreezy::Abime));
    // En-tête PNG qui annonce 30 000 × 30 000 : refusé avant toute allocation.
    let mut gros = Vec::new();
    {
        let mut enc = ::png::Encoder::new(&mut gros, 30_000, 1);
        enc.set_color(::png::ColorType::Rgba);
        enc.set_depth(::png::BitDepth::Eight);
        let mut w = enc.write_header().unwrap();
        w.write_image_data(&vec![0u8; 30_000 * 4]).unwrap();
        w.finish().unwrap();
    }
    let z = zip_ecrire(&[
        (NOM_MANIFESTE, &get(NOM_MANIFESTE)),
        (NOM_IMAGE, &gros),
        (NOM_ANNOTATIONS, &get(NOM_ANNOTATIONS)),
    ])
    .unwrap();
    assert_eq!(lire(&z), Err(ErreurBreezy::TropGros));
}

#[test]
fn on_n_ecrit_pas_une_image_vide_ni_trop_grande() {
    let vide = Pixels {
        largeur: 0,
        hauteur: 0,
        rgba: vec![],
    };
    assert!(ecrire(&vide, false, b"{}", "v", "c").is_err());
    assert!(!taille_permise(COTE_MAX + 1, 1));
    assert!(!taille_permise(10_000, 10_000));
    assert!(taille_permise(5_000, 5_000));
}

#[test]
fn des_annotations_trop_grosses_ne_sont_pas_ecrites() {
    let grosses = vec![b' '; ANNOTATIONS_OCTETS_MAX + 1];
    assert!(ecrire(&image(2, 2, 255), false, &grosses, "v", "c").is_err());
}

#[test]
fn les_causes_ne_contiennent_jamais_de_chemin() {
    for e in [
        ErreurBreezy::Abime,
        ErreurBreezy::PlusRecent,
        ErreurBreezy::TropGros,
    ] {
        assert!(!e.code().contains('/') && !e.code().contains('\\'));
    }
}

#[test]
fn une_entree_inconnue_n_est_jamais_lue_meme_abimee() {
    let brut = fichier();
    let e = zip_lire(&brut).unwrap();
    let get = |n: &str| e.iter().find(|(x, _)| x == n).unwrap().1.to_vec();
    let z = zip_ecrire(&[
        (NOM_MANIFESTE, &get(NOM_MANIFESTE)),
        ("zzz.bin", b"donnees"),
        (NOM_IMAGE, &get(NOM_IMAGE)),
        (NOM_ANNOTATIONS, &get(NOM_ANNOTATIONS)),
    ])
    .unwrap();
    // Le CRC de l'entrée inconnue est faux, et même sa méthode : elle est ignorée, le reste se lit.
    let mut abime = z.clone();
    let centrale = u32_a(&abime, abime.len() - 22 + 16).unwrap() as usize;
    let mut p = centrale;
    // Deuxième entrée du répertoire central : on saute la première (46 + nom).
    p += 46 + NOM_MANIFESTE.len();
    abime[p + 10] = 8; // méthode « compressée »
    abime[p + 16] ^= 0xFF; // CRC faux
    assert!(lire(&abime).is_ok());
}
