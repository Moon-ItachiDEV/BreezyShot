//! Fenêtres de l'interface (§6). Pour l'instant : le bandeau de messages.
//!
//! Le module crée, place, montre et cache ; il ne décide d'aucun message. Ce qu'il reçoit
//! de la page remonte à l'Application par `Signal`.

mod place;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::systeme;
pub use place::{cliquable, ecran_sous, placer, zone_ecran, Ecran, Rect, ZoneCss, TAILLE_LOGIQUE};

/// Étiquette de la fenêtre du bandeau (aussi dans `capabilities/`).
pub const ETIQUETTE_BANDEAU: &str = "bandeau";
/// Événement écouté par la page.
pub const EVENEMENT_AFFICHER: &str = "bandeau:afficher";
/// Intervalle de surveillance de la souris pendant un message à croix.
const INTERVALLE_SOURIS: Duration = Duration::from_millis(30);

/// Genre d'un message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Genre {
    Reussite,
    Info,
    Erreur,
}

impl Genre {
    pub fn nom(self) -> &'static str {
        match self {
            Genre::Reussite => "reussite",
            Genre::Info => "info",
            Genre::Erreur => "erreur",
        }
    }

    /// Info et erreur portent une croix et restent jusqu'au clic.
    pub fn a_croix(self) -> bool {
        self != Genre::Reussite
    }
}

/// Un bouton d'action d'un message (fourni par l'Application).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Action {
    pub id: String,
    pub libelle: String,
}

/// Un message du bandeau.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub genre: Genre,
    pub titre: String,
    pub detail: String,
    pub actions: Vec<Action>,
}

/// Ce que la page envoie (`bandeau:afficher`).
#[derive(Debug, Clone, Serialize)]
struct Charge<'a> {
    id: u64,
    genre: Genre,
    titre: &'a str,
    detail: &'a str,
    actions: &'a [Action],
}

/// Ce que le module signale à l'Application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    /// Clic sur une action ou sur la croix (`fermer`) du message `id`.
    ActionBandeau { id: u64, action: String },
}

type Rappel = Box<dyn Fn(Signal) + Send + Sync + 'static>;

/// Le message en cours, vu du cœur.
#[derive(Debug, Clone)]
struct EnCours {
    id: u64,
    message: Message,
    /// Place de la fenêtre et échelle de son écran (connues une fois affiché).
    fenetre: Option<(Rect, f64)>,
    zone: Option<ZoneCss>,
}

#[derive(Default)]
struct Etat {
    dernier_id: u64,
    en_cours: Option<EnCours>,
    /// La page écoute `bandeau:afficher` (sinon le message attend).
    page_prete: bool,
    /// La fenêtre reçoit les clics en ce moment.
    recoit_clics: bool,
}

/// Le bandeau : une fenêtre préparée au démarrage et cachée.
pub struct Bandeau {
    app: AppHandle,
    rappel: Rappel,
    etat: Arc<Mutex<Etat>>,
    /// Drapeau de la surveillance de souris en cours (une seule à la fois) : passé à faux,
    /// le fil s'arrête au tour suivant.
    surveillance: Mutex<Arc<AtomicBool>>,
}

impl Bandeau {
    /// Crée la fenêtre cachée. Rend le bandeau, à confier à Tauri (`app.manage`).
    pub fn creer<F>(app: &AppHandle, rappel: F) -> Result<Bandeau, String>
    where
        F: Fn(Signal) + Send + Sync + 'static,
    {
        let (largeur, hauteur) = TAILLE_LOGIQUE;
        let fenetre = WebviewWindowBuilder::new(
            app,
            ETIQUETTE_BANDEAU,
            WebviewUrl::App("vignette/bandeau.html".into()),
        )
        .title("BreezyShot")
        .inner_size(largeur, hauteur)
        .visible(false)
        .focused(false)
        .focusable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .content_protected(true)
        .build()
        .map_err(|e| format!("fenetre du bandeau: {e}"))?;
        let exclue = systeme::exclure_des_captures(&fenetre);
        log::info!(
            "bandeau exclu des captures: {}",
            if exclue { "oui" } else { "non" }
        );
        traverser(&fenetre, true);
        Ok(Bandeau {
            app: app.clone(),
            rappel: Box::new(rappel),
            etat: Arc::new(Mutex::new(Etat::default())),
            surveillance: Mutex::new(Arc::new(AtomicBool::new(false))),
        })
    }

    fn fenetre(&self) -> Option<WebviewWindow> {
        self.app.get_webview_window(ETIQUETTE_BANDEAU)
    }

    /// Montre un message ; il remplace aussitôt le précédent. Rend son `id`.
    pub fn afficher(&self, message: Message) -> u64 {
        let Ok(mut etat) = self.etat.lock() else {
            log::error!("bandeau: etat inaccessible");
            return 0;
        };
        etat.dernier_id += 1;
        let id = etat.dernier_id;
        etat.en_cours = Some(EnCours {
            id,
            message,
            fenetre: None,
            zone: None,
        });
        if etat.page_prete {
            self.montrer(&mut etat);
        }
        id
    }

    /// Place, montre sans activer, et envoie le message en cours à la page.
    fn montrer(&self, etat: &mut Etat) {
        let Some(fenetre) = self.fenetre() else {
            log::warn!("bandeau: fenetre absente");
            return;
        };
        let Some(en_cours) = etat.en_cours.as_mut() else {
            return;
        };
        // Toute surveillance en cours s'arrête : la nouvelle partira si besoin.
        self.arreter_surveillance();
        let ecrans = ecrans(&self.app);
        let souris = self.app.cursor_position().ok().map(|p| (p.x, p.y));
        let ecran = ecran_sous(souris, &ecrans);
        if let Some(ecran) = ecran {
            let place = placer(&ecran, TAILLE_LOGIQUE);
            poser(&fenetre, place);
            en_cours.fenetre = Some((place, ecran.echelle));
            log::info!(
                "bandeau affiche {} {} ecran {},{} {}x{} echelle {}",
                en_cours.id,
                en_cours.message.genre.nom(),
                place.x,
                place.y,
                place.largeur,
                place.hauteur,
                ecran.echelle
            );
        } else {
            log::info!(
                "bandeau affiche {} {} ecran inconnu",
                en_cours.id,
                en_cours.message.genre.nom()
            );
        }
        match systeme::montrer_sans_activer(&fenetre) {
            Ok(true) => {}
            Ok(false) => {
                if let Err(e) = fenetre.show() {
                    log::warn!("bandeau: affichage impossible: {e}");
                }
            }
            Err(e) => log::warn!("bandeau: affichage impossible: {e}"),
        }
        // Un nouveau message part toujours avec des clics traversants ; la surveillance
        // (message à croix) les rend sur la pilule seulement.
        traverser(&fenetre, true);
        etat.recoit_clics = false;
        let m = &en_cours.message;
        let charge = Charge {
            id: en_cours.id,
            genre: m.genre,
            titre: &m.titre,
            detail: &m.detail,
            actions: &m.actions,
        };
        if let Err(e) = self
            .app
            .emit_to(ETIQUETTE_BANDEAU, EVENEMENT_AFFICHER, charge)
        {
            log::warn!("bandeau: message non envoye: {e}");
        }
        if m.genre.a_croix() {
            self.surveiller(en_cours.id);
        }
    }

    /// Lance la surveillance de la souris pour le message `id` (message à croix).
    fn surveiller(&self, id: u64) {
        let drapeau = Arc::new(AtomicBool::new(true));
        // Un drapeau par surveillance : l'ancien fil voit le sien passer à faux et s'arrête.
        let ancien = match self.surveillance.lock() {
            Ok(mut courante) => std::mem::replace(&mut *courante, Arc::clone(&drapeau)),
            Err(_) => return,
        };
        ancien.store(false, Ordering::SeqCst);
        let app = self.app.clone();
        let etat = Arc::clone(&self.etat);
        let lancement = std::thread::Builder::new()
            .name("bandeau-souris".into())
            .spawn(move || {
                while drapeau.load(Ordering::SeqCst) {
                    let souris = app.cursor_position().ok().map(|p| (p.x, p.y));
                    let Ok(mut e) = etat.lock() else {
                        break;
                    };
                    let Some(en_cours) = e.en_cours.as_ref().filter(|c| c.id == id) else {
                        break;
                    };
                    let voulu = match en_cours.fenetre {
                        Some((place, echelle)) => cliquable(
                            en_cours.message.genre.a_croix(),
                            en_cours.zone.as_ref(),
                            echelle,
                            (place.x, place.y),
                            souris,
                        ),
                        None => false,
                    };
                    if voulu != e.recoit_clics {
                        e.recoit_clics = voulu;
                        let app2 = app.clone();
                        let _ = app.run_on_main_thread(move || {
                            if let Some(f) = app2.get_webview_window(ETIQUETTE_BANDEAU) {
                                traverser(&f, !voulu);
                            }
                        });
                    }
                    drop(e);
                    std::thread::sleep(INTERVALLE_SOURIS);
                }
            });
        if let Err(e) = lancement {
            log::warn!("bandeau: surveillance de la souris impossible: {e}");
        }
    }

    fn arreter_surveillance(&self) {
        if let Ok(courante) = self.surveillance.lock() {
            courante.store(false, Ordering::SeqCst);
        }
    }

    /// Arrête la surveillance, rend les clics traversants et cache la fenêtre.
    fn terminer(&self, etat: &mut Etat) {
        self.arreter_surveillance();
        etat.en_cours = None;
        etat.recoit_clics = false;
        if let Some(f) = self.fenetre() {
            traverser(&f, true);
            if let Err(e) = f.hide() {
                log::warn!("bandeau: fenetre non cachee: {e}");
            }
        }
    }

    /// À l'arrêt de l'app.
    pub fn fermer(&self) {
        if let Ok(mut etat) = self.etat.lock() {
            self.terminer(&mut etat);
        }
        if let Some(f) = self.fenetre() {
            let _ = f.destroy();
        }
    }

    fn page_prete(&self) {
        let Ok(mut etat) = self.etat.lock() else {
            return;
        };
        if etat.page_prete {
            return;
        }
        etat.page_prete = true;
        log::info!("bandeau pret");
        if etat.en_cours.is_some() {
            self.montrer(&mut etat);
        }
    }

    /// Vrai si `id` est le message en cours.
    fn est_en_cours(etat: &Etat, id: u64) -> bool {
        etat.en_cours.as_ref().is_some_and(|c| c.id == id)
    }
}

/// Les écrans connus de Tauri, l'écran principal en tête.
fn ecrans(app: &AppHandle) -> Vec<Ecran> {
    let principal = app.primary_monitor().ok().flatten();
    let mut tous: Vec<tauri::Monitor> = app.available_monitors().unwrap_or_default();
    if let Some(p) = &principal {
        if let Some(i) = tous.iter().position(|m| m.position() == p.position()) {
            let m = tous.remove(i);
            tous.insert(0, m);
        }
    }
    tous.iter()
        .map(|m| {
            let (pos, taille, utile) = (m.position(), m.size(), m.work_area());
            Ecran {
                rect: Rect {
                    x: pos.x,
                    y: pos.y,
                    largeur: taille.width,
                    hauteur: taille.height,
                },
                zone_utile: Rect {
                    x: utile.position.x,
                    y: utile.position.y,
                    largeur: utile.size.width,
                    hauteur: utile.size.height,
                },
                echelle: m.scale_factor(),
            }
        })
        .collect()
}

/// Pose la fenêtre (pixels réels). La position est redonnée après la taille : changer
/// d'écran peut faire réajuster la taille par le système (changement d'échelle).
fn poser(fenetre: &WebviewWindow, place: Rect) {
    let position = tauri::PhysicalPosition::new(place.x, place.y);
    let taille = tauri::PhysicalSize::new(place.largeur, place.hauteur);
    let r = fenetre
        .set_position(position)
        .and_then(|_| fenetre.set_size(taille))
        .and_then(|_| fenetre.set_position(position));
    if let Err(e) = r {
        log::warn!("bandeau: placement impossible: {e}");
    }
}

/// Clics traversants (`true`) ou reçus (`false`).
fn traverser(fenetre: &WebviewWindow, oui: bool) {
    let fait = match systeme::traverser_clics(fenetre, oui) {
        Ok(true) => Ok(()),
        // Par Tauri, seulement sur une fenêtre visible : sous Linux, tao panique sur une
        // fenêtre jamais montrée (event_loop.rs, `CursorIgnoreEvents`). Une fenêtre cachée
        // ne reçoit de toute façon aucun clic.
        Ok(false) if !fenetre.is_visible().unwrap_or(false) => Ok(()),
        Ok(false) => fenetre
            .set_ignore_cursor_events(oui)
            .map_err(|e| e.to_string()),
        Err(e) => Err(e),
    };
    if let Err(e) = fait {
        log::warn!("bandeau: clics non regles: {e}");
    }
}

// Commandes appelées par la page. Un `id` qui n'est pas celui du message en cours est ignoré.

/// La page écoute `bandeau:afficher` : le message en attente peut partir.
#[tauri::command]
pub fn bandeau_pret(bandeau: State<'_, Bandeau>) {
    bandeau.page_prete();
}

/// Rectangle de la pilule (pixels CSS, relatif à la page).
#[tauri::command]
pub fn bandeau_zone(
    bandeau: State<'_, Bandeau>,
    id: u64,
    x: f64,
    y: f64,
    largeur: f64,
    hauteur: f64,
) {
    let Ok(mut etat) = bandeau.etat.lock() else {
        return;
    };
    let Some(en_cours) = etat.en_cours.as_mut().filter(|c| c.id == id) else {
        return;
    };
    let zone = ZoneCss {
        x,
        y,
        largeur,
        hauteur,
    };
    if ![x, y, largeur, hauteur].iter().all(|v| v.is_finite()) {
        return;
    }
    en_cours.zone = Some(zone);
    if let Some((place, echelle)) = en_cours.fenetre {
        let r = zone_ecran(&zone, echelle, (place.x, place.y));
        log::info!(
            "bandeau zone {id} {},{} {}x{}",
            r.x,
            r.y,
            r.largeur,
            r.hauteur
        );
    }
}

/// Clic sur une action ou sur la croix (`fermer`).
#[tauri::command]
pub fn bandeau_action(bandeau: State<'_, Bandeau>, id: u64, action: String) {
    let valide = match bandeau.etat.lock() {
        Ok(etat) => Bandeau::est_en_cours(&etat, id),
        Err(_) => false,
    };
    if valide {
        (bandeau.rappel)(Signal::ActionBandeau { id, action });
    }
}

/// Le message a fini de disparaître : on cache la fenêtre.
#[tauri::command]
pub fn bandeau_termine(bandeau: State<'_, Bandeau>, id: u64) {
    let Ok(mut etat) = bandeau.etat.lock() else {
        return;
    };
    if !Bandeau::est_en_cours(&etat, id) {
        return;
    }
    bandeau.terminer(&mut etat);
    log::info!("bandeau termine {id}");
}
