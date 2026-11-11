//! Tests du module `sortie`.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, Local, TimeDelta, TimeZone, Utc};

use super::annonce::{annonce, Annonce};
use super::nom::{chemin_libre, nom_affiche, nom_fichier};
use super::recents::{
    a_supprimer_apres_ecriture, base_recent, chemin_recent_libre, cle_recent, recents_a_supprimer,
    RECENTS_GARDES,
};
use super::{fichier, png, presse_papiers, sortir, Bilan, Demande, Etape, EtapeFichier};
use crate::systeme::gel::Pixels;

// ---------- Aides ----------

fn instant() -> DateTime<Local> {
    let base = Local
        .with_ymd_and_hms(2026, 11, 11, 19, 40, 12)
        .single()
        .expect("date non ambigue");
    base + TimeDelta::milliseconds(123)
}

fn image(largeur: u32, hauteur: u32) -> Pixels {
    let mut rgba = Vec::with_capacity((largeur * hauteur * 4) as usize);
    for y in 0..hauteur {
        for x in 0..largeur {
            rgba.extend_from_slice(&[
                (x % 256) as u8,
                (y % 256) as u8,
                ((x * 7 + y * 3) % 256) as u8,
                255,
            ]);
        }
    }
    Pixels {
        largeur,
        hauteur,
        rgba,
    }
}

fn decoder(octets: &[u8]) -> Pixels {
    let decodeur = ::png::Decoder::new(std::io::Cursor::new(octets));
    let mut lecteur = decodeur.read_info().expect("entete PNG");
    let taille = lecteur.output_buffer_size().expect("taille");
    let mut tampon = vec![0; taille];
    let info = lecteur.next_frame(&mut tampon).expect("image PNG");
    assert_eq!(info.color_type, ::png::ColorType::Rgba);
    assert_eq!(info.bit_depth, ::png::BitDepth::Eight);
    tampon.truncate(info.buffer_size());
    Pixels {
        largeur: info.width,
        hauteur: info.height,
        rgba: tampon,
    }
}

fn demande(racine: &Path) -> Demande {
    Demande {
        copier: false,
        enregistrer: true,
        dossier_enregistrement: racine.join("Images").join("BreezyShot"),
        dossier_affiche: "Images › BreezyShot".to_string(),
        dossier_recents: racine.join("local").join("Recents"),
        modele_nom: "BreezyShot {date} à {heure}".to_string(),
        maintenant: instant(),
    }
}

fn noms(liste: &[&str]) -> Vec<String> {
    liste.iter().map(|s| s.to_string()).collect()
}

// ---------- Nom de fichier ----------

#[test]
fn nom_modele_par_defaut() {
    assert_eq!(
        nom_fichier("BreezyShot {date} à {heure}", instant()),
        "BreezyShot 2026-11-11 à 19.40.12"
    );
}

#[test]
fn nom_modele_vide_ou_blanc() {
    let attendu = "BreezyShot 2026-11-11 19.40.12";
    assert_eq!(nom_fichier("", instant()), attendu);
    assert_eq!(nom_fichier("   ", instant()), attendu);
    assert_eq!(nom_fichier(" . . ", instant()), attendu);
}

#[test]
fn nom_caracteres_interdits() {
    assert_eq!(
        nom_fichier("a<b>c:d\"e/f\\g|h?i*j\u{0}k\tl\u{7f}m", instant()),
        "a-b-c-d-e-f-g-h-i-j-k-l-m"
    );
    // Pas d'espace ni de point a la fin.
    assert_eq!(nom_fichier("capture. . ", instant()), "capture");
}

#[test]
fn nom_reserves() {
    let attendu = "BreezyShot 2026-11-11 19.40.12";
    for reserve in [
        "CON", "con", "PRN", "AUX", "NUL", "COM1", "com9", "LPT1", "LPT9", "COM¹", "NUL.txt",
        "CON ",
    ] {
        assert_eq!(nom_fichier(reserve, instant()), attendu, "{reserve}");
    }
    // Proches mais permis.
    assert_eq!(nom_fichier("CONSOLE", instant()), "CONSOLE");
    assert_eq!(nom_fichier("COM10", instant()), "COM10");
    assert_eq!(nom_fichier("LPT", instant()), "LPT");
}

#[test]
fn nom_tres_long() {
    let long = "é".repeat(300);
    let nom = nom_fichier(&long, instant());
    // 120 caracteres, puis 200 octets : 100 « é ».
    assert_eq!(nom, "é".repeat(100));
    let nom = nom_fichier(&"a".repeat(300), instant());
    assert_eq!(nom.chars().count(), 120);
    // La coupe ne laisse ni espace ni point a la fin.
    let modele = format!("{}{}", "a".repeat(118), " .xyz");
    let nom = nom_fichier(&modele, instant());
    assert_eq!(nom, "a".repeat(118));
}

#[test]
fn nom_accents_conserves() {
    assert_eq!(
        nom_fichier("Écran été {date} – ça", instant()),
        "Écran été 2026-11-11 – ça"
    );
}

#[test]
fn nom_modele_sans_balise_et_balise_inconnue() {
    assert_eq!(nom_fichier("Capture", instant()), "Capture");
    assert_eq!(nom_fichier("{jour} %Y", instant()), "{jour} %Y");
}

// ---------- Chemin libre ----------

#[test]
fn chemin_libre_ajoute_png_puis_numero() {
    let dossier = Path::new("d");
    assert_eq!(chemin_libre(dossier, "x", |_| false), dossier.join("x.png"));

    let pris = [dossier.join("x.png"), dossier.join("x (2).png")];
    assert_eq!(
        chemin_libre(dossier, "x", |p| pris.iter().any(|q| q == p)),
        dossier.join("x (3).png")
    );
}

#[test]
fn chemin_libre_termine_meme_si_tout_existe() {
    let chemin = chemin_libre(Path::new("d"), "x", |_| true);
    assert!(chemin.to_string_lossy().ends_with(").png"));
}

#[test]
fn nom_affiche_deux_derniers_elements() {
    let dossier = Path::new("/")
        .join("home")
        .join("Images")
        .join("BreezyShot");
    assert_eq!(nom_affiche(&dossier), "Images › BreezyShot");
    assert_eq!(nom_affiche(Path::new("BreezyShot")), "BreezyShot");
}

// ---------- PNG ----------

#[test]
fn png_aller_retour_pixel_par_pixel() {
    for (l, h) in [(1, 1), (3, 2), (1920, 1080)] {
        let entree = image(l, h);
        let octets = super::encoder_png(&entree).expect("encodage");
        assert_eq!(&octets[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(decoder(&octets), entree, "{l}x{h}");
    }
}

#[test]
fn png_alpha_force_a_255() {
    let entree = Pixels {
        largeur: 2,
        hauteur: 1,
        rgba: vec![10, 20, 30, 0, 40, 50, 60, 128],
    };
    let sortie = decoder(&super::encoder_png(&entree).expect("encodage"));
    assert_eq!(sortie.rgba, vec![10, 20, 30, 255, 40, 50, 60, 255]);
}

#[test]
fn png_sans_metadonnees() {
    let octets = super::encoder_png(&image(3, 2)).expect("encodage");
    // Parcours des morceaux : seulement IHDR, IDAT, IEND.
    let mut i = 8;
    let mut types = Vec::new();
    while i + 8 <= octets.len() {
        let longueur = u32::from_be_bytes([octets[i], octets[i + 1], octets[i + 2], octets[i + 3]]);
        types.push(String::from_utf8_lossy(&octets[i + 4..i + 8]).into_owned());
        i += 12 + longueur as usize;
    }
    types.dedup();
    assert_eq!(types, ["IHDR", "IDAT", "IEND"]);
}

#[test]
fn pixels_invalides_refuses_sans_panique() {
    let cas = [
        Pixels {
            largeur: 0,
            hauteur: 5,
            rgba: vec![],
        },
        Pixels {
            largeur: 5,
            hauteur: 0,
            rgba: vec![],
        },
        Pixels {
            largeur: 2,
            hauteur: 2,
            rgba: vec![0; 15],
        },
        Pixels {
            largeur: 2,
            hauteur: 2,
            rgba: vec![0; 17],
        },
        Pixels {
            largeur: u32::MAX,
            hauteur: u32::MAX,
            rgba: vec![0; 4],
        },
    ];
    for pixels in &cas {
        assert_eq!(png::valider(pixels), Err(png::CAUSE_PIXELS.to_string()));
        assert!(super::encoder_png(pixels).is_err());
    }
}

#[test]
fn sortir_pixels_invalides_tout_en_echec() {
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let mut d = demande(racine.path());
    d.copier = true;
    let pixels = Pixels {
        largeur: 4,
        hauteur: 4,
        rgba: vec![1, 2, 3],
    };
    let bilan = sortir(&pixels, &d);
    assert_eq!(bilan.copie, Etape::Echec(png::CAUSE_PIXELS.to_string()));
    assert_eq!(bilan.recent, Err(png::CAUSE_PIXELS.to_string()));
    assert_eq!(
        bilan.enregistrement,
        EtapeFichier::Echec {
            dossier: d.dossier_enregistrement.clone(),
            cause: png::CAUSE_PIXELS.to_string()
        }
    );
    // Rien n'a ete cree sur le disque.
    assert!(!d.dossier_recents.exists());
    assert!(!d.dossier_enregistrement.exists());
}

// ---------- Recents (pur) ----------

#[test]
fn recents_nom_et_collision() {
    // Le nom est en UTC, quel que soit le fuseau de la machine.
    let utc = Utc
        .with_ymd_and_hms(2026, 11, 11, 18, 40, 12)
        .single()
        .expect("date")
        + TimeDelta::milliseconds(123);
    assert_eq!(
        base_recent(utc.with_timezone(&Local)),
        "20261111-184012-123"
    );
    let dossier = Path::new("R");
    assert_eq!(
        chemin_recent_libre(dossier, "20261111-194012-123", |_| false),
        dossier.join("20261111-194012-123.png")
    );
    let pris = dossier.join("20261111-194012-123.png");
    assert_eq!(
        chemin_recent_libre(dossier, "20261111-194012-123", |p| p == pris),
        dossier.join("20261111-194012-123_2.png")
    );
}

#[test]
fn recents_format_exact() {
    for bon in [
        "20261111-194012-123.png",
        "20261111-194012-123_2.png",
        "20261111-194012-123_17.png",
    ] {
        assert!(cle_recent(bon).is_some(), "{bon}");
    }
    for mauvais in [
        "20261111-194012-123.PNG",
        "20261111-194012-12.png",
        "20261111-194012-1234.png",
        "20261111_194012-123.png",
        "2026111a-194012-123.png",
        "20261111-194012-123_1.png",
        "20261111-194012-123_02.png",
        "20261111-194012-123_.png",
        "20261111-194012-123 (2).png",
        "x20261111-194012-123.png",
        "notes.txt",
        "",
        "ééééééééééééééééééééé.png",
    ] {
        assert!(cle_recent(mauvais).is_none(), "{mauvais}");
    }
}

#[test]
fn recents_a_supprimer_ordre_et_format() {
    let liste = noms(&[
        "20261111-194012-123_2.png",
        "20240101-000000-000.png",
        "notes.txt",
        "20261111-194012-123.png",
        "20261111-194012-123_10.png",
        "20250101-000000-000.png",
        "image.png",
    ]);
    // Du plus recent au plus ancien : _10, _2, base, 2025, 2024.
    assert_eq!(
        recents_a_supprimer(&liste, 2),
        noms(&[
            "20261111-194012-123.png",
            "20250101-000000-000.png",
            "20240101-000000-000.png"
        ])
    );
    assert!(recents_a_supprimer(&liste, 5).is_empty());
    assert!(recents_a_supprimer(&liste, RECENTS_GARDES).is_empty());
    assert_eq!(recents_a_supprimer(&liste, 0).len(), 5);
}

#[test]
fn recents_fichier_neuf_protege() {
    // L'horloge a recule : le fichier neuf a le nom le plus ancien.
    let liste = noms(&[
        "20261111-194012-123.png",
        "20261111-194013-000.png",
        "20200101-000000-000.png",
    ]);
    let neuf = "20200101-000000-000.png";
    let a_supprimer = a_supprimer_apres_ecriture(&liste, neuf, 2);
    assert_eq!(a_supprimer, noms(&["20261111-194012-123.png"]));
    assert!(!a_supprimer_apres_ecriture(&liste, neuf, 0)
        .iter()
        .any(|n| n == neuf));
    assert!(!a_supprimer_apres_ecriture(&liste, neuf, 1)
        .iter()
        .any(|n| n == neuf));
}

// ---------- Annonces ----------

fn bilan(copie: Etape, enregistrement: EtapeFichier) -> Bilan {
    Bilan {
        copie,
        recent: Ok(PathBuf::from("r.png")),
        enregistrement,
    }
}

fn demande_annonce() -> Demande {
    demande(&Path::new("/").join("u"))
}

fn reussite(titre: &str) -> Annonce {
    Annonce::Reussite {
        titre: titre.to_string(),
    }
}

fn erreur(titre: &str, cause: &str) -> Annonce {
    Annonce::Erreur {
        titre: titre.to_string(),
        cause: cause.to_string(),
    }
}

fn echec_fichier() -> EtapeFichier {
    EtapeFichier::Echec {
        dossier: PathBuf::from("x"),
        cause: "Le dossier Images › BreezyShot est introuvable.".to_string(),
    }
}

fn fait() -> EtapeFichier {
    EtapeFichier::Fait(PathBuf::from("f.png"))
}

#[test]
fn annonce_copie_seule() {
    let a = annonce(
        &bilan(Etape::Faite, EtapeFichier::NonDemande),
        &demande_annonce(),
    );
    assert_eq!(a, reussite("Copié dans le presse-papiers"));
}

#[test]
fn annonce_enregistrement_seul() {
    let a = annonce(&bilan(Etape::NonDemandee, fait()), &demande_annonce());
    assert_eq!(a, reussite("Enregistré dans Images › BreezyShot"));
}

#[test]
fn annonce_les_deux() {
    let a = annonce(&bilan(Etape::Faite, fait()), &demande_annonce());
    assert_eq!(a, reussite("Copié et enregistré"));
}

#[test]
fn annonce_enregistrement_en_echec() {
    let attendu = erreur(
        "Impossible d'enregistrer",
        "Le dossier Images › BreezyShot est introuvable.",
    );
    for copie in [
        Etape::Faite,
        Etape::NonDemandee,
        Etape::Echec("occupé".to_string()),
    ] {
        let a = annonce(&bilan(copie, echec_fichier()), &demande_annonce());
        assert_eq!(a, attendu);
    }
}

#[test]
fn annonce_copie_en_echec() {
    let cause = "Le presse-papiers est occupé par une autre application.";
    let a = annonce(
        &bilan(Etape::Echec(cause.to_string()), EtapeFichier::NonDemande),
        &demande_annonce(),
    );
    assert_eq!(a, erreur("Impossible de copier", cause));
}

#[test]
fn annonce_seul_recents_en_echec_reste_une_reussite() {
    let mut b = bilan(Etape::Faite, fait());
    b.recent = Err("disque plein".to_string());
    assert_eq!(
        annonce(&b, &demande_annonce()),
        reussite("Copié et enregistré")
    );
    let mut b = bilan(Etape::Faite, EtapeFichier::NonDemande);
    b.recent = Err("disque plein".to_string());
    assert_eq!(
        annonce(&b, &demande_annonce()),
        reussite("Copié dans le presse-papiers")
    );
}

#[test]
fn annonce_cas_hors_contrat() {
    // Copie ratee, fichier enregistre.
    let a = annonce(
        &bilan(Etape::Echec("Occupé.".to_string()), fait()),
        &demande_annonce(),
    );
    assert_eq!(
        a,
        erreur(
            "Impossible de copier",
            "La capture est enregistrée dans Images › BreezyShot."
        )
    );
    // Rien demande.
    let b = bilan(Etape::NonDemandee, EtapeFichier::NonDemande);
    assert_eq!(annonce(&b, &demande_annonce()), reussite("Capture prise"));
    let mut b = b;
    b.recent = Err("Plein.".to_string());
    assert_eq!(
        annonce(&b, &demande_annonce()),
        erreur("Impossible de garder la capture", "Plein.")
    );
}

#[test]
fn annonce_utilise_le_nom_fourni_par_l_application() {
    let mut d = demande_annonce();
    d.dossier_affiche = "Images › BreezyShot".to_string();
    d.dossier_enregistrement = Path::new("/").join("home").join("Pictures").join("BS");
    assert_eq!(
        annonce(&bilan(Etape::NonDemandee, fait()), &d),
        reussite("Enregistré dans Images › BreezyShot")
    );
}

// ---------- Presse-papiers (pur) ----------

/// Horloge factice : `dormir` et chaque essai font avancer le temps.
struct Horloge {
    debut: Instant,
    ecoule: Cell<Duration>,
}

impl Horloge {
    fn new() -> Self {
        Horloge {
            debut: Instant::now(),
            ecoule: Cell::new(Duration::ZERO),
        }
    }
    fn maintenant(&self) -> Instant {
        self.debut + self.ecoule.get()
    }
    fn avancer(&self, d: Duration) {
        self.ecoule.set(self.ecoule.get() + d);
    }
}

#[test]
fn presse_papiers_echeance_reelle_toujours_occupe() {
    // Chaque essai occupe coute ce que coutent les 6 ouvertures d'arboard.
    for cout in [
        Duration::ZERO,
        Duration::from_millis(1),
        presse_papiers::COUT_ESSAI_INTERNE,
    ] {
        let h = Horloge::new();
        let essais = Cell::new(0);
        let r: Result<(), _> = presse_papiers::avec_echeance(
            presse_papiers::ECHEANCE,
            || h.maintenant(),
            |d| h.avancer(d),
            || {
                essais.set(essais.get() + 1);
                h.avancer(cout);
                Err(arboard::Error::ClipboardOccupied)
            },
        );
        assert!(matches!(r, Err(arboard::Error::ClipboardOccupied)));
        assert!(
            h.ecoule.get() <= presse_papiers::ECHEANCE,
            "cout {cout:?} : {:?}",
            h.ecoule.get()
        );
        assert!(essais.get() >= 2, "cout {cout:?} : {} essais", essais.get());
    }
}

#[test]
fn presse_papiers_echeance_s_arrete_si_un_essai_est_long() {
    // Un seul essai depasse deja l'echeance : pas de nouvel essai.
    let h = Horloge::new();
    let essais = Cell::new(0);
    let r: Result<(), _> = presse_papiers::avec_echeance(
        presse_papiers::ECHEANCE,
        || h.maintenant(),
        |d| h.avancer(d),
        || {
            essais.set(essais.get() + 1);
            h.avancer(Duration::from_millis(300));
            Err(arboard::Error::ClipboardOccupied)
        },
    );
    assert!(r.is_err());
    assert_eq!(essais.get(), 1);
}

#[test]
fn presse_papiers_reussite_apres_occupation_et_erreur_definitive() {
    let h = Horloge::new();
    let essais = Cell::new(0);
    let r = presse_papiers::avec_echeance(
        presse_papiers::ECHEANCE,
        || h.maintenant(),
        |d| h.avancer(d),
        || {
            essais.set(essais.get() + 1);
            if essais.get() < 3 {
                Err(arboard::Error::ClipboardOccupied)
            } else {
                Ok(7)
            }
        },
    );
    assert!(matches!(r, Ok(7)));

    // Erreur qui ne vaut pas un nouvel essai : on s'arrete tout de suite.
    let essais = Cell::new(0);
    let r: Result<(), _> = presse_papiers::avec_echeance(
        presse_papiers::ECHEANCE,
        || h.maintenant(),
        |d| h.avancer(d),
        || {
            essais.set(essais.get() + 1);
            Err(arboard::Error::ClipboardNotSupported)
        },
    );
    assert!(r.is_err());
    assert_eq!(essais.get(), 1);
}

#[test]
fn presse_papiers_echeance_avec_la_vraie_horloge() {
    let debut = Instant::now();
    let r: Result<(), _> = presse_papiers::avec_echeance(
        presse_papiers::ECHEANCE,
        Instant::now,
        std::thread::sleep,
        || Err(arboard::Error::ClipboardOccupied),
    );
    assert!(r.is_err());
    let duree = debut.elapsed();
    assert!(duree <= presse_papiers::ECHEANCE, "{duree:?}");
    assert!(duree >= presse_papiers::PAS, "{duree:?}");
}

#[test]
fn presse_papiers_causes_en_francais() {
    assert_eq!(
        presse_papiers::cause(&arboard::Error::ClipboardOccupied),
        "Le presse-papiers est occupé par une autre application."
    );
    assert_eq!(
        presse_papiers::cause(&arboard::Error::Unknown {
            description: "X11 server connection timed out".to_string()
        }),
        "Le presse-papiers a refusé l'image."
    );
}

// ---------- Ecriture (pur) ----------

#[test]
fn suite_apres_echec_du_lien_dur() {
    use super::fichier::{est_tenu, suite_lien, SuiteLien};
    use std::io::ErrorKind;
    assert_eq!(
        suite_lien(ErrorKind::AlreadyExists, Some(183), true),
        SuiteLien::NomSuivant
    );
    assert_eq!(
        suite_lien(ErrorKind::PermissionDenied, None, false),
        SuiteLien::Reessayer
    );
    // Erreur 32 de Windows : fichier tenu par un autre programme.
    assert_eq!(
        suite_lien(ErrorKind::Other, Some(32), true),
        SuiteLien::Reessayer
    );
    // Sous Linux, 32 est une autre erreur.
    assert!(!est_tenu(ErrorKind::Other, Some(32), false));
    assert_eq!(
        suite_lien(ErrorKind::Unsupported, Some(50), true),
        SuiteLien::Repli
    );
    assert_eq!(
        suite_lien(ErrorKind::Other, Some(1), true),
        SuiteLien::Repli
    );
}

#[test]
fn temporaires_perimes_format_exact() {
    use super::fichier::{est_nom_temporaire, temporaire_perime, AGE_TEMPORAIRE_PERIME};
    let vieux = Some(AGE_TEMPORAIRE_PERIME + Duration::from_secs(1));
    let jeune = Some(Duration::from_secs(10));
    assert!(est_nom_temporaire(".breezyshot-123-456789-0.tmp"));
    assert!(temporaire_perime(".breezyshot-123-456789-0.tmp", vieux));
    assert!(!temporaire_perime(".breezyshot-123-456789-0.tmp", jeune));
    assert!(!temporaire_perime(".breezyshot-123-456789-0.tmp", None));
    for autre in [
        ".breezyshot-123-456.tmp",
        ".breezyshot-1-2-3-4.tmp",
        ".breezyshot-a-2-3.tmp",
        ".breezyshot--2-3.tmp",
        "breezyshot-1-2-3.tmp",
        ".breezyshot-1-2-3.TMP",
        ".breezyshot-1-2-3.tmp.png",
        "notes.tmp",
    ] {
        assert!(!temporaire_perime(autre, vieux), "{autre}");
    }
}

#[test]
fn dossiers_utilisables() {
    use super::fichier::dossier_utilisable;
    assert!(!dossier_utilisable(&PathBuf::new()));
    assert!(!dossier_utilisable(Path::new("Images")));
    assert!(!dossier_utilisable(Path::new("./Images")));
    assert!(dossier_utilisable(&std::env::temp_dir()));
}

// ---------- Disque ----------

fn noms_du_dossier(dossier: &Path) -> Vec<String> {
    fs::read_dir(dossier)
        .expect("liste")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect()
}

fn sans_temporaire(dossier: &Path) {
    let restes: Vec<_> = noms_du_dossier(dossier)
        .into_iter()
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert!(restes.is_empty(), "{restes:?}");
}

#[test]
fn recents_ecrit_et_relu() {
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let mut d = demande(racine.path());
    d.enregistrer = false;
    let entree = image(5, 3);
    let base = base_recent(instant());
    let bilan = sortir(&entree, &d);
    let chemin = bilan.recent.expect("recent ecrit");
    assert_eq!(chemin, d.dossier_recents.join(format!("{base}.png")));
    assert_eq!(decoder(&fs::read(&chemin).expect("lecture")), entree);
    assert_eq!(bilan.copie, Etape::NonDemandee);
    assert_eq!(bilan.enregistrement, EtapeFichier::NonDemande);

    // Meme instant : collision en « _2 ».
    let bilan = sortir(&entree, &d);
    assert_eq!(
        bilan.recent,
        Ok(d.dossier_recents.join(format!("{base}_2.png")))
    );
    sans_temporaire(&d.dossier_recents);
}

#[test]
fn recents_nettoyage_a_50() {
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let mut d = demande(racine.path());
    d.enregistrer = false;
    fs::create_dir_all(&d.dossier_recents).expect("dossier");
    for i in 0..60 {
        let nom = format!("20250101-0000{:02}-000.png", i);
        fs::write(d.dossier_recents.join(nom), b"ancien").expect("ecriture");
    }
    let etrangers = ["notes.txt", "20250101-000000-000.PNG", "photo.png"];
    for nom in etrangers {
        fs::write(d.dossier_recents.join(nom), b"autre").expect("ecriture");
    }

    let bilan = sortir(&image(2, 2), &d);
    let neuf = bilan.recent.expect("recent ecrit");
    assert!(neuf.exists());

    let restants = noms_du_dossier(&d.dossier_recents);
    let au_format = restants.iter().filter(|n| cle_recent(n).is_some()).count();
    assert_eq!(au_format, RECENTS_GARDES);
    for nom in etrangers {
        assert!(restants.iter().any(|n| n == nom), "{nom} supprime");
    }
    // Les plus anciens sont partis.
    assert!(!d.dossier_recents.join("20250101-000000-000.png").exists());
    assert!(d.dossier_recents.join("20250101-000059-000.png").exists());
}

#[test]
fn enregistrement_cree_le_dossier() {
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let d = demande(racine.path());
    assert!(!d.dossier_enregistrement.exists());
    let entree = image(4, 4);
    let bilan = sortir(&entree, &d);
    let attendu = d
        .dossier_enregistrement
        .join("BreezyShot 2026-11-11 à 19.40.12.png");
    assert_eq!(bilan.enregistrement, EtapeFichier::Fait(attendu.clone()));
    assert_eq!(decoder(&fs::read(&attendu).expect("lecture")), entree);
    assert_eq!(
        annonce(&bilan, &d),
        reussite("Enregistré dans Images › BreezyShot")
    );
    sans_temporaire(&d.dossier_enregistrement);
}

#[test]
fn enregistrement_dossier_impossible_a_creer() {
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let fichier_bloquant = racine.path().join("bloque");
    fs::write(&fichier_bloquant, b"x").expect("ecriture");
    let mut d = demande(racine.path());
    d.dossier_enregistrement = fichier_bloquant.join("BreezyShot");

    let bilan = sortir(&image(2, 2), &d);
    assert_eq!(
        bilan.enregistrement,
        EtapeFichier::Echec {
            dossier: d.dossier_enregistrement.clone(),
            cause: "Le dossier Images › BreezyShot est introuvable.".to_string()
        }
    );
    // Les autres etapes ont continue.
    assert!(bilan.recent.is_ok());
    assert_eq!(
        annonce(&bilan, &d),
        erreur(
            "Impossible d'enregistrer",
            "Le dossier Images › BreezyShot est introuvable."
        )
    );
}

#[test]
fn enregistrement_dossier_qui_est_un_fichier() {
    // Repris du relecteur : le fichier tiers reste intact, cause en clair.
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let mut d = demande(racine.path());
    let f = racine.path().join("Images");
    fs::write(&f, b"x").expect("ecriture");
    d.dossier_enregistrement = f.clone();
    let bilan = sortir(&image(2, 2), &d);
    assert_eq!(
        bilan.enregistrement,
        EtapeFichier::Echec {
            dossier: f.clone(),
            cause: "Le dossier Images › BreezyShot est introuvable.".to_string()
        }
    );
    assert_eq!(fs::read(&f).expect("lecture"), b"x");
}

#[test]
fn dossier_vide_ou_relatif_refuse_sans_rien_ecrire() {
    // Repris du relecteur : un chemin vide ecrivait dans le dossier courant.
    let courant = std::env::current_dir().expect("dossier courant");
    let avant = noms_du_dossier(&courant);
    let racine = tempfile::tempdir().expect("dossier temporaire");
    for dossier in [
        PathBuf::new(),
        PathBuf::from("relatif-breezyshot-test").join("BreezyShot"),
    ] {
        let mut d = demande(racine.path());
        d.dossier_enregistrement = dossier.clone();
        let bilan = sortir(&image(2, 2), &d);
        assert_eq!(
            bilan.enregistrement,
            EtapeFichier::Echec {
                dossier: dossier.clone(),
                cause: "Le dossier Images › BreezyShot est introuvable.".to_string()
            }
        );
        assert!(bilan.recent.is_ok());

        let mut d = demande(racine.path());
        d.enregistrer = false;
        d.dossier_recents = dossier.clone();
        let bilan = sortir(&image(2, 2), &d);
        let cause = bilan.recent.expect_err("recents refuse");
        assert!(cause.ends_with("est introuvable."), "{cause}");
    }
    assert_eq!(noms_du_dossier(&courant), avant);
    assert!(!courant.join("relatif-breezyshot-test").exists());
}

#[test]
fn enregistrement_n_ecrase_jamais() {
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let d = demande(racine.path());
    fs::create_dir_all(&d.dossier_enregistrement).expect("dossier");
    let existant = d
        .dossier_enregistrement
        .join("BreezyShot 2026-11-11 à 19.40.12.png");
    fs::write(&existant, b"ancien").expect("ecriture");

    let bilan = sortir(&image(2, 2), &d);
    let attendu = d
        .dossier_enregistrement
        .join("BreezyShot 2026-11-11 à 19.40.12 (2).png");
    assert_eq!(bilan.enregistrement, EtapeFichier::Fait(attendu));
    assert_eq!(fs::read(&existant).expect("lecture"), b"ancien");
    sans_temporaire(&d.dossier_enregistrement);
}

#[test]
fn ecriture_refuse_d_ecraser_meme_si_le_choix_se_trompe() {
    // `choisir` designe toujours un fichier existant : on echoue sans ecraser.
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let existant = racine.path().join("pris.png");
    fs::write(&existant, b"ancien").expect("ecriture");
    let resultat =
        fichier::ecrire_sans_ecraser(racine.path(), "Test", b"neuf", |d| d.join("pris.png"));
    assert_eq!(
        resultat,
        Err("Le fichier n'a pas pu être écrit dans le dossier Test.".to_string())
    );
    assert_eq!(fs::read(&existant).expect("lecture"), b"ancien");
    sans_temporaire(racine.path());
}

#[test]
fn nom_tres_long_en_octets_enregistre_reellement() {
    // Repris du relecteur : 120 emoji = 480 octets, refuse par ext4 et APFS.
    for (modele, attendu) in [
        ("😀".repeat(300), "😀".repeat(50)),
        ("€".repeat(120), "€".repeat(66)),
        (
            format!("{}é", "a".repeat(119)),
            format!("{}é", "a".repeat(119)),
        ),
    ] {
        let nom = nom_fichier(&modele, instant());
        assert_eq!(nom, attendu);
        assert!(nom.len() <= 200);

        let racine = tempfile::tempdir().expect("dossier temporaire");
        let mut d = demande(racine.path());
        d.modele_nom = modele;
        let bilan = sortir(&image(2, 2), &d);
        let chemin = d.dossier_enregistrement.join(format!("{attendu}.png"));
        assert_eq!(bilan.enregistrement, EtapeFichier::Fait(chemin.clone()));
        assert!(chemin.exists());
        // Une collision reste possible : « (2) » tient encore.
        let bilan = sortir(&image(2, 2), &d);
        assert_eq!(
            bilan.enregistrement,
            EtapeFichier::Fait(d.dossier_enregistrement.join(format!("{attendu} (2).png")))
        );
    }
    // Coupe a 200 octets puis retrait des espaces et points de fin.
    let modele = format!("{}  .. b", "é".repeat(99));
    assert_eq!(nom_fichier(&modele, instant()), "é".repeat(99));
}

#[test]
fn sorties_concurrentes_meme_milliseconde() {
    // Repris du relecteur : 16 sorties au meme instant.
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let d = demande(racine.path());
    let fils: Vec<_> = (0..16u8)
        .map(|i| {
            let d = d.clone();
            std::thread::spawn(move || {
                let mut px = image(3, 3);
                px.rgba[0] = i;
                (i, sortir(&px, &d))
            })
        })
        .collect();
    let mut enregistres = std::collections::HashSet::new();
    let mut recents = std::collections::HashSet::new();
    for fil in fils {
        let (i, b) = fil.join().expect("fil");
        let EtapeFichier::Fait(p) = &b.enregistrement else {
            panic!("{i}: {:?}", b.enregistrement)
        };
        assert_eq!(decoder(&fs::read(p).expect("lecture")).rgba[0], i);
        assert!(enregistres.insert(p.clone()));
        let r = b.recent.clone().expect("recent");
        assert_eq!(decoder(&fs::read(&r).expect("lecture")).rgba[0], i);
        assert!(recents.insert(r));
    }
    sans_temporaire(&d.dossier_recents);
    sans_temporaire(&d.dossier_enregistrement);
}

#[test]
fn nettoyage_de_nos_temporaires_oublies() {
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let d = demande(racine.path());
    fs::create_dir_all(&d.dossier_enregistrement).expect("dossier");
    let ancien = SystemTime::now() - Duration::from_secs(2 * 3600);
    let creer = |nom: &str, date: Option<SystemTime>| {
        let chemin = d.dossier_enregistrement.join(nom);
        fs::write(&chemin, b"x").expect("ecriture");
        if let Some(date) = date {
            fs::File::options()
                .write(true)
                .open(&chemin)
                .expect("ouverture")
                .set_modified(date)
                .expect("date");
        }
    };
    creer(".breezyshot-1-2-3.tmp", Some(ancien));
    creer(".breezyshot-4-5-6.tmp", None);
    creer(".breezyshot-x.tmp", Some(ancien));
    creer("vacances.tmp", Some(ancien));

    let bilan = sortir(&image(2, 2), &d);
    assert!(matches!(bilan.enregistrement, EtapeFichier::Fait(_)));
    let restants = noms_du_dossier(&d.dossier_enregistrement);
    assert!(!restants.iter().any(|n| n == ".breezyshot-1-2-3.tmp"));
    for garde in [".breezyshot-4-5-6.tmp", ".breezyshot-x.tmp", "vacances.tmp"] {
        assert!(restants.iter().any(|n| n == garde), "{garde} supprime");
    }
}

// ---------- Presse-papiers (reel) ----------

/// Un seul test a la fois touche au vrai presse-papiers.
static PRESSE_PAPIERS_TESTS: Mutex<()> = Mutex::new(());

/// Vrai si un presse-papiers existe. Sans lui : saute, ou echec si
/// `BREEZYSHOT_PRESSE_PAPIERS_OBLIGATOIRE=1`.
fn presse_papiers_present(test: &str) -> bool {
    match arboard::Clipboard::new() {
        Ok(_) => true,
        Err(e) => {
            let obligatoire =
                std::env::var("BREEZYSHOT_PRESSE_PAPIERS_OBLIGATOIRE").as_deref() == Ok("1");
            assert!(
                !obligatoire,
                "{test} : presse-papiers obligatoire mais absent ({e})"
            );
            println!("{test} : pas de presse-papiers ici ({e}), saute.");
            false
        }
    }
}

#[test]
fn presse_papiers_pose_et_relit_quatre_carres() {
    let _seul = PRESSE_PAPIERS_TESTS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if !presse_papiers_present("presse_papiers_pose_et_relit_quatre_carres") {
        return;
    }
    // 8x8 : rouge, vert / bleu, jaune ; alphas varies a l'entree.
    let (l, h) = (8u32, 8u32);
    let mut entree = Vec::new();
    let mut attendu = Vec::new();
    for y in 0..h {
        for x in 0..l {
            let couleur = match (x < l / 2, y < h / 2) {
                (true, true) => [255, 0, 0],
                (false, true) => [0, 255, 0],
                (true, false) => [0, 0, 255],
                (false, false) => [255, 255, 0],
            };
            let alpha = [0u8, 1, 128, 254][((x + y) % 4) as usize];
            entree.extend_from_slice(&couleur);
            entree.push(alpha);
            attendu.extend_from_slice(&couleur);
            attendu.push(255);
        }
    }
    let pixels = Pixels {
        largeur: l,
        hauteur: h,
        rgba: entree,
    };
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let mut d = demande(racine.path());
    d.copier = true;
    d.enregistrer = false;
    let bilan = sortir(&pixels, &d);
    assert_eq!(bilan.copie, Etape::Faite);

    // Relu dans le meme processus (sous X11, c'est lui qui sert l'image),
    // en reessayant pendant 1 s.
    let debut = Instant::now();
    let relu = loop {
        let essai = arboard::Clipboard::new().and_then(|mut c| c.get_image());
        match essai {
            Ok(image) => break image,
            Err(e) if debut.elapsed() >= Duration::from_secs(1) => {
                panic!("image non relue en 1 s : {e}")
            }
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    assert_eq!((relu.width, relu.height), (l as usize, h as usize));
    assert_eq!(relu.bytes.as_ref(), attendu.as_slice());
    println!("Presse-papiers : image 8x8 posee et relue pixel par pixel, alpha a 255.");
}

// ---------- Mesures ----------

/// Image de type ecran : aplats clairs, bandes de « texte ».
fn image_ecran(l: u32, h: u32) -> Pixels {
    let mut pixels = image(l, h);
    for (i, p) in pixels.rgba.chunks_exact_mut(4).enumerate() {
        let x = (i as u32) % l;
        let y = (i as u32) / l;
        if !(y / 40).is_multiple_of(3) || x % 9 > 5 {
            p.copy_from_slice(&[240, 240, 240, 255]);
        }
    }
    pixels
}

#[test]
fn mesure_sortir_avec_copie() {
    // Sans seuil : la duree est affichee pour comparer les machines.
    let _seul = PRESSE_PAPIERS_TESTS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let copie = presse_papiers_present("mesure_sortir_avec_copie");
    let profil = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    for (l, h) in [(1920, 1080), (3840, 2160)] {
        let pixels = image_ecran(l, h);
        let racine = tempfile::tempdir().expect("dossier temporaire");
        let mut d = demande(racine.path());
        d.copier = copie;
        let debut = Instant::now();
        let bilan = sortir(&pixels, &d);
        let duree = debut.elapsed();
        assert!(bilan.recent.is_ok());
        println!(
            "Mesure ({profil}) sortir {l}x{h} {} + Recents + enregistrement : {:.0} ms ({:?})",
            if copie {
                "avec copie"
            } else {
                "SANS copie (pas de presse-papiers)"
            },
            duree.as_secs_f64() * 1000.0,
            bilan.copie
        );
    }
}

#[test]
#[ignore = "mesure de vitesse, a lancer en --release"]
fn mesure_encodage_png() {
    for (l, h) in [(1920, 1080), (3840, 2160)] {
        let pixels = image_ecran(l, h);
        let mut meilleur = f64::MAX;
        let mut taille = 0;
        for _ in 0..5 {
            let debut = Instant::now();
            let octets = super::encoder_png(&pixels).expect("encodage");
            meilleur = meilleur.min(debut.elapsed().as_secs_f64() * 1000.0);
            taille = octets.len();
        }
        println!(
            "PNG {l}x{h} : {meilleur:.1} ms (meilleur de 5), {} Ko",
            taille / 1024
        );
    }
}
