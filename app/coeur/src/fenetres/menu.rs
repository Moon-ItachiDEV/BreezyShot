//! Menu de l'icône de la zone de notification (piste A « Liste », choisie le 13/11/2026) :
//! le menu natif ne se style pas, c'est donc une petite fenêtre web sans cadre, posée
//! au-dessus de l'icône. Créée cachée à la première ouverture, puis gardée cachée et
//! réutilisée (elle s'ouvre alors sans attendre).
//!
//! Le module crée, place, montre et cache ; il ne décide de rien. L'Application lui donne
//! les lignes et reçoit le choix (`menu_choisi`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use super::place::ecran_sous;
use super::{ecrans, theme, Ecran, Rect, ARGUMENTS_NAVIGATEUR};
use crate::systeme;

/// Étiquette de la fenêtre (aussi dans `capabilities/menu.json`).
pub const ETIQUETTE_MENU: &str = "menu";
/// Page de la fenêtre, sous `app/interface/`.
pub const PAGE: &str = "menu/menu.html";
/// Événements écoutés par la page : les lignes à montrer, à chaque ouverture ; vider la
/// carte (menu caché : la prochaine ouverture ne montre jamais les anciennes lignes).
pub const EVENEMENT_OUVRIR: &str = "menu:ouvrir";
pub const EVENEMENT_VIDER: &str = "menu:vider";
/// Pas de la surveillance des clics hors du menu.
pub const PAS_SURVEILLANCE: Duration = Duration::from_millis(30);

/// Mesures du menu en pixels logiques, les mêmes que `menu/menu.css`.
pub const LARGEUR: f64 = 336.0;
pub const HAUTEUR_LIGNE: f64 = 34.0;
/// Un séparateur : 1 px et 4 px de marge de part et d'autre.
pub const HAUTEUR_SEPARATEUR: f64 = 9.0;
/// Bord (1 px) et retrait (4 px), en haut et en bas.
pub const CADRE: f64 = 10.0;
/// Place de l'ombre autour de la carte, de chaque côté.
pub const MARGE: f64 = 16.0;
/// Écart entre la carte et la barre des tâches (ou l'icône).
pub const ECART: f64 = 8.0;
/// La carte dépasse l'icône de ce décalage vers la droite (maquette : menu presque aligné
/// à droite sur l'icône).
pub const DEBORD_DROITE: f64 = 44.0;

/// Un clic sur l'icône qui suit de si près la fermeture du menu (perte du focus causée par
/// ce même clic) ne le rouvre pas : le clic referme, comme un menu natif.
pub const ANTI_REBOND: Duration = Duration::from_millis(300);

/// Une ligne du menu, telle que la page la reçoit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ligne {
    /// Identifiant renvoyé par `menu_choisi` ; vide pour un séparateur.
    pub id: String,
    pub libelle: String,
    pub raccourci: Option<String>,
    pub separateur: bool,
}

/// Ce que reçoit la page à chaque ouverture.
#[derive(Debug, Clone, Serialize)]
pub struct Ouverture {
    pub lignes: Vec<Ligne>,
}

/// Dernière fermeture (pour `ANTI_REBOND`) et lignes en attente de la page.
static FERME_A: Mutex<Option<Instant>> = Mutex::new(None);
static EN_ATTENTE: Mutex<Option<Ouverture>> = Mutex::new(None);
/// Vrai tant que la fenêtre du menu est exclue des captures (posé par `ouvrir`).
static EXCLU: AtomicBool = AtomicBool::new(false);

fn fenetre(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(ETIQUETTE_MENU)
}

/// Échelle utilisable : une valeur absurde vaut 1.
fn echelle_sure(echelle: f64) -> f64 {
    if echelle.is_finite() && echelle > 0.0 {
        echelle
    } else {
        1.0
    }
}

/// Hauteur de la carte (pixels logiques) pour ces lignes.
pub fn hauteur_carte(lignes: &[Ligne]) -> f64 {
    let seps = lignes.iter().filter(|l| l.separateur).count() as f64;
    let actions = lignes.len() as f64 - seps;
    CADRE + actions * HAUTEUR_LIGNE + seps * HAUTEUR_SEPARATEUR
}

/// Place de la fenêtre (pure, pixels réels), ombre comprise. La carte se pose au-dessus de
/// l'icône, sans descendre sous la zone utile (la barre des tâches), le bord droit à
/// `DEBORD_DROITE` à droite du centre de l'icône ; sans place au-dessus, en dessous de
/// l'icône (barre des tâches en haut). Toujours dans la zone utile, à `ECART` des bords.
pub fn placer(icone: Rect, ecran: &Ecran, hauteur_logique: f64) -> Rect {
    let e = echelle_sure(ecran.echelle);
    let px = |v: f64| (v * e).round() as i64;
    let (marge, ecart) = (px(MARGE), px(ECART));
    let (lc, hc) = (px(LARGEUR), px(hauteur_logique));
    let z = ecran.zone_utile;
    let (zg, zh) = (i64::from(z.x), i64::from(z.y));
    let (zd, zb) = (zg + i64::from(z.largeur), zh + i64::from(z.hauteur));
    let (ig, ih) = (i64::from(icone.x), i64::from(icone.y));
    let ib = ih + i64::from(icone.hauteur);
    let centre = ig + i64::from(icone.largeur) / 2;

    // Horizontal : bord droit près de l'icône, borné à la zone utile.
    let droite_voulue = centre + px(DEBORD_DROITE);
    let gauche = (droite_voulue - lc).min(zd - ecart - lc).max(zg + ecart);

    // Vertical : au-dessus si la carte tient, sinon en dessous.
    let bas = ih.min(zb) - ecart;
    let haut = if bas - hc >= zh + ecart {
        bas - hc
    } else {
        (ib.max(zh) + ecart).min(zb - ecart - hc).max(zh + ecart)
    };
    Rect {
        x: (gauche - marge) as i32,
        y: (haut - marge) as i32,
        largeur: (lc + 2 * marge).max(1) as u32,
        hauteur: (hc + 2 * marge).max(1) as u32,
    }
}

/// Vrai si un clic sur l'icône arrive juste après une fermeture : il ne rouvre pas.
pub fn vient_de_fermer() -> bool {
    FERME_A
        .lock()
        .ok()
        .and_then(|f| *f)
        .is_some_and(|t| t.elapsed() < ANTI_REBOND)
}

/// Vrai si le menu est montré.
pub fn ouvert(app: &AppHandle) -> bool {
    fenetre(app).is_some_and(|f| f.is_visible().unwrap_or(false))
}

fn creer(app: &AppHandle) -> Result<WebviewWindow, String> {
    let f = WebviewWindowBuilder::new(app, ETIQUETTE_MENU, WebviewUrl::App(PAGE.into()))
        .title("BreezyShot")
        .inner_size(LARGEUR + 2.0 * MARGE, 200.0)
        .visible(false)
        .focused(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .theme(theme::pour_cadre(theme::actuel()))
        .additional_browser_args(ARGUMENTS_NAVIGATEUR)
        .build()
        .map_err(|e| format!("creation: {e}"))?;
    if let Err(e) = systeme::preparer_fenetre_outil_active(&f) {
        log::warn!("menu web: style outil non pose: {e}");
    }
    let pour_focus = app.clone();
    f.on_window_event(move |e| {
        // Un clic ailleurs (ou Alt+Tab) : le menu se ferme, comme un menu natif.
        if let tauri::WindowEvent::Focused(false) = e {
            let _ = cacher(&pour_focus);
        }
    });
    match systeme::identifiant_fenetre(&f) {
        Some(h) => log::info!("menu fenetre {h}"),
        None => log::info!("menu fenetre creee"),
    }
    Ok(f)
}

/// Vrai si le rectangle donné pour l'icône est inutilisable (Windows ne l'a pas donné).
pub fn rect_inconnu(r: &Rect) -> bool {
    (r.largeur <= 1 && r.hauteur <= 1) || (r.x == 0 && r.y == 0)
}

/// Ouvre le menu au-dessus de l'icône (`icone` : son rectangle, pixels réels ; `None` ou
/// inconnu : sous la souris). La première fois, la fenêtre est créée et la page envoie
/// `menu_pret` : les lignes lui sont données à ce moment-là. `exclu` : la fenêtre est
/// exclue des captures pour cette ouverture (pendant une défilante, où elle pourrait
/// entrer dans une image), et l'exclusion est retirée à la première ouverture sans. Rend
/// vrai si le menu est montré.
pub fn ouvrir(app: &AppHandle, icone: Option<Rect>, lignes: Vec<Ligne>, exclu: bool) -> bool {
    let icone = icone.filter(|r| !rect_inconnu(r));
    let ancre = icone.or_else(|| {
        super::souris(app).map(|(x, y)| Rect {
            x: x as i32,
            y: y as i32,
            largeur: 1,
            hauteur: 1,
        })
    });
    let centre = ancre.map(|r| {
        (
            f64::from(r.x) + f64::from(r.largeur) / 2.0,
            f64::from(r.y) + f64::from(r.hauteur) / 2.0,
        )
    });
    let Some(ecran) = ecran_sous(centre, &ecrans(app)) else {
        log::warn!("menu non ouvert: aucun ecran");
        return false;
    };
    let ancre = ancre.unwrap_or(ecran.zone_utile);
    let place = placer(ancre, &ecran, hauteur_carte(&lignes));
    let charge = Ouverture { lignes };
    let (f, nouvelle) = match fenetre(app) {
        Some(f) => (f, false),
        None => match creer(app) {
            Ok(f) => (f, true),
            Err(e) => {
                log::warn!("menu non ouvert: {e}");
                return false;
            }
        },
    };
    regler_exclusion(&f, exclu);
    let _ = f.set_position(tauri::PhysicalPosition::new(place.x, place.y));
    let _ = f.set_size(tauri::PhysicalSize::new(place.largeur, place.hauteur));
    if nouvelle {
        // La page n'écoute pas encore : elle prendra les lignes à `menu_pret`.
        if let Ok(mut a) = EN_ATTENTE.lock() {
            *a = Some(charge);
        }
    } else {
        theme::envoyer_a(app, ETIQUETTE_MENU);
        if let Err(e) = app.emit_to(ETIQUETTE_MENU, EVENEMENT_OUVRIR, charge) {
            log::warn!("menu web: lignes non envoyees: {e}");
        }
    }
    let _ = f.show();
    if !matches!(systeme::prendre_premier_plan(&f), Ok(true)) {
        let _ = f.set_focus();
    }
    log::info!("menu ouvert");
    surveiller_clics(app, place);
    true
}

/// Pose ou retire l'exclusion des captures, avant que la fenêtre soit montrée. Seulement
/// quand elle change : hors défilante, rien n'est appelé ni écrit. En essai capturable
/// (`BREEZYSHOT_ESSAI_CAPTURABLE=1`), rien n'est exclu, comme pour les autres fenêtres.
fn regler_exclusion(f: &WebviewWindow, exclu: bool) {
    if exclu == EXCLU.load(Ordering::SeqCst) {
        return;
    }
    if exclu {
        if systeme::essai_capturable() {
            log::info!("menu exclu des captures: non (essai)");
            return;
        }
        let fait = systeme::exclure_des_captures(f);
        EXCLU.store(fait, Ordering::SeqCst);
        log::info!(
            "menu exclu des captures: {}",
            if fait { "oui" } else { "non" }
        );
    } else {
        let fait = systeme::inclure_dans_les_captures(f);
        EXCLU.store(!fait, Ordering::SeqCst);
        log::info!(
            "menu exclusion des captures retiree: {}",
            if fait { "oui" } else { "non" }
        );
    }
}

/// Ferme le menu au premier clic hors de lui, même s'il n'a jamais reçu le focus
/// (Windows peut refuser le premier plan : `Focused(false)` n'arriverait jamais). Un fil
/// regarde les boutons de la souris tant que le menu est montré.
fn surveiller_clics(app: &AppHandle, place: Rect) {
    let app = app.clone();
    let lance = std::thread::Builder::new()
        .name("menu-clics".into())
        .spawn(move || {
            // Un bouton encore enfoncé à l'ouverture (le clic sur l'icône) ne compte pas.
            let mut relache = !systeme::bouton_souris_enfonce();
            loop {
                std::thread::sleep(PAS_SURVEILLANCE);
                if !ouvert(&app) {
                    return;
                }
                let enfonce = systeme::bouton_souris_enfonce();
                if !enfonce {
                    relache = true;
                    continue;
                }
                if !relache {
                    continue;
                }
                let dedans = systeme::position_souris().is_some_and(|(x, y)| place.contient(x, y));
                if !dedans {
                    let pour = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        cacher(&pour);
                    });
                    return;
                }
                relache = false;
            }
        });
    if let Err(e) = lance {
        log::warn!("menu web: surveillance des clics impossible: {e}");
    }
}

/// Les lignes en attente de la page (première ouverture), une seule fois.
pub fn prendre_en_attente() -> Option<Ouverture> {
    EN_ATTENTE.lock().ok().and_then(|mut a| a.take())
}

/// Cache le menu (choix fait, Échap, clic ailleurs) et vide la page. Rend vrai s'il était
/// montré.
pub fn cacher(app: &AppHandle) -> bool {
    let Some(f) = fenetre(app) else {
        return false;
    };
    if !f.is_visible().unwrap_or(false) {
        return false;
    }
    if let Ok(mut t) = FERME_A.lock() {
        *t = Some(Instant::now());
    }
    let _ = f.hide();
    if let Err(e) = app.emit_to(ETIQUETTE_MENU, EVENEMENT_VIDER, ()) {
        log::warn!("menu web: page non videe: {e}");
    }
    log::info!("menu ferme");
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ligne(id: &str) -> Ligne {
        Ligne {
            id: id.into(),
            libelle: id.into(),
            raccourci: None,
            separateur: false,
        }
    }
    fn sep() -> Ligne {
        Ligne {
            id: String::new(),
            libelle: String::new(),
            raccourci: None,
            separateur: true,
        }
    }

    /// Écran 1920 × 1080 à 100 %, barre des tâches de 48 px en bas.
    fn ecran(echelle: f64) -> Ecran {
        let h = (1080.0 * echelle) as u32;
        let l = (1920.0 * echelle) as u32;
        let barre = (48.0 * echelle) as u32;
        Ecran {
            rect: Rect {
                x: 0,
                y: 0,
                largeur: l,
                hauteur: h,
            },
            zone_utile: Rect {
                x: 0,
                y: 0,
                largeur: l,
                hauteur: h - barre,
            },
            echelle,
        }
    }

    #[test]
    fn rectangle_de_l_icone_inconnu() {
        assert!(rect_inconnu(&Rect {
            x: 0,
            y: 0,
            largeur: 0,
            hauteur: 0
        }));
        assert!(rect_inconnu(&Rect {
            x: 0,
            y: 0,
            largeur: 24,
            hauteur: 40
        }));
        assert!(rect_inconnu(&Rect {
            x: 500,
            y: 500,
            largeur: 1,
            hauteur: 1
        }));
        assert!(!rect_inconnu(&Rect {
            x: 1700,
            y: 1036,
            largeur: 24,
            hauteur: 40
        }));
    }

    #[test]
    fn hauteur_selon_les_lignes() {
        let l = [ligne("a"), ligne("b"), sep(), ligne("c")];
        assert_eq!(
            hauteur_carte(&l),
            CADRE + 3.0 * HAUTEUR_LIGNE + HAUTEUR_SEPARATEUR
        );
    }

    #[test]
    fn au_dessus_de_la_barre_des_taches() {
        let e = ecran(1.0);
        // Icône dans la barre, près de l'horloge.
        let icone = Rect {
            x: 1700,
            y: 1036,
            largeur: 24,
            hauteur: 40,
        };
        let p = placer(icone, &e, 200.0);
        let (marge, ecart) = (MARGE as i32, ECART as i32);
        // Carte : bas à 8 px au-dessus de la zone utile, bord droit à 44 px du centre.
        assert_eq!(p.y + p.hauteur as i32 - marge, 1032 - ecart);
        assert_eq!(p.x + p.largeur as i32 - marge, 1712 + 44);
        assert_eq!(p.largeur, (LARGEUR + 2.0 * MARGE) as u32);
    }

    #[test]
    fn bornee_au_bord_droit() {
        let e = ecran(1.0);
        let icone = Rect {
            x: 1900,
            y: 1036,
            largeur: 20,
            hauteur: 40,
        };
        let p = placer(icone, &e, 200.0);
        assert_eq!(p.x + p.largeur as i32 - MARGE as i32, 1920 - ECART as i32);
    }

    #[test]
    fn a_l_echelle_de_l_ecran() {
        let e = ecran(1.5);
        let icone = Rect {
            x: 2500,
            y: 1560,
            largeur: 36,
            hauteur: 60,
        };
        let p = placer(icone, &e, 200.0);
        assert_eq!(p.largeur, ((LARGEUR + 2.0 * MARGE) * 1.5).round() as u32);
        assert_eq!(p.hauteur, ((200.0 + 2.0 * MARGE) * 1.5).round() as u32);
        let bas_carte = p.y + p.hauteur as i32 - (MARGE * 1.5) as i32;
        assert_eq!(bas_carte, 1620 - 48 * 3 / 2 - (ECART * 1.5) as i32);
    }

    #[test]
    fn dans_le_panneau_des_icones_cachees() {
        // L'icône est dans le panneau au-dessus de la barre : la carte se pose au-dessus
        // de l'icône elle-même.
        let e = ecran(1.0);
        let icone = Rect {
            x: 1600,
            y: 960,
            largeur: 32,
            hauteur: 32,
        };
        let p = placer(icone, &e, 200.0);
        assert_eq!(p.y + p.hauteur as i32 - MARGE as i32, 960 - ECART as i32);
    }

    #[test]
    fn barre_des_taches_en_haut() {
        let e = Ecran {
            rect: Rect {
                x: 0,
                y: 0,
                largeur: 1920,
                hauteur: 1080,
            },
            zone_utile: Rect {
                x: 0,
                y: 48,
                largeur: 1920,
                hauteur: 1032,
            },
            echelle: 1.0,
        };
        let icone = Rect {
            x: 1700,
            y: 4,
            largeur: 24,
            hauteur: 40,
        };
        let p = placer(icone, &e, 300.0);
        assert_eq!(p.y + MARGE as i32, 48 + ECART as i32);
    }

    #[test]
    fn ecran_a_gauche_coordonnees_negatives() {
        let mut e = ecran(1.0);
        e.rect.x = -1920;
        e.zone_utile.x = -1920;
        let icone = Rect {
            x: -30,
            y: 1036,
            largeur: 24,
            hauteur: 40,
        };
        let p = placer(icone, &e, 200.0);
        assert_eq!(p.x + p.largeur as i32 - MARGE as i32, -ECART as i32);
    }
}
