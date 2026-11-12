//! Câblage de l'installateur et de la désinstallation (contrat §3). Les décisions sont
//! dans `crate::installation` (fonctions pures testées) ; ici, les gestes : Tauri, les
//! fils, le journal, les appels à `systeme::installation`. Aucune décision n'est prise ici.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, RunEvent};

use crate::fenetres::installateur::{EtatPage, Installateur, Signal};
use crate::installation::emplacements::Retrait;
use crate::installation::parcours::{
    besoin_octets, verifier_place, Action, ErreurInstallation, Etape, EtatInstallateur, Geste,
    ATTENTE_REMPLACEMENT, DELAI_ARRET, DELAI_LANCEMENT, ETAPES, PAS_PROGRESSION,
};
use crate::installation::{
    self, emplacements, envol, nom_verrou_app, place_fenetre, reagir, Emplacements, Plan, Trouvee,
    Version, IDENTIFIANT, VERROU_INSTALLATEUR,
};
use crate::systeme::installation as sys;
use crate::systeme::installation::{ErreurFichier, Valeur, Verrou};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// L'état partagé pendant l'installation (fil principal et fil d'installation).
struct Etat {
    emplacements: Emplacements,
    plan: Plan,
    /// Taille de l'exe en cours (ce qu'on va copier).
    taille: u64,
    etat: Mutex<EtatInstallateur>,
    /// Vrai une fois l'app lancée : la fermeture peut alors sortir.
    app_lancee: AtomicBool,
    /// Code de sortie, posé avant `app.exit`.
    code: AtomicI32,
}

/// Lance l'installateur (contrat §3.2). Rend le code de sortie du processus.
pub fn lancer_installateur(contexte: tauri::Context) -> i32 {
    let essai = std::env::var_os(crate::systeme::VARIABLE_ESSAI).map(PathBuf::from);
    let connus = sys::dossiers_connus();
    let emplacements = match emplacements(essai.as_deref(), &connus) {
        Ok(e) => e,
        Err(_) => {
            eprintln!("BreezyShot : dossier des applications introuvable");
            return 1;
        }
    };
    if let Err(e) = crate::application::journal::installer_installation(&emplacements.journal) {
        eprintln!("BreezyShot : journal d'installation indisponible : {e}");
    }
    log::info!("installateur BreezyShot {VERSION}");
    match crate::systeme::version_windows() {
        Some(build) => log::info!("systeme windows build {build}"),
        None => log::info!("systeme {}", std::env::consts::OS),
    }

    // Un seul installateur à la fois.
    let verrou = match sys::prendre_verrou(VERROU_INSTALLATEUR) {
        Some(v) => v,
        None => {
            log::info!("seconde installation refusee");
            return 0;
        }
    };

    // WebView2 : l'écran lui-même en a besoin.
    match sys::version_webview2() {
        Some(v) => log::info!("installateur: webview2 {v}"),
        None => {
            log::info!("installateur: webview2 absent");
            sys::message_bloquant(
                "Installer BreezyShot",
                "BreezyShot a besoin de Microsoft Edge WebView2, absent de ce PC.",
            );
            return 1;
        }
    }

    nettoyer_webview(&emplacements);

    let taille = match crate::systeme::installation::exe_courant()
        .ok()
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
    {
        Some(t) => t,
        None => {
            log::warn!("installateur: taille de l'exe inconnue");
            0
        }
    };

    let trouvee = etat_trouve(&emplacements);
    log::info!("installation trouvee: {}", trouvee.ligne());
    let courante = Version::lire(VERSION).unwrap_or(Version {
        majeure: 0,
        mineure: 0,
        correctif: 0,
    });
    let plan = installation::decider_plan(courante, &trouvee);
    log::info!("plan: {}", plan.ligne());

    let depart = if plan.copie() {
        EtatInstallateur::Accueil
    } else {
        EtatInstallateur::Pret
    };
    let etat = Arc::new(Etat {
        emplacements,
        plan,
        taille,
        etat: Mutex::new(depart),
        app_lancee: AtomicBool::new(false),
        code: AtomicI32::new(0),
    });

    let (largeur, hauteur) = taille_fenetre();
    let pour_setup = Arc::clone(&etat);
    let construction = tauri::Builder::default()
        .manage(Arc::clone(&etat))
        .invoke_handler(tauri::generate_handler![
            crate::fenetres::installateur::installateur_pret,
            crate::fenetres::installateur::installateur_action
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            let pour_rappel = handle.clone();
            let fenetre = Installateur::creer(
                &handle,
                &pour_setup.emplacements.webview_installateur,
                largeur,
                hauteur,
                move |s| traiter(&pour_rappel, s),
            )?;
            app.manage(fenetre);
            if let Some((rect, echelle)) = sys::zone_utile_sous_souris() {
                placer(&handle, rect, echelle);
            }
            app.state::<Installateur>().montrer();
            Ok(())
        })
        .build(contexte);

    let _verrou: Verrou = verrou; // tenu toute la vie de l'installateur
    match construction {
        Ok(app) => app.run_return(move |app, evenement| {
            if let RunEvent::ExitRequested {
                code: None, api, ..
            } = &evenement
            {
                api.prevent_exit();
            } else if let RunEvent::Exit = evenement {
                if let Some(f) = app.try_state::<Installateur>() {
                    f.fermer();
                }
            }
        }),
        Err(e) => {
            log::error!("installateur: construction impossible: {e}");
            1
        }
    }
}

/// Taille de la fenêtre : carte + marge d'ombre, en pixels logiques.
fn taille_fenetre() -> (f64, f64) {
    (
        envol::CARTE.0 + 2.0 * envol::MARGE,
        envol::CARTE.1 + 2.0 * envol::MARGE,
    )
}

/// Place et redimensionne la fenêtre pour l'écran de la souris.
fn placer(app: &AppHandle, rect: crate::systeme::Rect, echelle: f64) {
    let voulu = place_fenetre(rect, echelle);
    if let Some(f) = app.try_state::<Installateur>() {
        if let Some(id) = f.identifiant() {
            sys::deplacer_fenetre(id, voulu.x, voulu.y);
        }
    }
}

/// Ce qu'on trouve sur la machine : la clé de désinstallation et l'exe installé.
fn etat_trouve(e: &Emplacements) -> Trouvee {
    let version = sys::lire_texte(&e.cle_desinstallation, "DisplayVersion");
    let cle = version.is_some();
    let exe_present = e.exe.is_file();
    if !cle && !exe_present {
        Trouvee::Aucune
    } else {
        Trouvee::Installee {
            cle,
            version: version.as_deref().and_then(Version::lire),
            exe_present,
        }
    }
}

/// Nettoie le dossier WebView2 de l'installateur s'il ne contient que ses données (reste
/// d'une installation précédente ; pendant que l'installateur tourne, WebView2 le tient).
fn nettoyer_webview(e: &Emplacements) {
    let dossier = &e.webview_installateur;
    if dossier.is_dir() && installation::dossier_webview_a_nous(&sys::entrees(dossier)) {
        let _ = sys::supprimer_dossier_webview(dossier);
    }
}

fn etat_courant(etat: &Etat) -> EtatInstallateur {
    *etat.etat.lock().unwrap_or_else(|e| e.into_inner())
}

fn poser_etat(etat: &Etat, nouvel: EtatInstallateur) {
    *etat.etat.lock().unwrap_or_else(|e| e.into_inner()) = nouvel;
}

/// Réagit à un signal de la page (fil principal).
fn traiter(app: &AppHandle, signal: Signal) {
    let etat = match app.try_state::<Arc<Etat>>() {
        Some(e) => Arc::clone(&e),
        None => return,
    };
    match signal {
        Signal::Pret => {
            log::info!("installateur pret");
            envoyer_etat(app, &etat);
        }
        Signal::Action(nom) => {
            let Some(action) = Action::depuis_nom(&nom) else {
                log::warn!("installateur action inconnue: {nom}");
                return;
            };
            log::info!("installateur action {}", action.nom());
            agir(app, &etat, action);
        }
        Signal::FermetureDemandee => agir(app, &etat, Action::Fermer),
    }
}

/// Envoie à la page l'état courant (accueil, erreur, prêt ; `installation` n'est pas envoyé,
/// la page y passe seule au clic « Installer »).
fn envoyer_etat(app: &AppHandle, etat: &Etat) {
    let Some(fenetre) = app.try_state::<Installateur>() else {
        return;
    };
    let courant = etat_courant(etat);
    let page = match courant {
        EtatInstallateur::Pret => EtatPage {
            etat: "pret",
            cause: String::new(),
            total: etat.taille,
            direct: !etat.plan.copie(),
        },
        _ => EtatPage {
            etat: "accueil",
            cause: String::new(),
            total: etat.taille,
            direct: false,
        },
    };
    fenetre.envoyer(page);
    // Prêt d'emblée (plan « plus récente installée ») : on lance l'app si besoin.
    if matches!(courant, EtatInstallateur::Pret) && !etat.plan.copie() {
        lancer_app_installee(app, etat);
    }
}

fn agir(app: &AppHandle, etat: &Arc<Etat>, action: Action) {
    let Some((nouvel, geste)) = reagir(etat_courant(etat), action) else {
        log::info!(
            "action ignoree: {} en {}",
            action.nom(),
            etat_courant(etat).nom()
        );
        return;
    };
    poser_etat(etat, nouvel);
    match geste {
        Geste::Installer => lancer_installation(app, etat),
        Geste::Capturer => essayer_capture(app, etat),
        Geste::Envol => voler(app, etat),
        Geste::FermerEnFondu => fermer_en_fondu(app, etat),
        Geste::CacherPuisFinir => cacher_pendant_installation(app, etat),
        Geste::Reduire => {
            if let Some(f) = app.try_state::<Installateur>() {
                f.reduire();
            }
            // `reduire` ne change pas l'état : on le remet.
            poser_etat(etat, etat_courant(etat));
        }
    }
}

/// Lance les étapes dans un fil d'installation.
fn lancer_installation(app: &AppHandle, etat: &Arc<Etat>) {
    let app = app.clone();
    let etat = Arc::clone(etat);
    let lance = std::thread::Builder::new()
        .name("installation".into())
        .spawn(move || {
            let debut = Instant::now();
            match installer(&app, &etat) {
                Ok(()) => {
                    log::info!(
                        "installation finie {VERSION} en {} ms",
                        debut.elapsed().as_millis()
                    );
                    finir(&app, &etat);
                }
                Err(e) => {
                    log::info!("installation erreur: {}", e.code());
                    poser_etat(&etat, EtatInstallateur::Erreur);
                    if let Some(f) = app.try_state::<Installateur>() {
                        f.envoyer(EtatPage {
                            etat: "erreur",
                            cause: e.cause(),
                            total: etat.taille,
                            direct: false,
                        });
                    }
                }
            }
        });
    if let Err(e) = lance {
        log::error!("installation: fil impossible: {e}");
    }
}

/// Les étapes (contrat §3.2), chacune reportée au journal.
fn installer(app: &AppHandle, etat: &Etat) -> Result<(), ErreurInstallation> {
    let e = &etat.emplacements;
    for etape in ETAPES {
        match etape {
            Etape::Place => {
                let besoin = besoin_octets(etat.taille);
                let libre = sys::espace_libre(&e.dossier);
                let lettre = sys::lettre_volume(&e.dossier);
                log::info!(
                    "etape place: libre {} Mo besoin {} Mo",
                    libre.map(mo).unwrap_or(0),
                    mo(besoin)
                );
                verifier_place(libre, besoin, lettre.as_deref())?;
            }
            Etape::Arret => arreter_app(etat)?,
            Etape::Copie => copier(app, etat)?,
            Etape::Verification => {
                let source = sys::exe_courant().map_err(|_| ErreurInstallation::Copie)?;
                let ok = sys::identiques(&source, &e.exe).map_err(de_fichier)?;
                log::info!(
                    "etape verification: {}",
                    if ok { "ok" } else { "differente" }
                );
                if !ok {
                    return Err(ErreurInstallation::Copie);
                }
            }
            Etape::Raccourci => {
                let fait = sys::creer_raccourci(&e.raccourci, &e.exe)
                    .map_err(|_| ErreurInstallation::Raccourci)?;
                log::info!(
                    "etape raccourci: {}",
                    if fait { "fait" } else { "sans objet" }
                );
            }
            Etape::Inscription => inscrire(etat)?,
            Etape::Lancement => lancer_etape(app, etat)?,
        }
    }
    Ok(())
}

fn arreter_app(etat: &Etat) -> Result<(), ErreurInstallation> {
    let verrou = nom_verrou_app(IDENTIFIANT);
    if !sys::verrou_existe(&verrou) {
        log::info!("etape arret: aucune instance");
        return Ok(());
    }
    // On demande l'arrêt à l'instance, par n'importe quel breezyshot : le nôtre (installé)
    // s'il est là, sinon celui qu'on va installer n'existe pas encore — on utilise l'exe
    // en cours (le téléchargé), qui comprend `--quitter` comme toute version.
    let qui = etat.emplacements.exe.clone();
    let qui = if qui.is_file() {
        qui
    } else {
        sys::exe_courant().unwrap_or(qui)
    };
    let debut = Instant::now();
    let _ = sys::lancer(&qui, &["--quitter"]);
    while debut.elapsed() < DELAI_ARRET {
        if !sys::verrou_existe(&verrou) {
            log::info!(
                "etape arret: instance arretee en {} ms",
                debut.elapsed().as_millis()
            );
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    log::info!("etape arret: instance toujours la");
    Err(ErreurInstallation::Ouvert)
}

fn copier(app: &AppHandle, etat: &Etat) -> Result<(), ErreurInstallation> {
    let e = &etat.emplacements;
    let source = sys::exe_courant().map_err(|_| ErreurInstallation::Ecriture)?;
    let app = app.clone();
    let total = etat.taille;
    let mut dernier = Instant::now()
        .checked_sub(PAS_PROGRESSION)
        .unwrap_or_else(Instant::now);
    let debut = Instant::now();
    let octets = sys::copier(&source, &e.nouveau, &mut |fait| {
        if dernier.elapsed() >= PAS_PROGRESSION {
            dernier = Instant::now();
            if let Some(f) = app.try_state::<Installateur>() {
                f.progression(fait, total);
            }
        }
    })
    .map_err(de_fichier)?;
    if let Some(f) = app.try_state::<Installateur>() {
        f.progression(octets, total);
    }
    sys::remplacer(&e.nouveau, &e.exe, ATTENTE_REMPLACEMENT).map_err(de_fichier)?;
    log::info!(
        "etape copie: {octets} octets en {} ms",
        debut.elapsed().as_millis()
    );
    Ok(())
}

fn inscrire(etat: &Etat) -> Result<(), ErreurInstallation> {
    let e = &etat.emplacements;
    let date = chrono::Local::now().format("%Y%m%d").to_string();
    let valeurs = installation::valeurs_desinstallation(e, VERSION, etat.taille, &date);
    let refs: Vec<(&str, Valeur)> = valeurs.iter().map(|(n, v)| (*n, v.clone())).collect();
    let fait = sys::ecrire_cle(&e.cle_desinstallation, &refs)
        .map_err(|_| ErreurInstallation::Inscription)?;
    log::info!(
        "etape inscription: {}",
        if fait { "faite" } else { "sans objet" }
    );
    Ok(())
}

fn lancer_etape(app: &AppHandle, etat: &Etat) -> Result<(), ErreurInstallation> {
    sys::permettre_premier_plan();
    sys::lancer(&etat.emplacements.exe, &[]).map_err(|_| ErreurInstallation::Lancement)?;
    let verrou = nom_verrou_app(IDENTIFIANT);
    let debut = Instant::now();
    while debut.elapsed() < DELAI_LANCEMENT {
        if sys::verrou_existe(&verrou) {
            log::info!(
                "etape lancement: instance en {} ms",
                debut.elapsed().as_millis()
            );
            etat.app_lancee.store(true, Ordering::SeqCst);
            let _ = app;
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(ErreurInstallation::Lancement)
}

/// Après les étapes : montrer l'écran « prêt » ; si la fermeture a été demandée pendant
/// l'installation, sortir.
fn finir(app: &AppHandle, etat: &Etat) {
    if matches!(etat_courant(etat), EtatInstallateur::Fermeture) {
        sortir(app, etat, 0);
        return;
    }
    poser_etat(etat, EtatInstallateur::Pret);
    if let Some(f) = app.try_state::<Installateur>() {
        f.envoyer(EtatPage {
            etat: "pret",
            cause: String::new(),
            total: etat.taille,
            direct: false,
        });
    }
}

/// « Essayer une capture » : demande une capture de zone à l'app installée.
fn essayer_capture(app: &AppHandle, etat: &Etat) {
    lancer_app_installee(app, etat);
    sys::permettre_premier_plan();
    let _ = sys::lancer(&etat.emplacements.exe, &["--capturer", "zone"]);
    log::info!("essayer: capture zone demandee");
}

/// Lance l'app installée si elle ne tourne pas (plan « plus récente installée », ou
/// « essayer » après une fermeture).
fn lancer_app_installee(app: &AppHandle, etat: &Etat) {
    if etat.app_lancee.swap(true, Ordering::SeqCst) {
        return;
    }
    if !sys::verrou_existe(&nom_verrou_app(IDENTIFIANT)) {
        let _ = sys::lancer(&etat.emplacements.exe, &[]);
    }
    let _ = app;
}

/// Fermeture pendant l'installation : la fenêtre disparaît, l'installation continue.
fn cacher_pendant_installation(app: &AppHandle, _etat: &Etat) {
    if let Some(f) = app.try_state::<Installateur>() {
        f.cacher();
    }
}

/// Fermeture depuis l'accueil ou l'erreur : fondu puis sortie.
fn fermer_en_fondu(app: &AppHandle, etat: &Etat) {
    if let Some(f) = app.try_state::<Installateur>() {
        f.fermer_en_fondu();
    }
    attendre_puis_sortir(app, etat, Duration::from_millis(200), 0);
}

/// Envol de la fenêtre vers la zone de notification (contrat §4.5), puis sortie.
fn voler(app: &AppHandle, etat: &Etat) {
    let reduit = crate::systeme::animations_reduites();
    let (rect, echelle) = sys::zone_utile_sous_souris().unwrap_or((
        crate::systeme::Rect {
            x: 0,
            y: 0,
            largeur: 1024,
            hauteur: 768,
        },
        1.0,
    ));
    let depart = place_fenetre(rect, echelle);
    let cible = installation::cible_envol(sys::zone_de_notification(), rect, echelle);
    if reduit {
        if let Some(f) = app.try_state::<Installateur>() {
            f.fermer_en_fondu();
        }
        attendre_puis_sortir(app, etat, Duration::from_millis(160), 0);
        return;
    }
    let identifiant = app
        .try_state::<Installateur>()
        .and_then(|f| f.identifiant());
    if let Some(f) = app.try_state::<Installateur>() {
        f.envol(envol::DUREE_ENVOL.as_millis() as u64);
    }
    let cle = match sys::zone_de_notification() {
        Some(_) => "notification",
        None => "coin",
    };
    let secours = app.clone();
    let app = app.clone();
    let debut = Instant::now();
    let lance = std::thread::Builder::new()
        .name("envol".into())
        .spawn(move || {
            loop {
                let t = debut.elapsed().as_secs_f64() / envol::DUREE_ENVOL.as_secs_f64();
                let (x, y) = installation::position_envol(depart, cible, t.min(1.0), echelle);
                if let Some(id) = identifiant {
                    sys::deplacer_fenetre(id, x, y);
                }
                if t >= 1.0 {
                    break;
                }
                std::thread::sleep(envol::PAS_ENVOL);
            }
            log::info!("envol cible {cle} en {} ms", debut.elapsed().as_millis());
            let pour = app.clone();
            let _ = app.run_on_main_thread(move || {
                if let Some(etat) = pour.try_state::<Arc<Etat>>() {
                    let etat = Arc::clone(&etat);
                    sortir(&pour, &etat, 0);
                }
            });
        });
    if let Err(e) = lance {
        log::warn!("envol impossible ({e}): fermeture directe");
        sortir(&secours, etat, 0);
    }
}

fn attendre_puis_sortir(app: &AppHandle, etat: &Etat, delai: Duration, code: i32) {
    let secours = app.clone();
    let app = app.clone();
    let lance = std::thread::Builder::new()
        .name("fermeture".into())
        .spawn(move || {
            std::thread::sleep(delai);
            let pour = app.clone();
            let _ = app.run_on_main_thread(move || {
                if let Some(etat) = pour.try_state::<Arc<Etat>>() {
                    let etat = Arc::clone(&etat);
                    sortir(&pour, &etat, code);
                }
            });
        });
    if lance.is_err() {
        sortir(&secours, etat, code);
    }
}

/// Sort proprement : ferme la fenêtre, pose le code, quitte la boucle.
fn sortir(app: &AppHandle, etat: &Etat, code: i32) {
    etat.code.store(code, Ordering::SeqCst);
    log::info!("installateur ferme en {}", etat_courant(etat).nom());
    crate::application::journal::vider();
    app.exit(code);
}

fn mo(octets: u64) -> u64 {
    octets / (1 << 20)
}

fn de_fichier(e: ErreurFichier) -> ErreurInstallation {
    match e {
        ErreurFichier::Pris => ErreurInstallation::Ecriture,
        ErreurFichier::Refuse(_) => ErreurInstallation::Ecriture,
    }
}

// --- Désinstallation (contrat §3.6) -----------------------------------------------------

/// Fermeture « retirer le démarrage avec Windows », injectée par l'Application à
/// l'assemblage. Par défaut : rien (la valeur `Run` appartient au chantier des Réglages).
pub type RetirerDemarrage = Box<dyn FnOnce() -> Result<(), String>>;

/// Désinstalle BreezyShot, sans fenêtre ni question (contrat §3.6). Rend le code de sortie.
pub fn desinstaller() -> i32 {
    desinstaller_avec(Box::new(|| {
        // Geste vide documenté : l'Application le remplace à l'assemblage par le retrait de
        // la valeur `Run` (chantier des Réglages).
        log::info!("desinstallation demarrage: pas a nous");
        Ok(())
    }))
}

/// Cœur de la désinstallation, avec la fermeture « retirer le démarrage » injectée.
pub fn desinstaller_avec(retirer_demarrage: RetirerDemarrage) -> i32 {
    let essai = std::env::var_os(crate::systeme::VARIABLE_ESSAI).map(PathBuf::from);
    let connus = sys::dossiers_connus();
    let e = match emplacements(essai.as_deref(), &connus) {
        Ok(e) => e,
        Err(_) => {
            eprintln!("BreezyShot : dossier des applications introuvable");
            return 1;
        }
    };
    let _ = crate::application::journal::installer_installation(&e.journal);
    log::info!("desinstallation BreezyShot {VERSION}");

    // Arrêt de l'app en cours (jamais tuée).
    let verrou = nom_verrou_app(IDENTIFIANT);
    if sys::verrou_existe(&verrou) {
        let _ = sys::lancer(&e.exe, &["--quitter"]);
        let debut = Instant::now();
        while debut.elapsed() < DELAI_ARRET && sys::verrou_existe(&verrou) {
            std::thread::sleep(Duration::from_millis(100));
        }
        if sys::verrou_existe(&verrou) {
            log::info!("desinstallation: instance toujours la");
        } else {
            log::info!("desinstallation: instance arretee");
        }
    } else {
        log::info!("desinstallation: aucune instance");
    }

    let processus = std::process::id();
    for retrait in installation::retraits(&e, processus) {
        appliquer_retrait(retrait);
    }
    // Démarrage avec Windows : la fermeture injectée écrit sa propre ligne
    // (`desinstallation demarrage: retire|absent`). Par défaut (geste vide, chantier des
    // Réglages non encore raccordé), elle écrit `pas a nous`.
    if let Err(e) = retirer_demarrage() {
        log::warn!("desinstallation demarrage: {e}");
    }
    log::info!("desinstallation finie");
    crate::application::journal::vider();
    0
}

fn appliquer_retrait(retrait: Retrait) {
    match retrait {
        Retrait::Raccourci(lien) => {
            let avait = sys::cible_raccourci(&lien).is_some();
            let _ = sys::supprimer_fichier(&lien);
            log::info!(
                "desinstallation raccourci: {}",
                si(avait, "retire", "absent")
            );
        }
        Retrait::Cle { cle, valeurs } => match sys::retirer_valeurs_et_cle(&cle, valeurs) {
            Ok(true) => log::info!("desinstallation inscription: retiree"),
            Ok(false) => log::info!("desinstallation inscription: absente"),
            Err(e) => log::warn!("desinstallation inscription: {e}"),
        },
        Retrait::Fichier(p) => {
            let _ = sys::supprimer_fichier(&p);
        }
        Retrait::ExeDeplace { de, vers } => {
            if de.is_file() {
                match sys::deplacer(&de, &vers) {
                    Ok(()) => log::info!("desinstallation exe: deplace"),
                    Err(_) => log::info!("desinstallation exe: reste"),
                }
            } else {
                log::info!("desinstallation exe: reste");
            }
        }
        Retrait::DossierSiVide(d) => {
            let _ = sys::retirer_dossier_vide(&d);
        }
        Retrait::DossierWebview(d) => {
            if d.is_dir() && installation::dossier_webview_a_nous(&sys::entrees(&d)) {
                let _ = sys::supprimer_dossier_webview(&d);
                log::info!("desinstallation cache webview: supprime");
            } else if d.is_dir() {
                log::info!("desinstallation cache webview: sans objet");
            } else {
                log::info!("desinstallation cache webview: absent");
            }
        }
    }
}

fn si(cond: bool, oui: &'static str, non: &'static str) -> &'static str {
    if cond {
        oui
    } else {
        non
    }
}
