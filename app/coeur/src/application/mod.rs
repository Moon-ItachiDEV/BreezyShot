//! L'Application : le chef d'orchestre. Seul module qui connaît les autres.
//!
//! Pour brancher un module (étapes suivantes) :
//! 1. le créer dans `demarrer`, à l'endroit marqué « Modules », en lui passant un rappel
//!    qui transforme son signal en `Evenement` : `move |s| traiter(&app, Evenement::X(s))` ;
//! 2. ajouter la variante `Evenement::X(module::Signal)` et son cas dans `traiter` ;
//! 3. libérer ce qu'il tient dans `arreter` (raccourcis, fenêtres) ;
//! 4. ajouter ses entrées de menu dans `entrees_menu`.

pub mod arguments;
pub mod etats;
pub mod icone;
pub mod journal;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

use tauri::{AppHandle, Manager, RunEvent};

use crate::systeme::{self, Theme};
use arguments::Commande;
use etats::{Automate, Declencheur, Mode};
use icone::EntreeMenu;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Identifiant de l'entrée « Quitter BreezyShot ».
pub const MENU_QUITTER: &str = "quitter";

/// Tout ce qui arrive à l'Application : signaux des modules, du système, du menu.
#[derive(Debug, Clone, PartialEq)]
pub enum Evenement {
    /// Une autre instance a été lancée, avec ses arguments (le premier est le programme).
    SecondeInstance(Vec<String>),
    /// Le thème de la barre des tâches a changé.
    ThemeBarre(Theme),
    /// Une entrée du menu de l'icône a été choisie (son identifiant).
    Menu(String),
}

/// État partagé de l'Application, tenu par Tauri (`app.state::<Application>()`).
#[derive(Default)]
pub struct Application {
    automate: Mutex<Automate>,
    /// Vrai une fois le démarrage complet : l'arrêt écrit alors `arret propre`.
    demarree: AtomicBool,
    taille_icone: AtomicU32,
}

/// Point d'entrée de l'app (appelé par `main.rs`).
pub fn lancer() {
    let (commande, inconnus) = arguments::lire(std::env::args().skip(1));
    let construction = tauri::Builder::default()
        // L'instance unique en premier : une seconde instance confie ses arguments à la
        // première et sort avec le code 0 avant d'aller plus loin.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _dossier| {
            traiter(app, Evenement::SecondeInstance(argv));
        }))
        .manage(Application::default())
        .setup(move |app| {
            demarrer(app.handle(), commande, &inconnus);
            Ok(())
        })
        .build(tauri::generate_context!());
    match construction {
        Ok(app) => app.run(|app, evenement| match evenement {
            // L'app vit sans fenêtre : fermer la dernière fenêtre ne la quitte pas.
            // Seul `app.exit(code)` (code présent) quitte.
            RunEvent::ExitRequested {
                code: None, api, ..
            } => api.prevent_exit(),
            RunEvent::Exit => arreter(app),
            _ => {}
        }),
        Err(e) => {
            if !journal::installe() {
                installer_journal_secours();
            }
            log::error!("lancement impossible: {e}");
            eprintln!("BreezyShot : lancement impossible : {e}");
            journal::vider();
            std::process::exit(1);
        }
    }
}

/// Journal de secours dans le dossier temporaire du système.
fn installer_journal_secours() {
    match journal::installer_secours(&systeme::dossier_secours()) {
        Ok(chemin) => eprintln!("BreezyShot : journal de secours : {}", chemin.display()),
        Err(e) => eprintln!("BreezyShot : aucun journal : {e}"),
    }
}

/// Démarrage de la première instance.
fn demarrer(app: &AppHandle, commande: Commande, inconnus: &[String]) {
    // Le journal ne dépend que du dossier local.
    if let Err(e) = systeme::dossier_journal().and_then(|d| journal::installer(&d)) {
        // Dossier local introuvable ou non inscriptible : journal de secours.
        installer_journal_secours();
        log::error!("journal indisponible: {e}");
        eprintln!("BreezyShot : journal indisponible : {e}");
    }
    log::info!("demarrage BreezyShot {VERSION}");
    match systeme::dossiers() {
        Ok(dossiers) => {
            let images = match &dossiers.images {
                Some(i) => i.display().to_string(),
                None => "introuvable".into(),
            };
            log::info!(
                "dossiers reglages={} local={} images={images}",
                dossiers.reglages.display(),
                dossiers.local.display(),
            );
        }
        Err(e) => {
            log::error!("dossiers introuvables: {e}");
            eprintln!("BreezyShot : dossiers introuvables : {e}");
        }
    }
    for a in inconnus {
        log::warn!("argument inconnu ignore: {a}");
    }

    if commande == Commande::Quitter {
        // Personne à fermer : on sort tout de suite, sans rien installer.
        log::info!("--quitter: aucune instance en cours");
        app.exit(0);
        return;
    }

    match systeme::version_windows() {
        Some(build) => log::info!("systeme windows build {build}"),
        None => log::info!("systeme {}", std::env::consts::OS),
    }
    let echelle = match systeme::echelle_par_ecran_v2() {
        Some(true) => "oui",
        Some(false) => "non",
        None => "sans objet",
    };
    log::info!("echelle par ecran v2: {echelle}");

    // Icône et menu.
    let theme = systeme::theme_barre();
    let echelle_principale = icone::echelle_ecran_principal(app);
    let taille = icone::taille_icone(echelle_principale);
    app.state::<Application>()
        .taille_icone
        .store(taille, Ordering::Relaxed);
    match icone::installer(app, theme, taille, &entrees_menu(), |app, id| {
        traiter(app, Evenement::Menu(id))
    }) {
        Ok(()) => log::info!(
            "icone installee ({taille} px, echelle {echelle_principale}, barre {})",
            theme.nom()
        ),
        Err(e) => log::warn!("icone non installee: {e}"),
    }
    let pour_theme = app.clone();
    if let Err(e) = systeme::surveiller_theme_barre(move |t| {
        traiter(&pour_theme, Evenement::ThemeBarre(t));
    }) {
        log::warn!("{e}");
    }

    // Modules : les étapes suivantes branchent ici reglages, raccourcis, fenetres…

    app.state::<Application>()
        .demarree
        .store(true, Ordering::SeqCst);
    log::info!("pret");
}

/// Les entrées du menu de l'icône, dans l'ordre. Seulement ce qui marche.
pub fn entrees_menu() -> Vec<EntreeMenu> {
    vec![EntreeMenu::Action {
        id: MENU_QUITTER.into(),
        texte: "Quitter BreezyShot".into(),
    }]
}

/// Le seul endroit où l'Application réagit à ce qui lui arrive.
pub fn traiter(app: &AppHandle, evenement: Evenement) {
    match evenement {
        Evenement::SecondeInstance(argv) => {
            let (commande, _) = arguments::lire(argv.iter().skip(1));
            match commande {
                Commande::Quitter => {
                    log::info!("--quitter recu: fermeture demandee");
                    app.exit(0);
                }
                Commande::Demarrer => log::info!("seconde instance refusee"),
            }
        }
        Evenement::ThemeBarre(theme) => {
            let taille = app
                .state::<Application>()
                .taille_icone
                .load(Ordering::Relaxed);
            match icone::changer_image(app, theme, taille) {
                Ok(()) => log::info!("icone mise a jour: barre {}", theme.nom()),
                Err(e) => log::warn!("icone non mise a jour: {e}"),
            }
        }
        Evenement::Menu(id) => match id.as_str() {
            MENU_QUITTER => {
                log::info!("menu: quitter");
                app.exit(0);
            }
            autre => log::warn!("menu: entree inconnue {autre}"),
        },
    }
}

/// Demande une capture (raccourci, menu). Tant que le module Capture n'existe pas
/// (étapes 1 à 3), le gel échoue aussitôt et l'app redescend au Repos.
pub fn lancer_capture(app: &AppHandle, mode: Mode) {
    let etat = app.state::<Application>();
    let Ok(mut automate) = etat.automate.lock() else {
        log::error!("automate inaccessible");
        return;
    };
    if automate
        .traiter(Declencheur::DemandeCapture(mode))
        .is_none()
    {
        return;
    }
    // Étape 4 : ici, la Capture fige les écrans. En attendant :
    log::warn!("capture indisponible a cette etape");
    automate.traiter(Declencheur::GelImpossible);
    // Étape 3 : message dans le bandeau (fenetres::Bandeau).
}

/// Arrêt : libère ce qui est tenu, retire l'icône, écrit `arret propre`.
fn arreter(app: &AppHandle) {
    if !app.state::<Application>().demarree.load(Ordering::SeqCst) {
        journal::vider();
        return;
    }
    // Modules : libérer ici les raccourcis, fermer les fenêtres…
    if icone::retirer(app) {
        log::info!("icone retiree");
    }
    log::info!("arret propre");
    journal::vider();
}
