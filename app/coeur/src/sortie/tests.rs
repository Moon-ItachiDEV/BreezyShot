//! Tests du module `sortie`.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, Local, TimeDelta, TimeZone, Utc};

use super::annonce::{annonce, Annonce};
use super::nom::{chemin_libre, nom_affiche, nom_fichier};
use super::recents::{
    a_supprimer_apres_ecriture, base_recent, chemin_recent_libre, cle_recent, recents_a_supprimer,
    RECENTS_GARDES,
};
use super::{fichier, png, presse_papiers, sortir, sur_blanc, Bilan, Demande, Etape, EtapeFichier};
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
        ranger_recents: true,
        modele_nom: "BreezyShot {date} à {heure}".to_string(),
        maintenant: instant(),
        transparence: false,
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
    assert_eq!(bilan.recent, Some(Err(png::CAUSE_PIXELS.to_string())));
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
        recent: Some(Ok(PathBuf::from("r.png"))),
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
    b.recent = Some(Err("disque plein".to_string()));
    assert_eq!(
        annonce(&b, &demande_annonce()),
        reussite("Copié et enregistré")
    );
    let mut b = bilan(Etape::Faite, EtapeFichier::NonDemande);
    b.recent = Some(Err("disque plein".to_string()));
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
    b.recent = Some(Err("Plein.".to_string()));
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
    // Aucune borne sur la duree totale : une machine chargee allonge les
    // pauses. On verifie ce que le code garantit : aucun essai ne demarre
    // apres l'echeance moins le cout reserve, d'apres l'horloge que lit le
    // code juste avant de decider. La borne stricte est prouvee par les
    // tests a horloge injectee.
    let lectures = std::cell::RefCell::new(Vec::<Instant>::new());
    let horloge = || {
        let t = Instant::now();
        lectures.borrow_mut().push(t);
        t
    };
    let departs = std::cell::RefCell::new(Vec::<Instant>::new());
    let r: Result<(), _> = presse_papiers::avec_echeance(
        presse_papiers::ECHEANCE,
        horloge,
        std::thread::sleep,
        || {
            let derniere = *lectures.borrow().last().expect("lecture de l'horloge");
            departs.borrow_mut().push(derniere);
            Err(arboard::Error::ClipboardOccupied)
        },
    );
    assert!(matches!(r, Err(arboard::Error::ClipboardOccupied)));
    let debut = lectures.borrow()[0];
    let departs = departs.into_inner();
    assert!(departs.len() >= 2, "{} essais", departs.len());
    let limite = presse_papiers::ECHEANCE - presse_papiers::COUT_ESSAI_INTERNE;
    for depart in &departs {
        let ecart = depart.saturating_duration_since(debut);
        assert!(ecart <= limite, "essai lance a {ecart:?}");
    }
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
        suite_lien(ErrorKind::AlreadyExists, Some(183)),
        SuiteLien::NomSuivant
    );
    assert_eq!(
        suite_lien(ErrorKind::PermissionDenied, None),
        SuiteLien::Reessayer
    );
    // Erreur 32 de Windows : fichier tenu par un autre programme.
    assert_eq!(suite_lien(ErrorKind::Other, Some(32)), SuiteLien::Reessayer);
    // Le meme test vaut partout (sous Unix, 32 est EPIPE, jamais rendu ici).
    assert!(est_tenu(ErrorKind::Other, Some(32)));
    assert!(!est_tenu(ErrorKind::Other, Some(5)));
    assert_eq!(
        suite_lien(ErrorKind::Unsupported, Some(50)),
        SuiteLien::Repli
    );
    assert_eq!(suite_lien(ErrorKind::Other, Some(1)), SuiteLien::Repli);
}

/// Compte les liens tentes et les attentes d'un appel a `placer_avec`.
fn essayer_placer(
    erreur: impl Fn(usize) -> Option<std::io::Error>,
) -> (std::io::Result<PathBuf>, Vec<PathBuf>, Vec<Duration>, bool) {
    use super::fichier::placer_avec;
    let liens = std::cell::RefCell::new(Vec::new());
    let mut attentes = Vec::new();
    let repli_fait = Cell::new(false);
    let pris = Cell::new(0);
    let choisir = |d: &Path| d.join(format!("x{}.png", pris.get()));
    let r = placer_avec(
        Path::new("d"),
        &choisir,
        |cible| {
            liens.borrow_mut().push(cible.to_path_buf());
            match erreur(liens.borrow().len()) {
                Some(e) => {
                    if e.kind() == std::io::ErrorKind::AlreadyExists {
                        pris.set(pris.get() + 1);
                    }
                    Err(e)
                }
                None => Ok(()),
            }
        },
        || {
            repli_fait.set(true);
            Ok(PathBuf::from("repli.png"))
        },
        &mut |d| attentes.push(d),
    );
    (r, liens.into_inner(), attentes, repli_fait.get())
}

#[test]
fn placer_trois_essais_a_50_ms_puis_repli() {
    use std::io::{Error, ErrorKind};
    let ms50 = Duration::from_millis(50);
    // Refus repete : 3 liens, 2 attentes de 50 ms, puis le repli.
    let (r, liens, attentes, repli) =
        essayer_placer(|_| Some(Error::from(ErrorKind::PermissionDenied)));
    assert_eq!(r.expect("repli"), PathBuf::from("repli.png"));
    assert_eq!(liens.len(), 3);
    assert_eq!(attentes, vec![ms50, ms50]);
    assert!(repli);

    // Fichier tenu (code 32) : meme traitement.
    let (_, liens, attentes, repli) = essayer_placer(|_| Some(Error::from_raw_os_error(32)));
    assert_eq!((liens.len(), attentes.len(), repli), (3, 2, true));

    // Refus puis reussite au 2e essai : une attente, pas de repli.
    let (r, liens, attentes, repli) =
        essayer_placer(|n| (n == 1).then(|| Error::from(ErrorKind::PermissionDenied)));
    assert_eq!(r.expect("lien"), Path::new("d").join("x0.png"));
    assert_eq!((liens.len(), attentes, repli), (2, vec![ms50], false));

    // Disque sans lien dur : repli tout de suite, sans attente.
    let (_, liens, attentes, repli) = essayer_placer(|_| Some(Error::from(ErrorKind::Unsupported)));
    assert_eq!((liens.len(), attentes.len(), repli), (1, 0, true));

    // Nom pris : nom suivant, sans attente ni repli.
    let (r, liens, attentes, repli) =
        essayer_placer(|n| (n <= 2).then(|| Error::from(ErrorKind::AlreadyExists)));
    assert_eq!(r.expect("lien"), Path::new("d").join("x2.png"));
    assert_eq!(
        liens,
        vec![
            Path::new("d").join("x0.png"),
            Path::new("d").join("x1.png"),
            Path::new("d").join("x2.png")
        ]
    );
    assert_eq!((attentes.len(), repli), (0, false));
}

#[test]
fn temporaires_perimes_format_exact() {
    use super::fichier::{est_nom_temporaire, temporaire_perime, AGE_TEMPORAIRE_PERIME};
    let vieux = Some(AGE_TEMPORAIRE_PERIME + Duration::from_secs(1));
    let jeune = Some(Duration::from_secs(10));
    let autre_pid = 999_999_999;
    assert!(est_nom_temporaire(".breezyshot-123-456789-0.tmp"));
    assert!(temporaire_perime(
        ".breezyshot-123-456789-0.tmp",
        vieux,
        autre_pid
    ));
    assert!(!temporaire_perime(
        ".breezyshot-123-456789-0.tmp",
        jeune,
        autre_pid
    ));
    assert!(!temporaire_perime(
        ".breezyshot-123-456789-0.tmp",
        None,
        autre_pid
    ));
    // Jamais un temporaire du processus courant, meme tres vieux.
    assert!(!temporaire_perime(
        ".breezyshot-123-456789-0.tmp",
        vieux,
        123
    ));
    // Le pid doit correspondre en entier, pas seulement par le debut.
    assert!(temporaire_perime(
        ".breezyshot-1234-456789-0.tmp",
        vieux,
        123
    ));
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
        assert!(!temporaire_perime(autre, vieux, autre_pid), "{autre}");
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
    let chemin = bilan.recent.expect("demande").expect("recent ecrit");
    assert_eq!(chemin, d.dossier_recents.join(format!("{base}.png")));
    assert_eq!(decoder(&fs::read(&chemin).expect("lecture")), entree);
    assert_eq!(bilan.copie, Etape::NonDemandee);
    assert_eq!(bilan.enregistrement, EtapeFichier::NonDemande);

    // Meme instant : collision en « _2 ».
    let bilan = sortir(&entree, &d);
    assert_eq!(
        bilan.recent,
        Some(Ok(d.dossier_recents.join(format!("{base}_2.png"))))
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
    // Le « .PNG » porte une autre date que les 60 : sur un disque qui ne distingue pas les
    // majuscules (Windows, macOS), il serait sinon le meme fichier que le premier recent.
    let etrangers = ["notes.txt", "20240101-000000-000.PNG", "photo.png"];
    for nom in etrangers {
        fs::write(d.dossier_recents.join(nom), b"autre").expect("ecriture");
    }

    let bilan = sortir(&image(2, 2), &d);
    let neuf = bilan.recent.expect("demande").expect("recent ecrit");
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
    assert!(matches!(bilan.recent, Some(Ok(_))));
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
        assert!(matches!(bilan.recent, Some(Ok(_))));

        let mut d = demande(racine.path());
        d.enregistrer = false;
        d.dossier_recents = dossier.clone();
        let bilan = sortir(&image(2, 2), &d);
        let cause = bilan.recent.expect("demande").expect_err("recents refuse");
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
        let r = b.recent.clone().expect("demande").expect("recent");
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
    // Temporaire de notre propre processus, date de deux heures : garde.
    let le_notre = format!(".breezyshot-{}-5-6.tmp", std::process::id());
    creer(&le_notre, Some(ancien));
    creer(".breezyshot-x.tmp", Some(ancien));
    creer("vacances.tmp", Some(ancien));

    let bilan = sortir(&image(2, 2), &d);
    assert!(matches!(bilan.enregistrement, EtapeFichier::Fait(_)));
    let restants = noms_du_dossier(&d.dossier_enregistrement);
    assert!(!restants.iter().any(|n| n == ".breezyshot-1-2-3.tmp"));
    for garde in [
        ".breezyshot-4-5-6.tmp",
        le_notre.as_str(),
        ".breezyshot-x.tmp",
        "vacances.tmp",
    ] {
        assert!(restants.iter().any(|n| n == garde), "{garde} supprime");
    }
}

// ---------- Transparence ----------

/// 4 × 3, coins transparents, alphas varies, couleurs qui disent leur place.
fn image_transparente() -> Pixels {
    let mut p = image(4, 3);
    let alphas = [0u8, 1, 64, 128, 200, 254, 255, 255, 17, 128, 255, 0];
    for (pixel, a) in p.rgba.chunks_exact_mut(4).zip(alphas) {
        pixel[3] = a;
    }
    p
}

#[test]
fn sur_blanc_compose_sur_un_fond_blanc() {
    let entree = [
        10, 20, 30, 0, // transparent : blanc
        10, 20, 30, 255, // opaque : intact
        0, 100, 255, 128, // moitie : melange arrondi
        200, 0, 50, 1,
    ];
    assert_eq!(
        sur_blanc(&entree),
        vec![
            255, 255, 255, 255, //
            10, 20, 30, 255, //
            127, 177, 255, 255, //
            255, 254, 254, 255,
        ]
    );
    // Un reste d'octets ignore, sans panique.
    assert_eq!(sur_blanc(&[1, 2, 3, 0, 9, 9]), vec![255, 255, 255, 255]);
    assert!(sur_blanc(&[]).is_empty());
}

#[test]
fn sortir_avec_transparence_garde_l_alpha() {
    let entree = image_transparente();
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let mut d = demande(racine.path());
    d.transparence = true;
    let bilan = sortir(&entree, &d);
    assert_eq!(bilan.copie, Etape::NonDemandee);
    let recent = bilan.recent.expect("demande").expect("recent ecrit");
    let EtapeFichier::Fait(chemin) = bilan.enregistrement else {
        panic!("enregistrement {:?}", bilan.enregistrement);
    };
    let octets = fs::read(&chemin).expect("fichier");
    // Les memes octets pour « Recents » et le fichier, encodes avec l'alpha.
    assert_eq!(fs::read(&recent).expect("recent"), octets);
    assert_eq!(decoder(&octets), entree);
    assert_eq!(
        png::encoder_rgba(4, 3, &entree.rgba).expect("encodage"),
        octets
    );
}

#[test]
fn sortir_sans_transparence_reste_opaque() {
    let entree = image_transparente();
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let d = demande(racine.path());
    assert!(!d.transparence);
    let bilan = sortir(&entree, &d);
    let EtapeFichier::Fait(chemin) = &bilan.enregistrement else {
        panic!("enregistrement {:?}", bilan.enregistrement);
    };
    let octets = fs::read(chemin).expect("fichier");
    // Au bit pres ce que donne l'encodage opaque.
    assert_eq!(octets, super::encoder_png(&entree).expect("encodage"));
    assert!(decoder(&octets).rgba.chunks_exact(4).all(|p| p[3] == 255));
    // Meme forme de bilan que la meme sortie avec transparence.
    let racine_t = tempfile::tempdir().expect("dossier temporaire");
    let mut t = demande(racine_t.path());
    t.transparence = true;
    let bilan_t = sortir(&entree, &t);
    assert_eq!(bilan.copie, bilan_t.copie);
    assert_eq!(
        bilan
            .recent
            .clone()
            .expect("demande")
            .as_ref()
            .map(|c| c.file_name().map(|n| n.to_owned())),
        bilan_t
            .recent
            .clone()
            .expect("demande")
            .as_ref()
            .map(|c| c.file_name().map(|n| n.to_owned()))
    );
    assert!(matches!(bilan_t.enregistrement, EtapeFichier::Fait(_)));
}

#[test]
fn sortir_avec_transparence_pixels_invalides_tout_en_echec() {
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let mut d = demande(racine.path());
    d.transparence = true;
    d.copier = true;
    let abimee = Pixels {
        largeur: 2,
        hauteur: 2,
        rgba: vec![0; 15],
    };
    let bilan = sortir(&abimee, &d);
    assert!(matches!(bilan.copie, Etape::Echec(_)));
    assert!(matches!(bilan.recent, Some(Err(_))));
    assert!(matches!(bilan.enregistrement, EtapeFichier::Echec { .. }));
}

#[test]
fn presse_papiers_natif_causes_et_reessai() {
    use crate::systeme::presse_papiers::ErreurPressePapiers as E;
    use presse_papiers::Reessayable;
    assert_eq!(
        presse_papiers::cause_native(E::Occupe),
        presse_papiers::cause(&arboard::Error::ClipboardOccupied)
    );
    assert_eq!(
        presse_papiers::cause_native(E::NonDisponible),
        presse_papiers::cause(&arboard::Error::ClipboardNotSupported)
    );
    assert_eq!(
        presse_papiers::cause_native(E::Refuse),
        "Le presse-papiers a refusé l'image."
    );
    assert!(E::Occupe.vaut_reessai());
    assert!(!E::Refuse.vaut_reessai());
    assert!(!E::NonDisponible.vaut_reessai());

    // La meme echeance que pour arboard.
    let h = Horloge::new();
    let essais = Cell::new(0);
    let r: Result<(), E> = presse_papiers::avec_echeance(
        presse_papiers::ECHEANCE,
        || h.maintenant(),
        |d| h.avancer(d),
        || {
            essais.set(essais.get() + 1);
            Err(E::Occupe)
        },
    );
    assert_eq!(r, Err(E::Occupe));
    assert!(h.ecoule.get() <= presse_papiers::ECHEANCE);
    assert!(essais.get() >= 2);
    let essais = Cell::new(0);
    let r: Result<(), E> = presse_papiers::avec_echeance(
        presse_papiers::ECHEANCE,
        || h.maintenant(),
        |d| h.avancer(d),
        || {
            essais.set(essais.get() + 1);
            Err(E::Refuse)
        },
    );
    assert_eq!(r, Err(E::Refuse));
    assert_eq!(essais.get(), 1);
}

// ---------- Presse-papiers (reel) ----------

/// Un seul test a la fois touche au vrai presse-papiers (verrou partage avec
/// les tests de `systeme::presse_papiers`).
use crate::systeme::presse_papiers::VERROU_ESSAIS as PRESSE_PAPIERS_TESTS;

/// Vrai si un presse-papiers existe. Sans lui : saute, ou echec si
/// `BREEZYSHOT_PRESSE_PAPIERS_OBLIGATOIRE=1`.
fn presse_papiers_present(test: &str) -> bool {
    match presse_papiers::sonder() {
        Ok(()) => true,
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

    // Relu par le meme objet garde, dans le meme processus (sous X11, c'est
    // lui qui sert l'image ; aucun objet n'est detruit en route),
    // en reessayant pendant 1 s.
    let debut = Instant::now();
    let relu = loop {
        let essai = presse_papiers::relire();
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

#[test]
fn presse_papiers_pose_et_relit_avec_transparence() {
    let _seul = PRESSE_PAPIERS_TESTS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if !presse_papiers_present("presse_papiers_pose_et_relit_avec_transparence") {
        return;
    }
    let entree = image_transparente();
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let mut d = demande(racine.path());
    d.copier = true;
    d.enregistrer = false;
    d.transparence = true;
    let bilan = sortir(&entree, &d);
    assert_eq!(bilan.copie, Etape::Faite);

    // Relu par arboard : il lit d'abord le format PNG (Windows) ou l'image
    // servie par notre processus (X11) ; l'alpha doit y etre.
    let debut = Instant::now();
    let relu = loop {
        match presse_papiers::relire() {
            Ok(image) => break image,
            Err(e) if debut.elapsed() >= Duration::from_secs(1) => {
                panic!("image non relue en 1 s : {e}")
            }
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    assert_eq!((relu.width, relu.height), (4, 3));
    assert_eq!(relu.bytes.as_ref(), entree.rgba.as_slice());
    println!("Presse-papiers : image 4x3 posee et relue avec son alpha.");
}

/// La chaine entiere, la ou le presse-papiers natif existe (Windows) : `sortir`
/// en mode transparent, puis le `CF_DIB` relu directement, sur fond blanc.
#[test]
fn presse_papiers_natif_dib_sur_blanc_depuis_sortir() {
    use crate::systeme::presse_papiers as natif;
    let _seul = PRESSE_PAPIERS_TESTS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if !natif::disponible() {
        println!("presse-papiers natif absent ici : chaine prouvee sous Windows seulement.");
        return;
    }
    // 6 x 6 d'une couleur, coin haut gauche transparent.
    let mut entree = Pixels {
        largeur: 6,
        hauteur: 6,
        rgba: [10u8, 20, 30, 255].repeat(36),
    };
    entree.rgba[3] = 0;
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let mut d = demande(racine.path());
    d.copier = true;
    d.enregistrer = false;
    d.transparence = true;
    assert_eq!(sortir(&entree, &d).copie, Etape::Faite);
    let dib = natif::lire_dib().expect("CF_DIB relu");
    assert_eq!(natif::pixel_dib(&dib, 0, 0), Some([255, 255, 255]), "coin");
    assert_eq!(natif::pixel_dib(&dib, 3, 3), Some([10, 20, 30]), "centre");
    assert_eq!(natif::pixel_dib(&dib, 5, 0), Some([10, 20, 30]));
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
#[ignore = "mesure : en debug les chiffres trompent et le presse-papiers est ecrase"]
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
        assert!(matches!(bilan.recent, Some(Ok(_))));
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

// ---------- Enregistrer sous... de l'editeur (enregistrer_a, 0.4) ----------

/// Des pixels a l'alpha varie (0, 128, 255) et aux couleurs toutes differentes.
fn image_alpha_variee(largeur: u32, hauteur: u32) -> Pixels {
    let mut pixels = image(largeur, hauteur);
    for (i, p) in pixels.rgba.chunks_exact_mut(4).enumerate() {
        p[3] = [0, 128, 255][i % 3];
    }
    pixels
}

#[test]
fn enregistrer_a_ecrit_le_png_exact_alpha_garde_ou_force() {
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let pixels = image_alpha_variee(7, 5);
    for (transparence, nom) in [(true, "garde.png"), (false, "force.png")] {
        let chemin = racine.path().join(nom);
        let etape = super::enregistrer_a(&pixels, &chemin, transparence);
        assert_eq!(etape, EtapeFichier::Fait(chemin.clone()));
        let relu = decoder(&fs::read(&chemin).expect("lecture"));
        assert_eq!((relu.largeur, relu.hauteur), (7, 5));
        let mut attendu = pixels.rgba.clone();
        if !transparence {
            attendu.chunks_exact_mut(4).for_each(|p| p[3] = 255);
        }
        assert_eq!(relu.rgba, attendu, "transparence {transparence}");
    }
    sans_temporaire(racine.path());
    assert_eq!(noms_du_dossier(racine.path()).len(), 2);
}

#[test]
fn enregistrer_a_remplace_le_fichier_choisi() {
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let chemin = racine.path().join("deja là.png");
    fs::write(&chemin, b"ancien contenu bien plus long qu'un petit PNG ?").expect("ecriture");
    let etape = super::enregistrer_a(&image(3, 2), &chemin, false);
    assert_eq!(etape, EtapeFichier::Fait(chemin.clone()));
    assert_eq!(decoder(&fs::read(&chemin).expect("lecture")), image(3, 2));
    // Un seul fichier : remplace, pas de « (2) », aucun temporaire.
    assert_eq!(noms_du_dossier(racine.path()), noms(&["deja là.png"]));
}

#[test]
fn enregistrer_a_echoue_sur_un_dossier_absent_sans_le_creer() {
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let absent = racine.path().join("Absent").join("Sous-dossier");
    let chemin = absent.join("capture.png");
    let etape = super::enregistrer_a(&image(2, 2), &chemin, false);
    assert_eq!(
        etape,
        EtapeFichier::Echec {
            dossier: absent.clone(),
            cause: "Le dossier est introuvable.".to_string()
        }
    );
    assert!(!racine.path().join("Absent").exists());
    assert!(noms_du_dossier(racine.path()).is_empty());
}

#[test]
fn enregistrer_a_echoue_sans_rien_laisser_si_la_cible_est_un_dossier() {
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let cible = racine.path().join("pris.png");
    fs::create_dir(&cible).expect("dossier");
    fs::write(cible.join("dedans"), b"x").expect("ecriture");
    let etape = super::enregistrer_a(&image(2, 2), &cible, false);
    let EtapeFichier::Echec { dossier, cause } = etape else {
        panic!("echec attendu : {etape:?}");
    };
    assert_eq!(dossier, racine.path());
    assert!(
        !cause.contains(&*racine.path().to_string_lossy()),
        "{cause}"
    );
    assert!(cible.is_dir());
    assert_eq!(noms_du_dossier(&cible), noms(&["dedans"]));
    sans_temporaire(racine.path());
    assert_eq!(noms_du_dossier(racine.path()), noms(&["pris.png"]));
}

#[test]
fn enregistrer_a_refuse_sans_panique_ni_ecriture() {
    let courant = std::env::current_dir().expect("dossier courant");
    let avant = noms_du_dossier(&courant);
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let abimee = Pixels {
        largeur: 3,
        hauteur: 3,
        rgba: vec![0; 35],
    };
    let vide = Pixels {
        largeur: 0,
        hauteur: 0,
        rgba: Vec::new(),
    };
    let dans = |nom: &str| racine.path().join(nom);
    let cas = [
        (abimee, dans("abimee.png"), "La capture est vide ou abîmée."),
        (vide, dans("vide.png"), "La capture est vide ou abîmée."),
        (image(2, 2), PathBuf::new(), "Le dossier est introuvable."),
        (
            image(2, 2),
            PathBuf::from("relatif-breezyshot-test.png"),
            "Le dossier est introuvable.",
        ),
        (
            image(2, 2),
            racine.path().join(".."),
            "Le dossier est introuvable.",
        ),
    ];
    for (pixels, chemin, cause) in cas {
        let etape = super::enregistrer_a(&pixels, &chemin, true);
        let EtapeFichier::Echec { cause: rendue, .. } = &etape else {
            panic!("echec attendu pour {chemin:?} : {etape:?}");
        };
        assert_eq!(rendue, cause, "{chemin:?}");
    }
    assert!(noms_du_dossier(racine.path()).is_empty());
    assert_eq!(noms_du_dossier(&courant), avant);
}

#[test]
fn enregistrer_a_pixels_abimes_laisse_l_original_intact() {
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let chemin = racine.path().join("original.png");
    let ancien: Vec<u8> = (0..=255).collect();
    fs::write(&chemin, &ancien).expect("ecriture");
    let abimee = Pixels {
        largeur: 4,
        hauteur: 4,
        rgba: vec![7; 63],
    };
    let etape = super::enregistrer_a(&abimee, &chemin, false);
    assert!(matches!(etape, EtapeFichier::Echec { .. }), "{etape:?}");
    assert_eq!(fs::read(&chemin).expect("lecture"), ancien);
    assert_eq!(noms_du_dossier(racine.path()), noms(&["original.png"]));
}

#[test]
fn enregistrer_a_tolere_un_disque_sans_vidage() {
    use std::io::{Error, ErrorKind};
    assert!(fichier::sync_tolere(Ok(())).is_ok());
    assert!(fichier::sync_tolere(Err(Error::from(ErrorKind::Unsupported))).is_ok());
    for genre in [
        ErrorKind::PermissionDenied,
        ErrorKind::StorageFull,
        ErrorKind::Other,
        ErrorKind::InvalidInput,
    ] {
        let rendu = fichier::sync_tolere(Err(Error::from(genre))).expect_err("echec garde");
        assert_eq!(rendu.kind(), genre);
    }
}

// ---------- Historique (contrat 0.5, §3.1) ----------

use super::historique::{
    crc32, dossier_apercus_de, filtrer_au_format, horodatage_du_nom, nettoyer_apercus, orphelins,
    taille_decodee, taille_ihdr, NomsLus, OCTETS_MAX,
};
use super::{
    chemin_apercu, chemin_recent, lire_recent, lister_recents, ranger_apercu, supprimer_recent,
    vider, BilanVidage,
};

/// `<racine>/local/{Recents,Apercus,Glisser}` et `<racine>/Images`, crees.
struct Dossiers {
    _racine: tempfile::TempDir,
    local: PathBuf,
    recents: PathBuf,
    apercus: PathBuf,
    glisser: PathBuf,
    images: PathBuf,
}

fn dossiers() -> Dossiers {
    let racine = tempfile::tempdir().expect("dossier temporaire");
    let local = racine.path().join("local");
    let d = Dossiers {
        recents: local.join("Recents"),
        apercus: local.join("Apercus"),
        glisser: local.join("Glisser"),
        images: racine.path().join("Images"),
        local,
        _racine: racine,
    };
    for dossier in [&d.recents, &d.apercus, &d.glisser, &d.images] {
        fs::create_dir_all(dossier).expect("dossier");
    }
    d
}

fn png_de(pixels: &Pixels) -> Vec<u8> {
    png::encoder_rgba(pixels.largeur, pixels.hauteur, &pixels.rgba).expect("encodage")
}

/// Des fichiers qui ne sont pas a nous, a cote des notres.
// Le « .PNG » porte une autre heure que les captures des tests : sur un disque qui ne
// distingue pas les majuscules (Windows, macOS), il serait sinon le meme fichier qu'elles.
const ETRANGERS: [&str; 6] = [
    "notes.txt",
    "photo.png",
    "20261111-194012-777.PNG",
    "20261111-194012-123.png.bak",
    "x20261111-194012-123.png",
    "20261111-194012-123_1.png",
];

fn poser_etrangers(dossier: &Path) {
    for nom in ETRANGERS {
        fs::write(dossier.join(nom), b"pas a nous").expect("ecriture");
    }
    // Un sous-dossier qui porte un nom au format, et un fichier dedans.
    let sous = dossier.join("20261111-194012-999.png");
    fs::create_dir_all(&sous).expect("sous-dossier");
    fs::write(sous.join("20261111-194012-998.png"), b"dedans").expect("ecriture");
}

fn etrangers_intacts(dossier: &Path) {
    for nom in ETRANGERS {
        assert_eq!(
            fs::read(dossier.join(nom)).expect(nom),
            b"pas a nous",
            "{nom} touche"
        );
    }
    let sous = dossier.join("20261111-194012-999.png");
    assert!(sous.is_dir(), "sous-dossier efface");
    assert!(
        sous.join("20261111-194012-998.png").is_file(),
        "fichier du sous-dossier efface"
    );
}

#[test]
fn horodatage_lu_dans_le_nom() {
    let t = horodatage_du_nom("20261111-194012-123_2.png").expect("date");
    assert_eq!(
        t,
        Utc.with_ymd_and_hms(2026, 11, 11, 19, 40, 12).unwrap() + TimeDelta::milliseconds(123)
    );
    // Le nom que `sortir` ecrit redonne l'heure de la capture.
    let base = base_recent(instant());
    let relu = horodatage_du_nom(&format!("{base}.png")).expect("date");
    assert_eq!(relu, instant().with_timezone(&Utc));
    // Au format mais impossible : ecarte.
    for nom in [
        "20261311-194012-123.png",
        "20261131-250000-000.png",
        "notes.png",
    ] {
        assert_eq!(horodatage_du_nom(nom), None, "{nom}");
    }
}

#[test]
fn ihdr_lu_et_verifie() {
    let octets = png_de(&image(37, 11));
    assert_eq!(taille_ihdr(&octets[..33]), Some((37, 11)));
    // Trop court, signature fausse, CRC faux.
    assert_eq!(taille_ihdr(&octets[..32]), None);
    let mut faux = octets.clone();
    faux[0] = 0;
    assert_eq!(taille_ihdr(&faux), None);
    let mut faux = octets.clone();
    faux[17] ^= 1; // largeur changee sans son CRC
    assert_eq!(taille_ihdr(&faux), None);
    // Un PNG RGB (pas ce que `sortir` ecrit) : ecarte.
    let mut rgb = Vec::new();
    let mut e = ::png::Encoder::new(&mut rgb, 2, 2);
    e.set_color(::png::ColorType::Rgb);
    e.set_depth(::png::BitDepth::Eight);
    e.write_header()
        .unwrap()
        .write_image_data(&[0; 12])
        .unwrap();
    assert_eq!(taille_ihdr(&rgb), None);
}

#[test]
fn lister_recents_ordre_tailles_et_ecartes() {
    let d = dossiers();
    poser_etrangers(&d.recents);
    let tailles = [
        ("20261111-194012-123.png", (30, 20)),
        ("20261111-194012-123_2.png", (31, 21)),
        ("20261111-194012-123_10.png", (32, 22)),
        ("20250101-000000-000.png", (5, 5)),
        ("20261112-080000-001.png", (64, 40)),
    ];
    for (nom, (l, h)) in tailles {
        fs::write(d.recents.join(nom), png_de(&image(l, h))).expect("ecriture");
    }
    let complet = png_de(&image(50, 50));
    // Tronques : avant la fin de l'en-tete, et au milieu des donnees.
    fs::write(d.recents.join("20261113-000000-000.png"), &complet[..20]).expect("ecriture");
    fs::write(
        d.recents.join("20261113-000000-001.png"),
        &complet[..complet.len() - 5],
    )
    .expect("ecriture");
    // Au format, mais pas un PNG ; vide ; date impossible.
    fs::write(d.recents.join("20261113-000000-002.png"), b"pas une image").expect("ecriture");
    fs::write(d.recents.join("20261113-000000-003.png"), b"").expect("ecriture");
    fs::write(d.recents.join("20261399-000000-000.png"), &complet).expect("ecriture");

    let liste = lister_recents(&d.recents);
    let vus: Vec<(&str, u32, u32)> = liste
        .iter()
        .map(|e| (e.nom.as_str(), e.largeur, e.hauteur))
        .collect();
    assert_eq!(
        vus,
        [
            ("20261112-080000-001.png", 64, 40),
            ("20261111-194012-123_10.png", 32, 22),
            ("20261111-194012-123_2.png", 31, 21),
            ("20261111-194012-123.png", 30, 20),
            ("20250101-000000-000.png", 5, 5),
        ]
    );
    assert_eq!(
        liste[0].horodatage,
        Utc.with_ymd_and_hms(2026, 11, 12, 8, 0, 0).unwrap() + TimeDelta::milliseconds(1)
    );
    // Lister ne touche a rien.
    etrangers_intacts(&d.recents);
    // Dossier absent ou chemin vide : liste vide, sans panique.
    assert!(lister_recents(&d.local.join("absent")).is_empty());
    assert!(lister_recents(Path::new("")).is_empty());
}

#[test]
fn chemin_recent_format_exact_seulement() {
    let dossier = Path::new("/l/Recents");
    assert_eq!(
        chemin_recent(dossier, "20261111-194012-123.png"),
        Some(dossier.join("20261111-194012-123.png"))
    );
    assert_eq!(
        chemin_recent(dossier, "20261111-194012-123_2.png"),
        Some(dossier.join("20261111-194012-123_2.png"))
    );
    for nom in [
        "",
        "../20261111-194012-123.png",
        "..\\20261111-194012-123.png",
        "/20261111-194012-123.png",
        "20261111-194012-123.PNG",
        "20261111-194012-123",
        "20261111-194012-123_1.png",
        "20261111-194012-123_02.png",
        "photo.png",
        "20261111-194012-123.png/..",
    ] {
        assert_eq!(chemin_recent(dossier, nom), None, "{nom:?}");
        assert_eq!(chemin_apercu(dossier, nom), None, "{nom:?}");
    }
    assert_eq!(
        chemin_recent(Path::new(""), "20261111-194012-123.png"),
        None
    );
    assert_eq!(
        dossier_apercus_de(dossier),
        Some(Path::new("/l").join("Apercus"))
    );
    assert_eq!(dossier_apercus_de(Path::new("Recents")), None);
}

#[test]
fn lire_recent_aller_retour_exact_et_alpha() {
    let d = dossiers();
    // Ecrit par `sortir` lui-meme, avec et sans transparence.
    let mut dem = demande(d.local.parent().unwrap());
    dem.enregistrer = false;
    dem.dossier_recents = d.recents.clone();
    dem.transparence = true;
    let transparente = image_transparente();
    let chemin = sortir(&transparente, &dem).recent.unwrap().expect("recent");
    let (relu, alpha) = lire_recent(&chemin).expect("relu");
    assert_eq!(relu, transparente);
    assert!(alpha);

    dem.transparence = false;
    let opaque = image(33, 17);
    let chemin = sortir(&opaque, &dem).recent.unwrap().expect("recent");
    let (relu, alpha) = lire_recent(&chemin).expect("relu");
    assert_eq!(relu, opaque);
    assert!(!alpha);

    // Tronque, abime, absent, autre format : une erreur sans chemin.
    let octets = fs::read(&chemin).unwrap();
    let abime = d.recents.join("20261113-000000-000.png");
    for contenu in [
        &octets[..octets.len() / 2],
        &octets[..10],
        b"rien".as_slice(),
    ] {
        fs::write(&abime, contenu).unwrap();
        let cause = lire_recent(&abime).expect_err("illisible");
        assert!(!cause.contains("Recents"), "{cause}");
    }
    assert!(lire_recent(&d.recents.join("absent.png")).is_err());
    let mut rgb = Vec::new();
    let mut e = ::png::Encoder::new(&mut rgb, 2, 2);
    e.set_color(::png::ColorType::Rgb);
    e.set_depth(::png::BitDepth::Eight);
    e.write_header()
        .unwrap()
        .write_image_data(&[0; 12])
        .unwrap();
    fs::write(&abime, rgb).unwrap();
    assert!(lire_recent(&abime).is_err());
}

/// Compte la plus grosse allocation demandee par le fil courant (tous les tests de ce
/// binaire passent par lui ; il ne fait que mesurer, puis laisse faire `System`).
struct MesureAllocations;

thread_local! {
    static PLUS_GROSSE: Cell<usize> = const { Cell::new(0) };
}

fn noter_allocation(octets: usize) {
    // `try_with` : jamais de panique pendant la fin du fil.
    let _ = PLUS_GROSSE.try_with(|c| c.set(c.get().max(octets)));
}

// SAFETY : chaque appel est passe tel quel a `System`, qui tient le contrat de `GlobalAlloc`.
unsafe impl std::alloc::GlobalAlloc for MesureAllocations {
    unsafe fn alloc(&self, mise: std::alloc::Layout) -> *mut u8 {
        noter_allocation(mise.size());
        // SAFETY : meme contrat que l'appelant.
        unsafe { std::alloc::System.alloc(mise) }
    }

    unsafe fn alloc_zeroed(&self, mise: std::alloc::Layout) -> *mut u8 {
        noter_allocation(mise.size());
        // SAFETY : meme contrat que l'appelant.
        unsafe { std::alloc::System.alloc_zeroed(mise) }
    }

    unsafe fn realloc(&self, p: *mut u8, mise: std::alloc::Layout, taille: usize) -> *mut u8 {
        noter_allocation(taille);
        // SAFETY : meme contrat que l'appelant.
        unsafe { std::alloc::System.realloc(p, mise, taille) }
    }

    unsafe fn dealloc(&self, p: *mut u8, mise: std::alloc::Layout) {
        // SAFETY : meme contrat que l'appelant.
        unsafe { std::alloc::System.dealloc(p, mise) }
    }
}

#[global_allocator]
static ALLOCATIONS: MesureAllocations = MesureAllocations;

/// La plus grosse allocation faite par ce fil pendant `f`.
fn plus_grosse_allocation<T>(f: impl FnOnce() -> T) -> (T, usize) {
    PLUS_GROSSE.with(|c| c.set(0));
    let resultat = f();
    (resultat, PLUS_GROSSE.with(Cell::get))
}

/// Un bloc PNG : longueur, type, donnees, CRC.
fn bloc_png(sortie: &mut Vec<u8>, genre: &[u8; 4], donnees: &[u8]) {
    sortie.extend_from_slice(&(donnees.len() as u32).to_be_bytes());
    let debut = sortie.len();
    sortie.extend_from_slice(genre);
    sortie.extend_from_slice(donnees);
    let crc = crc32(&sortie[debut..]);
    sortie.extend_from_slice(&crc.to_be_bytes());
}

/// Un PNG fait a la main : en-tete `largeur` × `hauteur` RGBA 8 bits, puis un IDAT qui ne
/// porte que deux lignes nulles (bloc zlib « stocke »), puis IEND. Pour une image de deux
/// lignes de `largeur` 1, c'est un PNG complet et juste.
fn png_a_la_main(largeur: u32, hauteur: u32) -> Vec<u8> {
    let mut octets = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&largeur.to_be_bytes());
    ihdr.extend_from_slice(&hauteur.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    bloc_png(&mut octets, b"IHDR", &ihdr);
    // Deux lignes : filtre 0 puis un pixel (0, 0, 0, 255).
    let brut = [0u8, 0, 0, 0, 255, 0, 0, 0, 0, 255];
    let (mut a, mut b) = (1u32, 0u32);
    for &o in &brut {
        a = (a + u32::from(o)) % 65521;
        b = (b + a) % 65521;
    }
    let longueur = brut.len() as u16;
    let mut zlib = vec![0x78, 0x01, 0x01];
    zlib.extend_from_slice(&longueur.to_le_bytes());
    zlib.extend_from_slice(&(!longueur).to_le_bytes());
    zlib.extend_from_slice(&brut);
    zlib.extend_from_slice(&((b << 16) | a).to_be_bytes());
    bloc_png(&mut octets, b"IDAT", &zlib);
    bloc_png(&mut octets, b"IEND", &[]);
    octets
}

#[test]
fn lire_recent_juge_l_en_tete_avant_d_allouer() {
    let d = dossiers();
    let chemin = d.recents.join("20261113-000000-000.png");
    // Le PNG fait a la main est juste : une image 1 × 2 opaque, relue.
    fs::write(&chemin, png_a_la_main(1, 2)).unwrap();
    let (relu, alpha) = lire_recent(&chemin).expect("png a la main");
    assert_eq!((relu.largeur, relu.hauteur), (1, 2));
    assert_eq!(relu.rgba, [0, 0, 0, 255, 0, 0, 0, 255]);
    assert!(!alpha);
    // La mesure voit bien une grosse allocation.
    let (_, vue) = plus_grosse_allocation(|| vec![1u8; 32 << 20]);
    assert!(vue >= 32 << 20, "{vue}");

    // 16384 × 16384 annonce (1 Gio decode), 1 Ko sur le disque : refuse sans allouer
    // l'image. Juste au-dessus de la limite aussi.
    for (largeur, hauteur) in [(16384, 16384), (16384, 8193), (u32::MAX, u32::MAX)] {
        fs::write(&chemin, png_a_la_main(largeur, hauteur)).unwrap();
        assert!(fs::metadata(&chemin).unwrap().len() < 1024);
        let (lu, plus_grosse) = plus_grosse_allocation(|| lire_recent(&chemin));
        assert!(lu.is_err(), "{largeur} × {hauteur}");
        assert!(
            plus_grosse < 8 << 20,
            "{largeur} × {hauteur} : {plus_grosse} octets alloues"
        );
    }
    // La limite elle-meme : 512 Mio.
    assert_eq!(OCTETS_MAX, 512 << 20);
    assert_eq!(taille_decodee(16384, 8192), Some(512 << 20));
    assert_eq!(taille_decodee(16384, 8193), None);
    assert_eq!(taille_decodee(16384, 16384), None);
    assert_eq!(taille_decodee(u32::MAX, u32::MAX), None);
    assert_eq!(taille_decodee(0, 5), None);
    assert_eq!(taille_decodee(3, 2), Some(24));
}

#[test]
fn apercu_range_relu_et_remplace() {
    let d = dossiers();
    let apercus = d.local.join("Apercus-neuf");
    let nom = "20261111-194012-123.png";
    let premier = image_transparente();
    ranger_apercu(&apercus, nom, &premier).expect("apercu");
    let chemin = chemin_apercu(&apercus, nom).expect("chemin");
    // Alpha garde (fenetre aux coins arrondis).
    assert_eq!(lire_recent(&chemin).expect("relu").0, premier);
    // Refait : remplace.
    let second = image(8, 5);
    ranger_apercu(&apercus, nom, &second).expect("apercu");
    assert_eq!(lire_recent(&chemin).expect("relu").0, second);
    assert_eq!(noms_du_dossier(&apercus), [nom]);
    // Nom refuse ou pixels abimes : rien d'ecrit.
    assert!(ranger_apercu(&apercus, "../photo.png", &second).is_err());
    assert!(ranger_apercu(&apercus, "photo.png", &second).is_err());
    let mut abimees = image(4, 4);
    abimees.rgba.pop();
    assert!(ranger_apercu(&apercus, "20261111-194012-124.png", &abimees).is_err());
    assert_eq!(noms_du_dossier(&apercus), [nom]);
    assert!(!d.local.join("photo.png").exists());
}

#[test]
fn supprimer_recent_le_png_et_son_apercu_seulement() {
    let d = dossiers();
    poser_etrangers(&d.recents);
    poser_etrangers(&d.apercus);
    let nom = "20261111-194012-123.png";
    let voisin = "20261111-194012-123_2.png";
    for n in [nom, voisin] {
        fs::write(d.recents.join(n), png_de(&image(4, 4))).unwrap();
        ranger_apercu(&d.apercus, n, &image(2, 2)).unwrap();
    }
    // Le fichier enregistre par l'utilisateur et un lien de glisser restent (R14).
    let enregistre = d.images.join("BreezyShot 2026-11-11 à 20.40.12.png");
    fs::write(&enregistre, b"a l'utilisateur").unwrap();
    let lien = d.glisser.join("BreezyShot 2026-11-11 à 20.40.12.png");
    fs::write(&lien, b"lien").unwrap();

    supprimer_recent(&d.recents, &d.apercus, nom).expect("supprime");
    assert!(!d.recents.join(nom).exists());
    assert!(!d.apercus.join(nom).exists());
    assert!(d.recents.join(voisin).is_file());
    assert!(d.apercus.join(voisin).is_file());
    assert!(enregistre.is_file() && lien.is_file());
    etrangers_intacts(&d.recents);
    etrangers_intacts(&d.apercus);

    // Deja parti : fait. Nom hors format : refuse, rien de touche.
    supprimer_recent(&d.recents, &d.apercus, nom).expect("deja parti");
    for faux in ETRANGERS.iter().copied().chain(["../Images/x.png", ""]) {
        assert!(
            supprimer_recent(&d.recents, &d.apercus, faux).is_err(),
            "{faux}"
        );
    }
    // Un sous-dossier au nom du format : pas dans la liste, gardé (le sous-dossier du meme
    // nom dans les apercus aussi).
    supprimer_recent(&d.recents, &d.apercus, "20261111-194012-999.png").expect("hors liste");
    etrangers_intacts(&d.recents);
    etrangers_intacts(&d.apercus);
    // Apercu absent : la capture part quand meme.
    fs::remove_file(d.apercus.join(voisin)).unwrap();
    supprimer_recent(&d.recents, &d.apercus, voisin).expect("supprime");
    assert!(!d.recents.join(voisin).exists());
}

#[test]
fn vider_ne_touche_que_nos_formats() {
    let d = dossiers();
    for dossier in [&d.recents, &d.apercus, &d.glisser] {
        poser_etrangers(dossier);
    }
    let captures = [
        "20261111-194012-123.png",
        "20261111-194012-123_2.png",
        "20250101-000000-000.png",
    ];
    for n in captures {
        fs::write(d.recents.join(n), png_de(&image(4, 4))).unwrap();
        ranger_apercu(&d.apercus, n, &image(2, 2)).unwrap();
    }
    // Un apercu sans capture part aussi.
    ranger_apercu(&d.apercus, "20240101-000000-000.png", &image(2, 2)).unwrap();
    for n in ["BreezyShot 2026-11-11 à 20.40.12.png", "BreezyShot (2).PNG"] {
        fs::write(d.glisser.join(n), b"lien").unwrap();
    }
    // Ce qui n'est pas dans ces trois dossiers reste : reglages, journal, Images.
    let reglages = d.local.join("reglages.json");
    let journal = d.local.join("20261111-194012-123.png");
    let enregistre = d.images.join("20261111-194012-123.png");
    for f in [&reglages, &journal, &enregistre] {
        fs::write(f, b"garde").unwrap();
    }

    let bilan = vider(&d.recents, &d.apercus, &d.glisser);
    assert_eq!(
        bilan,
        BilanVidage {
            effaces: 3,
            echecs: 0
        }
    );
    for dossier in [&d.recents, &d.apercus] {
        etrangers_intacts(dossier);
        let restes: Vec<String> = noms_du_dossier(dossier)
            .into_iter()
            .filter(|n| cle_recent(n).is_some())
            .collect();
        // Seul le sous-dossier au nom du format reste.
        assert_eq!(restes, ["20261111-194012-999.png"]);
    }
    // `Glisser` : tous les `.png` (toute casse), rien d'autre ; le sous-dossier reste.
    let mut restes = noms_du_dossier(&d.glisser);
    restes.sort();
    let mut attendus = vec![
        "20261111-194012-123.png.bak",
        "20261111-194012-999.png",
        "notes.txt",
    ];
    attendus.sort();
    assert_eq!(restes, attendus);
    assert!(d
        .glisser
        .join("20261111-194012-999.png")
        .join("20261111-194012-998.png")
        .is_file());
    for f in [&reglages, &journal, &enregistre] {
        assert_eq!(fs::read(f).unwrap(), b"garde");
    }
    // Encore : rien a faire. Dossiers absents ou vides : sans panique.
    assert_eq!(
        vider(&d.recents, &d.apercus, &d.glisser),
        BilanVidage::default()
    );
    let absent = d.local.join("absent");
    assert_eq!(vider(&absent, &absent, &absent), BilanVidage::default());
    let vide = Path::new("");
    assert_eq!(vider(vide, vide, vide), BilanVidage::default());
}

/// Les apercus au format exact de `dossier`, tries.
fn apercus_au_format(dossier: &Path) -> Vec<String> {
    let mut noms: Vec<String> = noms_du_dossier(dossier)
        .into_iter()
        .filter(|n| cle_recent(n).is_some() && dossier.join(n).is_file())
        .collect();
    noms.sort();
    noms
}

#[test]
fn nettoyage_a_50_efface_l_apercu_des_captures_retirees() {
    let d = dossiers();
    let mut dem = demande(d.local.parent().unwrap());
    dem.enregistrer = false;
    dem.dossier_recents = d.recents.clone();
    poser_etrangers(&d.apercus);
    // 55 captures, chacune avec son apercu ; aucun apercu orphelin.
    let noms: Vec<String> = (0..55)
        .map(|i| format!("20250101-0000{i:02}-000.png"))
        .collect();
    for nom in &noms {
        fs::write(d.recents.join(nom), png_de(&image(2, 2))).unwrap();
        ranger_apercu(&d.apercus, nom, &image(1, 1)).unwrap();
    }
    let neuf = sortir(&image(2, 2), &dem).recent.unwrap().expect("recent");
    let nom_neuf = neuf.file_name().unwrap().to_str().unwrap().to_string();

    // Les 6 plus anciennes parties, avec leur apercu ; les 49 autres gardent le leur.
    let liste = lister_recents(&d.recents);
    assert_eq!(liste.len(), RECENTS_GARDES);
    assert_eq!(liste[0].nom, nom_neuf);
    for nom in &noms[..6] {
        assert!(!d.recents.join(nom).exists(), "{nom}");
        assert!(!d.apercus.join(nom).exists(), "{nom}");
    }
    assert_eq!(apercus_au_format(&d.apercus), noms[6..]);
    etrangers_intacts(&d.apercus);

    // Le chemin des captures retirees, seul : un nom donne comme parti perd son apercu
    // meme si sa capture est encore la (ce n'est donc pas l'effacement des orphelins).
    let garde = &noms[10];
    assert!(d.recents.join(garde).is_file());
    nettoyer_apercus(&d.recents, std::slice::from_ref(garde));
    assert!(!d.apercus.join(garde).exists());
    assert!(d.recents.join(garde).is_file());
    assert_eq!(apercus_au_format(&d.apercus).len(), 48);
}

#[test]
fn nettoyage_efface_les_apercus_orphelins() {
    let d = dossiers();
    let mut dem = demande(d.local.parent().unwrap());
    dem.enregistrer = false;
    dem.dossier_recents = d.recents.clone();
    poser_etrangers(&d.apercus);
    // Trois captures, loin de la limite : le nettoyage ne retire aucune capture.
    let captures = [
        "20250101-000000-000.png",
        "20250101-000001-000.png",
        "20250101-000001-000_2.png",
    ];
    for nom in captures {
        fs::write(d.recents.join(nom), png_de(&image(2, 2))).unwrap();
        ranger_apercu(&d.apercus, nom, &image(1, 1)).unwrap();
    }
    // Deux apercus sans capture (capture supprimee par la vignette, ou a la main).
    for orphelin in ["20240101-000000-000.png", "20250101-000001-000_3.png"] {
        ranger_apercu(&d.apercus, orphelin, &image(1, 1)).unwrap();
    }
    sortir(&image(2, 2), &dem).recent.unwrap().expect("recent");

    assert_eq!(lister_recents(&d.recents).len(), 4);
    for nom in captures {
        assert!(d.recents.join(nom).is_file(), "{nom}");
    }
    // Les orphelins seuls sont partis.
    assert_eq!(apercus_au_format(&d.apercus), captures);
    etrangers_intacts(&d.apercus);
}

#[test]
fn liste_partielle_aucun_apercu_juge_orphelin() {
    use std::ffi::OsString;
    use std::io;
    let entree = |nom: &str, fichier: bool| Ok((OsString::from(nom), Ok(fichier)));
    let a = "20250101-000000-000.png";
    let b = "20250101-000001-000.png";
    // Entrees triees : seuls les fichiers ordinaires au format restent.
    let lus = filtrer_au_format(
        [
            entree(a, true),
            entree("notes.txt", true),
            entree("20250101-000002-000.png", false),
            entree(b, true),
        ]
        .into_iter(),
    );
    assert_eq!(
        lus,
        NomsLus {
            noms: vec![a.to_string(), b.to_string()],
            complete: true
        }
    );
    // Une entree en erreur pendant le parcours, ou un type illisible : liste incomplete,
    // le reste est quand meme lu.
    let en_erreur = filtrer_au_format(
        [
            entree(a, true),
            Err(io::Error::other("entree illisible")),
            entree(b, true),
        ]
        .into_iter(),
    );
    assert_eq!(en_erreur.noms, [a, b]);
    assert!(!en_erreur.complete);
    let type_illisible = filtrer_au_format(
        [
            entree(a, true),
            Ok((OsString::from(b), Err(io::Error::other("type")))),
        ]
        .into_iter(),
    );
    assert_eq!(type_illisible.noms, [a]);
    assert!(!type_illisible.complete);

    // Les orphelins : seulement avec deux listes completes.
    let apercus = NomsLus {
        noms: vec![a.to_string(), b.to_string()],
        complete: true,
    };
    let presents = NomsLus {
        noms: vec![a.to_string()],
        complete: true,
    };
    assert_eq!(orphelins(&apercus, &presents), [b]);
    let partiels = NomsLus {
        noms: vec![a.to_string()],
        complete: false,
    };
    assert!(orphelins(&apercus, &partiels).is_empty());
    let apercus_partiels = NomsLus {
        noms: apercus.noms.clone(),
        complete: false,
    };
    assert!(orphelins(&apercus_partiels, &presents).is_empty());
    // « Recents » absent : liste incomplete, aucun apercu efface.
    let d = dossiers();
    ranger_apercu(&d.apercus, a, &image(1, 1)).unwrap();
    fs::remove_dir(&d.recents).unwrap();
    nettoyer_apercus(&d.recents, &[]);
    assert!(d.apercus.join(a).is_file());
}

#[test]
fn supprimer_recent_nom_de_la_liste_a_l_octet_pres() {
    let d = dossiers();
    let complet = png_de(&image(4, 4));
    // Dans la liste : le PNG et son apercu partent.
    let liste = "20261111-194012-125.png";
    fs::write(d.recents.join(liste), &complet).unwrap();
    ranger_apercu(&d.apercus, liste, &image(2, 2)).unwrap();
    assert_eq!(lister_recents(&d.recents)[0].nom, liste);
    supprimer_recent(&d.recents, &d.apercus, liste).expect("supprime");
    assert!(!d.recents.join(liste).exists());
    assert!(!d.apercus.join(liste).exists());
}

#[test]
fn supprimer_recent_hors_liste_n_efface_que_l_apercu() {
    let d = dossiers();
    let complet = png_de(&image(4, 4));
    // Sous Windows, « …123.png » ouvrirait ce « …123.PNG » : il n'est pas dans la liste.
    let majuscules = "20261111-194012-123.PNG";
    let nom = "20261111-194012-123.png";
    fs::write(d.recents.join(majuscules), &complet).unwrap();
    ranger_apercu(&d.apercus, nom, &image(2, 2)).unwrap();
    // Au format mais tronque : pas dans la liste non plus.
    let tronque = "20261111-194012-124.png";
    fs::write(d.recents.join(tronque), &complet[..complet.len() - 3]).unwrap();
    ranger_apercu(&d.apercus, tronque, &image(2, 2)).unwrap();
    assert!(lister_recents(&d.recents).is_empty());
    let avant = noms_du_dossier(&d.recents);

    // Absent de la liste, apercu present : apercu parti, succes, « Recents » intact.
    supprimer_recent(&d.recents, &d.apercus, nom).expect("apercu seul");
    supprimer_recent(&d.recents, &d.apercus, tronque).expect("apercu seul");
    assert!(!d.apercus.join(nom).exists());
    assert!(!d.apercus.join(tronque).exists());
    assert_eq!(noms_du_dossier(&d.recents), avant);
    assert_eq!(fs::read(d.recents.join(majuscules)).unwrap(), complet);
    assert_eq!(
        fs::read(d.recents.join(tronque)).unwrap(),
        complet[..complet.len() - 3]
    );

    // Absent de la liste, sans apercu : succes, rien de touche.
    let absent = "20261111-194012-126.png";
    supprimer_recent(&d.recents, &d.apercus, absent).expect("rien a faire");
    assert_eq!(noms_du_dossier(&d.recents), avant);

    // Mauvais format : refus, l'apercu de ce nom reste.
    let autre = "20261111-194012-127.png";
    ranger_apercu(&d.apercus, autre, &image(2, 2)).unwrap();
    for faux in [
        majuscules,
        "20261111-194012-127.png.bak",
        "../Apercus/x.png",
        "",
    ] {
        assert!(
            supprimer_recent(&d.recents, &d.apercus, faux).is_err(),
            "{faux}"
        );
    }
    assert!(d.apercus.join(autre).is_file());
    assert_eq!(noms_du_dossier(&d.recents), avant);
}

/// Cree le lien symbolique `lien` vers `cible` avec `ln -s` (pas de `cfg` de systeme hors de
/// `systeme/`, regle d'architecture 3). Vrai si le systeme est de la famille Unix.
fn lien_symbolique(cible: &Path, lien: &Path) -> bool {
    if std::env::consts::FAMILY != "unix" {
        return false;
    }
    let fait = std::process::Command::new("ln")
        .arg("-s")
        .arg(cible)
        .arg(lien)
        .status()
        .expect("ln");
    assert!(fait.success(), "ln -s {}", lien.display());
    assert!(lien.symlink_metadata().unwrap().file_type().is_symlink());
    true
}

#[test]
fn lien_symbolique_ni_liste_ni_suivi() {
    let d = dossiers();
    // Des cibles hors de nos dossiers, au format, et un dossier au format avec un fichier.
    let nom = "20261111-194012-123.png";
    let image_cible = d.images.join(nom);
    let octets = png_de(&image(4, 4));
    fs::write(&image_cible, &octets).unwrap();
    let dossier_cible = d.images.join("20261111-194012-200.png");
    fs::create_dir(&dossier_cible).unwrap();
    fs::write(dossier_cible.join(nom), &octets).unwrap();
    // Sous Unix seulement (ce que demandait `#[cfg(unix)]`, interdit ici par la regle 3).
    if !lien_symbolique(&image_cible, &d.recents.join(nom)) {
        return;
    }
    lien_symbolique(&dossier_cible, &d.recents.join("20261111-194012-200.png"));
    lien_symbolique(&image_cible, &d.apercus.join(nom));
    lien_symbolique(&image_cible, &d.glisser.join("BreezyShot lien.png"));

    assert!(lister_recents(&d.recents).is_empty());
    // Hors liste : rien dans « Recents », et l'apercu lien n'est pas un fichier ordinaire.
    supprimer_recent(&d.recents, &d.apercus, nom).expect("hors liste");
    assert_eq!(
        vider(&d.recents, &d.apercus, &d.glisser),
        BilanVidage::default()
    );
    // Les liens restent, leurs cibles sont intactes.
    for lien in [
        d.recents.join(nom),
        d.recents.join("20261111-194012-200.png"),
        d.apercus.join(nom),
        d.glisser.join("BreezyShot lien.png"),
    ] {
        assert!(lien.symlink_metadata().unwrap().file_type().is_symlink());
    }
    assert_eq!(fs::read(&image_cible).unwrap(), octets);
    assert_eq!(fs::read(dossier_cible.join(nom)).unwrap(), octets);
}

#[test]
fn horloge_reculee_la_capture_neuve_reste_avec_son_apercu() {
    let d = dossiers();
    let mut dem = demande(d.local.parent().unwrap());
    dem.enregistrer = false;
    dem.dossier_recents = d.recents.clone();
    // 50 captures datees de 2030 : l'horloge a recule depuis.
    for i in 0..RECENTS_GARDES {
        let nom = format!("20300101-0000{i:02}-000.png");
        fs::write(d.recents.join(&nom), png_de(&image(2, 2))).unwrap();
        ranger_apercu(&d.apercus, &nom, &image(1, 1)).unwrap();
    }
    let neuf = sortir(&image(3, 3), &dem).recent.unwrap().expect("recent");
    let nom_neuf = neuf.file_name().unwrap().to_str().unwrap().to_string();
    ranger_apercu(&d.apercus, &nom_neuf, &image(1, 1)).unwrap();
    let liste = lister_recents(&d.recents);
    assert_eq!(liste.len(), RECENTS_GARDES);
    // La neuve est gardee (rangee par son nom, en dernier), la plus ancienne de 2030 partie.
    assert_eq!(liste.last().unwrap().nom, nom_neuf);
    assert!(!d.recents.join("20300101-000000-000.png").exists());
    assert!(!d.apercus.join("20300101-000000-000.png").exists());
    assert!(d.apercus.join(&nom_neuf).is_file());
    // Encore une : la precedente neuve, maintenant la plus ancienne, part avec son apercu.
    dem.maintenant = instant() + TimeDelta::seconds(1);
    let second = sortir(&image(3, 3), &dem).recent.unwrap().expect("recent");
    assert!(second.is_file());
    assert!(!neuf.exists());
    assert!(!d.apercus.join(&nom_neuf).exists());
}

// ---------- Films (0.7, chantier E) ----------

mod films {
    use super::*;
    use crate::sortie::film::{cle_film, relire_fichiers, sonder_fichiers};
    use crate::sortie::nom::chemin_libre_ext;
    use crate::sortie::{
        a_effacer, annonce_film, chemin_film, copier_film, effacer_film, enregistrer_film,
        fichier_a_glisser, films_a_retirer, nettoyer_films, sortir_film, supprimer, BilanFilm,
        FilmGarde, SorteFilm, Suppression, FILMS_AGE_MAX, FILMS_GARDES, FILMS_OCTETS_MAX,
        FILM_JEUNE, GLISSER_GARDES, SOUS_DOSSIER_FILMS,
    };

    const MO: u64 = 1024 * 1024;

    /// Le dossier local d'essai : `Films`, `Glisser`, `Recents` dessous, et une demande
    /// qui enregistre dans `<racine>/Images/BreezyShot`, sans copie.
    struct Local {
        _racine: tempfile::TempDir,
        racine: PathBuf,
        films: PathBuf,
        glisser: PathBuf,
        demande: Demande,
    }

    fn local() -> Local {
        let racine = tempfile::tempdir().expect("dossier");
        let chemin = racine.path().to_path_buf();
        let mut d = demande(&chemin);
        d.ranger_recents = false;
        Local {
            films: chemin.join("local").join(SOUS_DOSSIER_FILMS),
            glisser: chemin.join("local").join("Glisser"),
            racine: chemin,
            _racine: racine,
            demande: d,
        }
    }

    /// Un film de travail de `octets` octets reconnaissables.
    fn film(l: &Local, sorte: SorteFilm, octets: usize) -> PathBuf {
        let chemin = chemin_film(&l.films, instant(), sorte).expect("chemin");
        let contenu: Vec<u8> = (0..octets).map(|i| (i % 251) as u8).collect();
        fs::write(&chemin, contenu).expect("film");
        chemin
    }

    fn date(age: Duration) -> SystemTime {
        maintenant() - age
    }

    fn maintenant() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000)
    }

    fn garde(nom: &str, octets: u64, age: Duration) -> FilmGarde {
        FilmGarde {
            nom: nom.to_string(),
            octets,
            date: date(age),
        }
    }

    fn heures(n: u64) -> Duration {
        Duration::from_secs(n * 3600)
    }

    #[test]
    fn nom_et_extension_du_fichier_de_travail() {
        let l = local();
        let base = base_recent(instant());
        let v = chemin_film(&l.films, instant(), SorteFilm::Video).expect("vidéo");
        assert!(l.films.is_dir(), "dossier créé");
        assert_eq!(v, l.films.join(format!("{base}.mp4")));
        assert!(!v.exists(), "le fichier n'est pas créé");
        let g = chemin_film(&l.films, instant(), SorteFilm::Gif).expect("gif");
        assert_eq!(g, l.films.join(format!("{base}.gif")));
        // Pris : « _2 », comme « Récents ».
        fs::write(&v, b"x").unwrap();
        let v2 = chemin_film(&l.films, instant(), SorteFilm::Video).expect("vidéo 2");
        assert_eq!(v2, l.films.join(format!("{base}_2.mp4")));
        // La forme exacte (la base UTC de « Récents »).
        assert_eq!(cle_film(&format!("{base}.mp4")), Some(SorteFilm::Video));
        assert_eq!(cle_film(&format!("{base}_2.gif")), Some(SorteFilm::Gif));
        for faux in [
            format!("{base}.png"),
            format!("{base}.MP4"),
            format!("{base}_1.mp4"),
            format!("{base}x.mp4"),
            "film.mp4".to_string(),
            format!("{base}mp4"),
            format!("{base}..gif"),
        ] {
            assert_eq!(cle_film(&faux), None, "{faux}");
        }
        // Un dossier relatif ou vide est refusé, sans rien créer.
        assert!(chemin_film(Path::new("Films"), instant(), SorteFilm::Gif).is_err());
        assert!(chemin_film(Path::new(""), instant(), SorteFilm::Gif).is_err());
        assert!(!Path::new("Films").exists());
        assert_eq!(SorteFilm::Video.extension(), "mp4");
        assert_eq!(SorteFilm::Gif.extension(), "gif");
    }

    #[test]
    fn chemin_libre_ext_et_png_inchange() {
        let d = Path::new("/d");
        let pris = |p: &Path| {
            p == Path::new("/d/BreezyShot.mp4") || p == Path::new("/d/BreezyShot (2).mp4")
        };
        assert_eq!(
            chemin_libre_ext(d, "BreezyShot", "mp4", pris),
            PathBuf::from("/d/BreezyShot (3).mp4")
        );
        assert_eq!(
            chemin_libre_ext(d, "BreezyShot", "gif", pris),
            PathBuf::from("/d/BreezyShot.gif")
        );
        assert_eq!(
            chemin_libre(d, "BreezyShot", pris),
            PathBuf::from("/d/BreezyShot.png")
        );
        assert_eq!(
            chemin_libre_ext(d, "a", "png", |_| false),
            chemin_libre(d, "a", |_| false)
        );
    }

    #[test]
    fn films_a_retirer_dix_films() {
        // Douze petits films, de 2 h à 13 h.
        let films: Vec<FilmGarde> = (0..12)
            .map(|i| garde(&format!("f{i:02}"), MO, heures(2 + i)))
            .collect();
        // Les dix plus récents restent.
        assert_eq!(films_a_retirer(&films, maintenant(), &[]), ["f10", "f11"]);
        // L'ordre donné ne compte pas.
        let mut inverse = films.clone();
        inverse.reverse();
        assert_eq!(films_a_retirer(&inverse, maintenant(), &[]), ["f10", "f11"]);
        // Un protégé ancien reste et compte : un autre part à sa place.
        let r = films_a_retirer(&films, maintenant(), &["f11".to_string()]);
        assert_eq!(r, ["f09", "f10"]);
        assert_eq!(FILMS_GARDES, 10);
        assert!(films_a_retirer(&films[..10], maintenant(), &[]).is_empty());
    }

    #[test]
    fn films_a_retirer_deux_go() {
        // Trois films de 900 Mo : 2 700 Mo ; le plus ancien part.
        let films = vec![
            garde("a", 900 * MO, heures(2)),
            garde("b", 900 * MO, heures(3)),
            garde("c", 900 * MO, heures(4)),
        ];
        assert_eq!(films_a_retirer(&films, maintenant(), &[]), ["c"]);
        // Juste 2 Go : tout reste.
        let juste = vec![
            garde("a", FILMS_OCTETS_MAX / 2, heures(2)),
            garde("b", FILMS_OCTETS_MAX / 2, heures(3)),
        ];
        assert!(films_a_retirer(&juste, maintenant(), &[]).is_empty());
        // Un seul film de 5 Go : le plus récent reste toujours.
        let gros = vec![garde("g", 5 * FILMS_OCTETS_MAX, heures(30))];
        assert!(films_a_retirer(&gros, maintenant(), &[]).is_empty());
        // Un gros plus récent, puis un petit : le petit tient encore s'il reste de la place.
        let films = vec![
            garde("gros", FILMS_OCTETS_MAX - 10 * MO, heures(2)),
            garde("moyen", 20 * MO, heures(3)),
            garde("petit", 5 * MO, heures(4)),
        ];
        assert_eq!(films_a_retirer(&films, maintenant(), &[]), ["moyen"]);
    }

    #[test]
    fn films_a_retirer_sept_jours_et_exceptions() {
        let films = vec![
            garde("neuf", MO, heures(24 * 8)),
            garde("vieux", MO, heures(24 * 8 + 1)),
            garde("tres_vieux", MO, heures(24 * 30)),
        ];
        // Le plus récent reste même au-delà de sept jours ; les autres partent.
        assert_eq!(
            films_a_retirer(&films, maintenant(), &[]),
            ["vieux", "tres_vieux"]
        );
        // Sept jours tout juste : gardé.
        let films = vec![garde("a", MO, heures(1)), garde("b", MO, FILMS_AGE_MAX)];
        assert!(films_a_retirer(&films, maintenant(), &[]).is_empty());
        // Moins d'une heure : jamais retiré, même au-delà des bornes de nombre et de taille.
        let jeunes: Vec<FilmGarde> = (0..12)
            .map(|i| {
                garde(
                    &format!("j{i:02}"),
                    FILMS_OCTETS_MAX / 4,
                    Duration::from_secs(60 * i),
                )
            })
            .collect();
        assert!(films_a_retirer(&jeunes, maintenant(), &[]).is_empty());
        assert_eq!(FILM_JEUNE, heures(1));
        // Ils comptent pourtant : un vieux de plus part.
        let mut tous = jeunes.clone();
        tous.push(garde("ancien", MO, heures(5)));
        assert_eq!(films_a_retirer(&tous, maintenant(), &[]), ["ancien"]);
        // Protégés : le film en cours, celui de la vignette, une sortie qui tourne.
        let films = vec![
            garde("recent", MO, heures(2)),
            garde("vignette", 3 * FILMS_OCTETS_MAX, heures(24 * 9)),
        ];
        assert!(films_a_retirer(&films, maintenant(), &["vignette".into()]).is_empty());
        assert_eq!(films_a_retirer(&films, maintenant(), &[]), ["vignette"]);
        // Une date dans le futur (horloge reculée) compte comme maintenant.
        let futur = vec![
            garde("a", MO, heures(3)),
            FilmGarde {
                nom: "futur".into(),
                octets: MO,
                date: maintenant() + heures(48),
            },
        ];
        assert!(films_a_retirer(&futur, maintenant(), &[]).is_empty());
        assert!(films_a_retirer(&[], maintenant(), &[]).is_empty());
    }

    fn dater(chemin: &Path, quand: SystemTime) {
        fs::File::options()
            .write(true)
            .open(chemin)
            .expect("ouvert")
            .set_modified(quand)
            .expect("date");
    }

    #[test]
    fn nettoyage_du_dossier_cache_sur_le_disque() {
        let l = local();
        fs::create_dir_all(&l.films).unwrap();
        fs::create_dir_all(&l.glisser).unwrap();
        let ici = SystemTime::now();
        // Douze films de 2 à 13 h, un film de huit jours protégé, un étranger.
        let mut noms = Vec::new();
        for i in 0..12u64 {
            let nom = format!(
                "20261101-{:02}0000-000.{}",
                i,
                ["mp4", "gif"][i as usize % 2]
            );
            assert!(cle_film(&nom).is_some(), "{nom}");
            fs::write(l.films.join(&nom), vec![0u8; 1000]).unwrap();
            dater(&l.films.join(&nom), ici - heures(2 + i));
            noms.push(nom);
        }
        let protege = "20261101-200000-000.mp4".to_string();
        fs::write(l.films.join(&protege), vec![0u8; 10]).unwrap();
        dater(&l.films.join(&protege), ici - heures(24 * 8));
        let etranger = l.films.join("mon film.mp4");
        fs::write(&etranger, b"x").unwrap();
        dater(&etranger, ici - heures(24 * 30));
        let vieux_temp = l.films.join("20261101-000000-000.mp4.part");
        fs::write(&vieux_temp, b"x").unwrap();
        dater(&vieux_temp, ici - heures(24 * 30));
        // Glisser : des liens de films (vieux et récents) et des images.
        let lien_vieux = l.glisser.join("BreezyShot vieux.gif");
        let lien_neuf = l.glisser.join("BreezyShot neuf.mp4");
        let image_vieille = l.glisser.join("BreezyShot vieux.png");
        for (p, age) in [
            (&lien_vieux, heures(24 * 9)),
            (&lien_neuf, Duration::from_secs(60)),
            (&image_vieille, heures(24 * 9)),
        ] {
            fs::write(p, b"x").unwrap();
            dater(p, ici - age);
        }
        let (n, octets) = nettoyer_films(&l.films, &l.glisser, ici, std::slice::from_ref(&protege));
        // Onze films gardés (dix plus le protégé, qui compte) : les deux plus anciens et
        // un troisième partent.
        assert_eq!((n, octets), (3, 3000));
        for (i, nom) in noms.iter().enumerate() {
            assert_eq!(l.films.join(nom).exists(), i < 9, "{nom}");
        }
        assert!(l.films.join(&protege).exists());
        assert!(
            etranger.exists() && vieux_temp.exists(),
            "étrangers intacts"
        );
        assert!(!lien_vieux.exists(), "lien de film de neuf jours parti");
        assert!(lien_neuf.exists());
        assert!(
            image_vieille.exists(),
            "les images de Glisser ne sont pas à lui"
        );
        // Une seconde fois : rien de plus.
        assert_eq!(
            nettoyer_films(&l.films, &l.glisser, ici, &[protege]),
            (0, 0)
        );
        // Dossiers absents ou vides : rien, sans panique.
        assert_eq!(
            nettoyer_films(&l.racine.join("absent"), Path::new(""), ici, &[]),
            (0, 0)
        );
    }

    #[test]
    fn enregistrer_copie_sous_le_nom_du_modele() {
        let l = local();
        let f = film(&l, SorteFilm::Video, 300_000);
        let dossier = &l.demande.dossier_enregistrement;
        let e = enregistrer_film(&f, SorteFilm::Video, &l.demande);
        let attendu = dossier.join("BreezyShot 2026-11-11 à 19.40.12.mp4");
        assert_eq!(e, EtapeFichier::Fait(attendu.clone()));
        assert_eq!(fs::read(&attendu).unwrap(), fs::read(&f).unwrap());
        assert!(f.exists(), "une copie : le fichier de travail reste");
        // Une seconde fois : « (2) », rien d'écrasé, aucun temporaire laissé.
        let e = enregistrer_film(&f, SorteFilm::Video, &l.demande);
        assert_eq!(
            e,
            EtapeFichier::Fait(dossier.join("BreezyShot 2026-11-11 à 19.40.12 (2).mp4"))
        );
        sans_temporaire(dossier);
        // Un GIF prend son extension.
        let g = film(&l, SorteFilm::Gif, 10);
        assert_eq!(
            enregistrer_film(&g, SorteFilm::Gif, &l.demande),
            EtapeFichier::Fait(dossier.join("BreezyShot 2026-11-11 à 19.40.12.gif"))
        );
    }

    #[test]
    fn sortir_selon_la_demande() {
        let l = local();
        let f = film(&l, SorteFilm::Gif, 5000);
        let mut d = l.demande.clone();
        // Ni copie ni enregistrement : rien.
        d.enregistrer = false;
        let b = sortir_film(&f, SorteFilm::Gif, &d, false);
        assert_eq!(
            b,
            BilanFilm {
                copie: Etape::NonDemandee,
                enregistrement: EtapeFichier::NonDemande,
                lien: None,
            }
        );
        assert!(f.exists());
        assert!(!d.dossier_enregistrement.exists());
        // Enregistrer sans déplacer : une copie.
        d.enregistrer = true;
        let b = sortir_film(&f, SorteFilm::Gif, &d, false);
        let EtapeFichier::Fait(copie) = &b.enregistrement else {
            panic!("{b:?}");
        };
        assert!(f.exists() && copie.exists());
        // Jamais « Récents » (F24).
        assert!(!d.dossier_recents.exists());
    }

    #[test]
    fn deplacer_le_film_dans_le_dossier_choix_u4() {
        let l = local();
        let f = film(&l, SorteFilm::Video, 70_000);
        let contenu = fs::read(&f).unwrap();
        let b = sortir_film(&f, SorteFilm::Video, &l.demande, true);
        let attendu = l
            .demande
            .dossier_enregistrement
            .join("BreezyShot 2026-11-11 à 19.40.12.mp4");
        assert_eq!(b.enregistrement, EtapeFichier::Fait(attendu.clone()));
        assert!(!f.exists(), "déplacé : le dossier caché ne garde rien");
        assert_eq!(fs::read(&attendu).unwrap(), contenu);
        sans_temporaire(&l.demande.dossier_enregistrement);
        // Le même nom est pris : « (2) », rien d'écrasé.
        let f = film(&l, SorteFilm::Video, 10);
        let b = sortir_film(&f, SorteFilm::Video, &l.demande, true);
        assert_eq!(
            b.enregistrement,
            EtapeFichier::Fait(
                l.demande
                    .dossier_enregistrement
                    .join("BreezyShot 2026-11-11 à 19.40.12 (2).mp4")
            )
        );
        assert_eq!(fs::read(&attendu).unwrap(), contenu);
    }

    #[test]
    fn deplacer_vers_un_autre_volume_copie_puis_efface() {
        // Un autre système de fichiers que le dossier temporaire, s'il y en a un ici
        // (`/dev/shm` sous Linux) : ni lien dur ni renommage n'y passent.
        let Some(ailleurs) = ["/dev/shm"]
            .iter()
            .map(Path::new)
            .find(|p| p.is_dir())
            .and_then(|p| tempfile::tempdir_in(p).ok())
        else {
            println!("pas d'autre volume ici : saute.");
            return;
        };
        let l = local();
        let f = film(&l, SorteFilm::Gif, 123_456);
        let contenu = fs::read(&f).unwrap();
        let mut d = l.demande.clone();
        d.dossier_enregistrement = ailleurs.path().join("Images");
        let b = sortir_film(&f, SorteFilm::Gif, &d, true);
        let EtapeFichier::Fait(chemin) = &b.enregistrement else {
            panic!("{b:?}");
        };
        assert_eq!(fs::read(chemin).unwrap(), contenu);
        assert!(!f.exists());
        sans_temporaire(&d.dossier_enregistrement);
    }

    #[test]
    fn dossier_d_enregistrement_refuse() {
        let l = local();
        let f = film(&l, SorteFilm::Video, 100);
        // Relatif : refusé sans rien écrire ; le film reste dans Films (même en déplaçant).
        let mut d = l.demande.clone();
        d.dossier_enregistrement = PathBuf::from("Images/BreezyShot");
        for deplacer in [false, true] {
            let b = sortir_film(&f, SorteFilm::Video, &d, deplacer);
            assert_eq!(
                b.enregistrement,
                EtapeFichier::Echec {
                    dossier: d.dossier_enregistrement.clone(),
                    cause: "Le dossier Images › BreezyShot est introuvable.".into(),
                }
            );
            assert!(f.exists());
        }
        assert!(!Path::new("Images").exists());
        // Un fichier à la place du dossier.
        let mut d = l.demande.clone();
        fs::write(l.racine.join("bloque"), b"x").unwrap();
        d.dossier_enregistrement = l.racine.join("bloque").join("BreezyShot");
        let b = sortir_film(&f, SorteFilm::Video, &d, true);
        assert!(
            matches!(b.enregistrement, EtapeFichier::Echec { .. }),
            "{b:?}"
        );
        assert!(f.exists());
        // Le film a disparu : échec dit, sans panique.
        let absent = l.films.join("20261111-000000-000.mp4");
        let mut d = l.demande.clone();
        d.copier = true;
        let b = sortir_film(&absent, SorteFilm::Video, &d, true);
        assert_eq!(
            b.copie,
            Etape::Echec("Le fichier du film est introuvable.".into())
        );
        assert!(matches!(b.enregistrement, EtapeFichier::Echec { .. }));
        assert_eq!(b.lien, None);
    }

    #[test]
    fn copier_pose_un_lien_au_nom_du_modele() {
        let _seul = PRESSE_PAPIERS_TESTS
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let l = local();
        let f = film(&l, SorteFilm::Gif, 4000);
        let (etape, lien) = copier_film(&f, SorteFilm::Gif, &l.demande);
        let lien = lien.expect("lien dans Glisser");
        assert_eq!(lien, l.glisser.join("BreezyShot 2026-11-11 à 19.40.12.gif"));
        assert_eq!(fs::read(&lien).unwrap(), fs::read(&f).unwrap());
        // Ici, le presse-papiers peut manquer : l'étape le dit, le lien est là quand même.
        // Comme les images : obligatoire avec `BREEZYSHOT_PRESSE_PAPIERS_OBLIGATOIRE=1`.
        match sonder_fichiers() {
            Ok(()) => {
                assert_eq!(etape, Etape::Faite);
                assert_eq!(
                    relire_fichiers().expect("relu"),
                    std::slice::from_ref(&lien)
                );
            }
            Err(e) => {
                let obligatoire =
                    std::env::var("BREEZYSHOT_PRESSE_PAPIERS_OBLIGATOIRE").as_deref() == Ok("1");
                assert!(!obligatoire, "presse-papiers obligatoire mais absent ({e})");
                assert!(matches!(etape, Etape::Echec(_)), "{etape:?}");
                println!("copier_pose_un_lien_au_nom_du_modele : pas de presse-papiers ici.");
            }
        }
        // Le lien garde les octets après le déplacement du film (choix U4).
        let mut d = l.demande.clone();
        d.copier = true;
        let b = sortir_film(&f, SorteFilm::Gif, &d, true);
        let second = b.lien.expect("lien");
        assert_eq!(
            second,
            l.glisser.join("BreezyShot 2026-11-11 à 19.40.12 (2).gif")
        );
        assert!(!f.exists());
        assert_eq!(fs::read(&second).unwrap().len(), 4000);
        // Sans « Recents » utilisable, pas de Glisser : le fichier lui-même est donné.
        let mut d = l.demande.clone();
        d.dossier_recents = PathBuf::new();
        let g = film(&l, SorteFilm::Video, 10);
        let (_, lien) = copier_film(&g, SorteFilm::Video, &d);
        assert_eq!(lien, None);
    }

    #[test]
    fn glisser_garde_l_extension_et_ne_compte_pas_les_films() {
        let l = local();
        let f = film(&l, SorteFilm::Video, 10);
        let a = fichier_a_glisser(&f, &l.glisser, "Film").expect("lien");
        assert_eq!(a, l.glisser.join("Film.mp4"));
        let b = fichier_a_glisser(&f, &l.glisser, "Film").expect("lien");
        assert_eq!(b, l.glisser.join("Film (2).mp4"));
        // Beaucoup d'images ensuite : la borne des .png ne retire aucun lien de film.
        let rec = l.racine.join("local").join("Recents");
        fs::create_dir_all(&rec).unwrap();
        let r = rec.join("20261111-194012-123.png");
        fs::write(&r, b"png").unwrap();
        for i in 0..(GLISSER_GARDES + 3) {
            fichier_a_glisser(&r, &l.glisser, &format!("img{i}")).expect("png");
        }
        assert!(a.exists() && b.exists());
        // Sans extension : `.png`, comme avant.
        let sans = l.racine.join("sans");
        fs::write(&sans, b"x").unwrap();
        assert_eq!(
            fichier_a_glisser(&sans, &l.glisser, "s").unwrap(),
            l.glisser.join("s.png")
        );
    }

    #[test]
    fn supprimer_un_film_efface_ses_fichiers() {
        let rec = Path::new("/l/Recents");
        let gli = Path::new("/l/Glisser");
        let fil = Path::new("/l/Films");
        let enregistre = std::env::temp_dir()
            .join("BreezyShot")
            .join("BreezyShot 2026-11-11 à 19.40.12.mp4");
        let s = Suppression {
            recent: None,
            enregistre: Some(enregistre.clone()),
            liens: vec![
                gli.join("BreezyShot 2026-11-11 à 19.40.12.mp4"),
                gli.join("BreezyShot 2026-11-11 à 19.40.12.gif"),
                gli.join("x.txt"),
                PathBuf::from("/ailleurs/x.mp4"),
            ],
            film: Some(fil.join("20261111-184012-123.mp4")),
        };
        assert_eq!(
            a_effacer(&s, rec, gli, fil),
            [
                fil.join("20261111-184012-123.mp4"),
                enregistre.clone(),
                gli.join("BreezyShot 2026-11-11 à 19.40.12.mp4"),
                gli.join("BreezyShot 2026-11-11 à 19.40.12.gif"),
            ]
        );
        // Un .mp4 hors de Films, ou mal nommé, ou un enregistré relatif : rien.
        for film in [
            PathBuf::from("/ailleurs/20261111-184012-123.mp4"),
            fil.join("mon film.mp4"),
            fil.join("20261111-184012-123.png"),
            fil.join("20261111-184012-123.mov"),
        ] {
            let s = Suppression {
                film: Some(film.clone()),
                enregistre: Some(PathBuf::from("relatif.mp4")),
                ..Suppression::default()
            };
            assert!(a_effacer(&s, rec, gli, fil).is_empty(), "{film:?}");
        }
        // Sans dossier des films : le fichier de travail n'est jamais effacé.
        let s = Suppression {
            film: Some(PathBuf::from("20261111-184012-123.mp4")),
            ..Suppression::default()
        };
        assert!(a_effacer(&s, rec, gli, Path::new("")).is_empty());
        // Un enregistré d'une autre sorte n'est pas pris.
        let s = Suppression {
            enregistre: Some(enregistre.with_extension("mov")),
            ..Suppression::default()
        };
        assert!(a_effacer(&s, rec, gli, fil).is_empty());
    }

    #[test]
    fn supprimer_un_film_sur_le_disque() {
        let l = local();
        let mut d = l.demande.clone();
        d.copier = true;
        let f = film(&l, SorteFilm::Gif, 100);
        // Lien de glisser posé avant (comme la copie), puis enregistrement sans déplacer.
        let lien = fichier_a_glisser(&f, &l.glisser, "BreezyShot").expect("lien");
        let EtapeFichier::Fait(enregistre) = enregistrer_film(&f, SorteFilm::Gif, &d) else {
            panic!("enregistrement");
        };
        let autre = l.glisser.join("autre.png");
        fs::write(&autre, b"x").unwrap();
        let s = Suppression {
            recent: None,
            enregistre: Some(enregistre.clone()),
            liens: vec![lien.clone()],
            film: Some(f.clone()),
        };
        let rec = l.racine.join("local").join("Recents");
        let r = supprimer(&s, &rec, &l.glisser, &l.films);
        assert_eq!(r.len(), 3);
        assert!(r.iter().all(|(_, r)| r.is_ok()));
        assert!(!f.exists() && !lien.exists() && !enregistre.exists());
        assert!(autre.exists());
        // Choix U4 : le fichier de travail déplacé n'est plus là ; compté comme effacé.
        assert!(supprimer(&s, &rec, &l.glisser, &l.films)
            .iter()
            .all(|(_, r)| r.is_ok()));
    }

    #[test]
    fn effacer_le_film_annule() {
        let l = local();
        let f = film(&l, SorteFilm::Video, 10);
        // Hors de Films, ou mal nommé : jamais effacé.
        let ailleurs = l.racine.join("20261111-184012-123.mp4");
        fs::write(&ailleurs, b"x").unwrap();
        effacer_film(&ailleurs);
        assert!(ailleurs.exists());
        let etranger = l.films.join("garder.mp4");
        fs::write(&etranger, b"x").unwrap();
        effacer_film(&etranger);
        assert!(etranger.exists());
        effacer_film(Path::new("Films/20261111-184012-123.mp4"));
        effacer_film(&f);
        assert!(!f.exists());
        // Déjà absent : rien, sans panique.
        effacer_film(&f);
    }

    fn bilan_film(copie: Etape, enregistrement: EtapeFichier) -> BilanFilm {
        BilanFilm {
            copie,
            enregistrement,
            lien: None,
        }
    }

    #[test]
    fn annonces_accordees() {
        let d = demande_annonce();
        let v = SorteFilm::Video;
        let g = SorteFilm::Gif;
        let f = || EtapeFichier::Fait(PathBuf::from("f.mp4"));
        let cas = [
            (
                Etape::Faite,
                f(),
                "Copiée et enregistrée",
                "Copié et enregistré",
            ),
            (
                Etape::Faite,
                EtapeFichier::NonDemande,
                "Copiée dans le presse-papiers",
                "Copié dans le presse-papiers",
            ),
            (
                Etape::NonDemandee,
                f(),
                "Enregistrée dans Images › BreezyShot",
                "Enregistré dans Images › BreezyShot",
            ),
            (
                Etape::NonDemandee,
                EtapeFichier::NonDemande,
                "Enregistrement terminé",
                "Enregistrement terminé",
            ),
        ];
        for (copie, fichier, video, gif) in cas {
            let b = bilan_film(copie, fichier);
            assert_eq!(annonce_film(&b, &d, v), reussite(video));
            assert_eq!(annonce_film(&b, &d, g), reussite(gif));
        }
        // Les erreurs gardent leurs textes, pour les deux sortes.
        let cause = "Le presse-papiers est occupé par une autre application.";
        for s in [v, g] {
            let b = bilan_film(Etape::Faite, echec_fichier());
            assert_eq!(
                annonce_film(&b, &d, s),
                erreur(
                    "Impossible d'enregistrer",
                    "Le dossier Images › BreezyShot est introuvable."
                )
            );
            let b = bilan_film(Etape::Echec(cause.into()), EtapeFichier::NonDemande);
            assert_eq!(
                annonce_film(&b, &d, s),
                erreur("Impossible de copier", cause)
            );
            let b = bilan_film(Etape::Echec(cause.into()), f());
            assert_eq!(
                annonce_film(&b, &d, s),
                annonce(
                    &Bilan {
                        copie: Etape::Echec(cause.into()),
                        recent: None,
                        enregistrement: f(),
                    },
                    &d
                )
            );
        }
    }

    #[test]
    fn grand_film_copie_sans_tout_lire_en_memoire() {
        // 64 Mo copiés : la plus grosse allocation reste celle d'un tampon de copie.
        let l = local();
        let f = chemin_film(&l.films, instant(), SorteFilm::Video).unwrap();
        {
            use std::io::Write;
            let mut w = fs::File::create(&f).unwrap();
            let bloc = vec![7u8; MO as usize];
            for _ in 0..64 {
                w.write_all(&bloc).unwrap();
            }
        }
        let (e, plus_grosse) =
            plus_grosse_allocation(|| enregistrer_film(&f, SorteFilm::Video, &l.demande));
        let EtapeFichier::Fait(chemin) = e else {
            panic!("{e:?}");
        };
        assert_eq!(fs::metadata(&chemin).unwrap().len(), 64 * MO);
        assert!(
            plus_grosse < MO as usize,
            "plus grosse allocation {plus_grosse}"
        );
    }
}
