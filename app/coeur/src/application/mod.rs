//! L'Application : le chef d'orchestre. Seul module qui connaît les autres.
//!
//! Pour brancher un module (étapes suivantes) :
//! 1. le créer dans `demarrer`, à l'endroit marqué « Modules », en lui passant un rappel
//!    qui transforme son signal en `Evenement` : `move |s| traiter(&app, Evenement::X(s))` ;
//! 2. ajouter la variante `Evenement::X(module::Signal)` et son cas dans `traiter` ;
//! 3. libérer ce qu'il tient dans `arreter` (raccourcis, fenêtres) ;
//! 4. ajouter ses entrées de menu dans `entrees_menu`.

pub mod arguments;
pub mod etat;
pub mod etats;
pub mod icone;
pub mod journal;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

use tauri::{AppHandle, Manager, RunEvent};

use crate::fenetres::{self, Bandeau, Genre, Message};
use crate::raccourcis::{self, Bilan, Decision};
use crate::reglages::{self, Constat, Plateforme, Reglages};
use crate::systeme::{self, Systeme, Theme};
use arguments::Commande;
use etats::{Automate, Declencheur, Mode};
use icone::EntreeMenu;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Identifiant de l'entrée « Quitter BreezyShot ».
pub const MENU_QUITTER: &str = "quitter";
/// Identifiant de l'entrée « Capturer une zone ».
pub const MENU_CAPTURER_ZONE: &str = "capturer_zone";

/// Les actions dont la capture existe : seules celles-ci reçoivent un raccourci (§5).
/// `plein_ecran` et `fenetre` s'ajouteront avec leur étape.
pub const ACTIONS_CONSTRUITES: [&str; 1] = ["zone"];

/// Tout ce qui arrive à l'Application : signaux des modules, du système, du menu.
#[derive(Debug, Clone, PartialEq)]
pub enum Evenement {
    /// Une autre instance a été lancée, avec ses arguments (le premier est le programme).
    SecondeInstance(Vec<String>),
    /// Le thème de la barre des tâches a changé.
    ThemeBarre(Theme),
    /// Une entrée du menu de l'icône a été choisie (son identifiant).
    Menu(String),
    /// Signal du module des raccourcis (appui).
    Raccourci(raccourcis::Signal),
    /// Signal du module des fenêtres (clic dans le bandeau).
    Fenetres(fenetres::Signal),
}

/// État partagé de l'Application, tenu par Tauri (`app.state::<Application>()`).
#[derive(Default)]
pub struct Application {
    automate: Mutex<Automate>,
    /// Vrai une fois le démarrage complet : l'arrêt écrit alors `arret propre`.
    demarree: AtomicBool,
    taille_icone: AtomicU32,
    /// Réglages lus au démarrage.
    reglages: Mutex<Option<Reglages>>,
    /// Vrai si des raccourcis ont été réservés (à libérer à l'arrêt).
    raccourcis_reserves: AtomicBool,
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
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(Application::default())
        .invoke_handler(tauri::generate_handler![
            fenetres::bandeau_pret,
            fenetres::bandeau_zone,
            fenetres::bandeau_action,
            fenetres::bandeau_termine
        ])
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
    let dossiers = systeme::dossiers();
    match &dossiers {
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
    let dossiers = dossiers.ok();

    // Réglages.
    let plateforme = plateforme_reglages(systeme::systeme());
    let reglages = match &dossiers {
        Some(d) => {
            let (reglages, constats) = reglages::charger(&d.reglages, plateforme);
            for c in &constats {
                match c {
                    Constat::Cree { .. } | Constat::MisANiveau { .. } => log::info!("{c}"),
                    _ => log::warn!("{c}"),
                }
            }
            reglages
        }
        None => Reglages::par_defaut(plateforme),
    };
    let candidats = table_candidats(&reglages);
    if let Ok(mut r) = app.state::<Application>().reglages.lock() {
        *r = Some(reglages);
    }

    // Bandeau de messages.
    let pour_bandeau = app.clone();
    match Bandeau::creer(app, move |s| traiter(&pour_bandeau, Evenement::Fenetres(s))) {
        Ok(bandeau) => {
            app.manage(bandeau);
        }
        Err(e) => log::warn!("bandeau indisponible: {e}"),
    }

    // Raccourcis.
    let pour_raccourcis = app.clone();
    let bilan = raccourcis::reserver(app, &candidats, systeme::impr_ecran_reserve(), move |s| {
        traiter(&pour_raccourcis, Evenement::Raccourci(s))
    });
    for ligne in raccourcis::lignes_journal(&bilan) {
        log::info!("{ligne}");
    }
    app.state::<Application>()
        .raccourcis_reserves
        .store(true, Ordering::SeqCst);

    // Icône et menu.
    let theme = systeme::theme_barre();
    let echelle_principale = icone::echelle_ecran_principal(app);
    let taille = icone::taille_icone(echelle_principale);
    app.state::<Application>()
        .taille_icone
        .store(taille, Ordering::Relaxed);
    let libelle_zone = bilan.libelle_menu("zone");
    match icone::installer(
        app,
        theme,
        taille,
        &entrees_menu(libelle_zone.as_deref()),
        |app, id| traiter(app, Evenement::Menu(id)),
    ) {
        Ok(()) => log::info!(
            "icone installee ({taille} px, echelle {echelle_principale}, barre {})",
            theme.nom()
        ),
        Err(e) => log::warn!("icone non installee: {e}"),
    }
    verifier_icone_visible(app);
    let pour_theme = app.clone();
    if let Err(e) = systeme::surveiller_theme_barre(move |t| {
        traiter(&pour_theme, Evenement::ThemeBarre(t));
    }) {
        log::warn!("{e}");
    }

    // Annonce du démarrage (une seule fois par situation).
    if let Some(d) = &dossiers {
        annoncer_raccourcis(app, &d.local, &bilan);
    }

    app.state::<Application>()
        .demarree
        .store(true, Ordering::SeqCst);
    log::info!("pret");
}

/// Écrit `icone visible` ou `icone invisible` selon que la zone de notification donne
/// le rectangle de l'icône (jusqu'à 3 s d'attente, sans bloquer le démarrage).
fn verifier_icone_visible(app: &AppHandle) {
    let app = app.clone();
    let lance = std::thread::Builder::new()
        .name("icone-visible".into())
        .spawn(move || {
            for _ in 0..15 {
                if icone::visible(&app) {
                    log::info!("icone visible");
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            log::warn!("icone invisible");
        });
    if let Err(e) = lance {
        log::warn!("icone invisible: verification impossible ({e})");
    }
}

/// Traduit le système en plateforme des réglages (les deux modules ne se connaissent pas).
pub fn plateforme_reglages(s: Systeme) -> Plateforme {
    match s {
        Systeme::Windows => Plateforme::Windows,
        Systeme::Mac => Plateforme::Mac,
        Systeme::Autre => Plateforme::Autre,
    }
}

/// La table `action → candidats` donnée au choix des raccourcis : seulement les actions
/// construites.
pub fn table_candidats(reglages: &Reglages) -> BTreeMap<String, Vec<String>> {
    ACTIONS_CONSTRUITES
        .iter()
        .map(|a| {
            let liste = reglages.raccourcis.candidats(a).unwrap_or_default();
            (a.to_string(), liste.to_vec())
        })
        .collect()
}

/// Ligne de journal d'une décision d'annonce (rien si tout est normal).
pub fn ligne_annonce(decision: &Decision) -> Option<String> {
    let situation = decision.a_retenir.as_ref()?;
    Some(match decision.annonce {
        Some(_) => format!("annonce raccourci: {}", situation.raison),
        None => "annonce raccourci: deja faite".into(),
    })
}

/// Lit la dernière situation, décide, montre l'annonce éventuelle, écrit ou efface.
fn annoncer_raccourcis(app: &AppHandle, local: &std::path::Path, bilan: &Bilan) {
    let derniere = etat::lire_situation(local);
    let decision = raccourcis::decider_annonce(derniere.as_ref(), bilan);
    if let Some(ligne) = ligne_annonce(&decision) {
        log::info!("{ligne}");
    }
    if let Some(annonce) = &decision.annonce {
        afficher(
            app,
            Message {
                genre: if annonce.erreur {
                    Genre::Erreur
                } else {
                    Genre::Info
                },
                titre: annonce.titre.clone(),
                detail: annonce.detail.clone(),
                actions: Vec::new(),
            },
        );
    }
    if let Err(e) = etat::ecrire_situation(local, decision.a_retenir.as_ref()) {
        log::warn!("etat non ecrit: {e}");
    }
}

/// Montre un message dans le bandeau, s'il existe.
fn afficher(app: &AppHandle, message: Message) {
    match app.try_state::<Bandeau>() {
        Some(bandeau) => {
            bandeau.afficher(message);
        }
        None => log::warn!("message non affiche (pas de bandeau): {}", message.titre),
    }
}

/// Texte de l'entrée « Capturer une zone », avec le raccourci actif à droite (tabulation :
/// alignée à droite par les menus Windows).
pub fn texte_capturer_zone(raccourci: Option<&str>) -> String {
    match raccourci {
        Some(r) => format!("Capturer une zone\t{r}"),
        None => "Capturer une zone".into(),
    }
}

/// Les entrées du menu de l'icône, dans l'ordre. Seulement ce qui marche.
pub fn entrees_menu(raccourci_zone: Option<&str>) -> Vec<EntreeMenu> {
    vec![
        EntreeMenu::Action {
            id: MENU_CAPTURER_ZONE.into(),
            texte: texte_capturer_zone(raccourci_zone),
        },
        EntreeMenu::Separateur,
        EntreeMenu::Action {
            id: MENU_QUITTER.into(),
            texte: "Quitter BreezyShot".into(),
        },
    ]
}

/// Message provisoire tant que la Capture n'existe pas (étapes 1 à 3).
pub fn message_provisoire(mode: Mode) -> String {
    let quoi = match mode {
        Mode::Zone => "capture de zone",
        Mode::PleinEcran => "capture plein écran",
        Mode::Fenetre => "capture de fenêtre",
    };
    format!("Raccourci reçu : {quoi}")
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
            MENU_CAPTURER_ZONE => {
                log::info!("menu: capturer une zone");
                lancer_capture(app, Mode::Zone);
            }
            autre => log::warn!("menu: entree inconnue {autre}"),
        },
        Evenement::Raccourci(raccourcis::Signal::Presse(action)) => {
            log::info!("raccourci presse: {action}");
            match Mode::depuis_nom(&action) {
                Some(mode) => lancer_capture(app, mode),
                None => log::warn!("raccourci: action inconnue {action}"),
            }
        }
        Evenement::Fenetres(fenetres::Signal::ActionBandeau { id, action }) => {
            // La page ferme le message elle-même ; aucune action n'existe encore.
            log::info!("bandeau action {id} {action}");
        }
    }
}

/// Demande une capture (raccourci, menu). Tant que le module Capture n'existe pas
/// (étapes 1 à 3), le gel échoue aussitôt et l'app redescend au Repos.
pub fn lancer_capture(app: &AppHandle, mode: Mode) {
    {
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
    }
    // Message provisoire, remplacé à l'étape 4.
    afficher(
        app,
        Message {
            genre: Genre::Reussite,
            titre: message_provisoire(mode),
            detail: String::new(),
            actions: Vec::new(),
        },
    );
}

/// Arrêt : libère ce qui est tenu, retire l'icône, écrit `arret propre`.
fn arreter(app: &AppHandle) {
    if !app.state::<Application>().demarree.load(Ordering::SeqCst) {
        journal::vider();
        return;
    }
    // Modules : libérer les raccourcis, fermer les fenêtres.
    if app
        .state::<Application>()
        .raccourcis_reserves
        .swap(false, Ordering::SeqCst)
    {
        match raccourcis::liberer(app) {
            Ok(()) => log::info!("raccourcis liberes"),
            Err(e) => log::warn!("raccourcis non liberes: {e}"),
        }
    }
    if let Some(bandeau) = app.try_state::<Bandeau>() {
        bandeau.fermer();
    }
    if icone::retirer(app) {
        log::info!("icone retiree");
    }
    log::info!("arret propre");
    journal::vider();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raccourcis::{choisir, decider_annonce, Situation};

    #[test]
    fn menu_avec_et_sans_raccourci() {
        let m = entrees_menu(Some("Ctrl + Maj + 1"));
        assert_eq!(
            m[0],
            EntreeMenu::Action {
                id: MENU_CAPTURER_ZONE.into(),
                texte: "Capturer une zone\tCtrl + Maj + 1".into()
            }
        );
        assert_eq!(m[1], EntreeMenu::Separateur);
        assert_eq!(
            m[2],
            EntreeMenu::Action {
                id: MENU_QUITTER.into(),
                texte: "Quitter BreezyShot".into()
            }
        );
        assert_eq!(texte_capturer_zone(None), "Capturer une zone");
    }

    #[test]
    fn seules_les_actions_construites_sont_proposees() {
        let r = Reglages::par_defaut(Plateforme::Windows);
        let t = table_candidats(&r);
        assert_eq!(t.len(), 1);
        assert_eq!(t["zone"], ["ImprEcran", "Ctrl+Maj+1"]);
    }

    #[test]
    fn plateformes() {
        assert_eq!(plateforme_reglages(Systeme::Windows), Plateforme::Windows);
        assert_eq!(plateforme_reglages(Systeme::Mac), Plateforme::Mac);
        assert_eq!(plateforme_reglages(Systeme::Autre), Plateforme::Autre);
    }

    #[test]
    fn lignes_d_annonce() {
        let t = table_candidats(&Reglages::par_defaut(Plateforme::Windows));
        let normal = choisir(&t, false, |_| true);
        assert_eq!(ligne_annonce(&decider_annonce(None, &normal)), None);
        let reserve = choisir(&t, true, |_| true);
        let d = decider_annonce(None, &reserve);
        assert_eq!(
            ligne_annonce(&d).as_deref(),
            Some("annonce raccourci: reserve_par_le_systeme")
        );
        let deja: Option<&Situation> = d.a_retenir.as_ref();
        assert_eq!(
            ligne_annonce(&decider_annonce(deja, &reserve)).as_deref(),
            Some("annonce raccourci: deja faite")
        );
        let rien = choisir(&t, false, |_| false);
        assert_eq!(
            ligne_annonce(&decider_annonce(None, &rien)).as_deref(),
            Some("annonce raccourci: aucun_raccourci")
        );
    }

    #[test]
    fn message_provisoire_de_zone() {
        assert_eq!(
            message_provisoire(Mode::Zone),
            "Raccourci reçu : capture de zone"
        );
    }
}
