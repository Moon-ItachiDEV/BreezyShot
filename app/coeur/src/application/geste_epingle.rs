//! Gestes des captures épinglées à l'écran : ouverture différée hors rappel WebView2,
//! registre des fenêtres actives, redimensionnement, opacité, copie et fermeture.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use super::epingle::{self, ChargeEpingle, TailleEpingle};
use super::{afficher, demande_sortie, plus_tard_nomme, sortir_ou_echec, Application};
use crate::fenetres::{epingle as fenetre, theme};
use crate::sortie::{self, Etape};
use crate::systeme::gel::Pixels;

/// Une capture épinglée actuellement ouverte.
pub(super) struct EpingleOuverte {
    pixels: Arc<Pixels>,
    transparence: bool,
    echelle_ecran: f64,
    largeur: u32,
    hauteur: u32,
    echelle: f64,
    opacite: f64,
    identifiant: Option<isize>,
}

fn epingles(app: &AppHandle) -> std::sync::MutexGuard<'_, HashMap<u64, EpingleOuverte>> {
    app.state::<Application>()
        .inner()
        .epingles
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Ouvre une nouvelle capture épinglée pour `pixels`.
///
/// La création WebView2 est toujours différée au tour suivant (`plus_tard_nomme`) pour ne
/// jamais bloquer le fil principal lorsqu'elle est demandée depuis une autre page WebView2
/// (vignette, éditeur, historique). Rend faux si la limite `MAX_EPINGLES` est déjà atteinte.
pub fn ouvrir(app: &AppHandle, pixels: Arc<Pixels>, transparence: bool) -> bool {
    if pixels.largeur == 0 || pixels.hauteur == 0 {
        return false;
    }
    let actives = epingles(app).len();
    if !epingle::peut_ouvrir(actives) {
        log::info!("epingle refusee: limite atteinte ({actives})");
        let pour = app.clone();
        let _ = app.run_on_main_thread(move || {
            afficher(&pour, epingle::message_limite());
        });
        return false;
    }
    plus_tard_nomme(app, "epingle-ouvrir", Duration::ZERO, move |app| {
        ouvrir_maintenant(app, pixels, transparence);
    });
    true
}

fn ouvrir_maintenant(app: &AppHandle, pixels: Arc<Pixels>, transparence: bool) {
    let rang = {
        let garde = epingles(app);
        if !epingle::peut_ouvrir(garde.len()) {
            drop(garde);
            log::info!("epingle refusee: limite atteinte");
            afficher(app, epingle::message_limite());
            return;
        }
        garde.len()
    };
    let ecran = fenetre::ecran_courant(app, None);
    let (largeur, hauteur, echelle) = epingle::taille_initiale(
        pixels.largeur,
        pixels.hauteur,
        ecran.zone_utile.largeur,
        ecran.zone_utile.hauteur,
        ecran.echelle,
    );
    let id = app
        .state::<Application>()
        .prochaine_epingle
        .fetch_add(1, Ordering::SeqCst)
        + 1;
    // Insérée avant `creer` : si WebView2 appelle `epingle_pret` pendant la création, la
    // capture est déjà là.
    epingles(app).insert(
        id,
        EpingleOuverte {
            pixels: Arc::clone(&pixels),
            transparence,
            echelle_ecran: ecran.echelle,
            largeur,
            hauteur,
            echelle,
            opacite: epingle::OPACITE_MAX,
            identifiant: None,
        },
    );
    let pour_detruite = app.clone();
    match fenetre::creer(app, id, &ecran, largeur, hauteur, rang, move |detruite| {
        retirer(&pour_detruite, detruite);
    }) {
        Ok(hwnd) => {
            if let Some(e) = epingles(app).get_mut(&id) {
                e.identifiant = hwnd;
            }
            log::info!(
                "epingle ouverte {id} {}x{} (logique {largeur}x{hauteur})",
                pixels.largeur,
                pixels.hauteur
            );
        }
        Err(e) => {
            log::warn!("epingle non ouverte {id}: {e}");
            epingles(app).remove(&id);
            afficher(app, epingle::message_impossible());
        }
    }
}

fn retirer(app: &AppHandle, id: u64) {
    if epingles(app).remove(&id).is_some() {
        log::info!("epingle fermee {id}");
    }
}

/// La page `epingle-{id}` est prête : envoie le thème, montre la fenêtre et rend sa charge.
pub fn pret(app: &AppHandle, id: u64) -> Result<ChargeEpingle, String> {
    let charge = {
        let garde = epingles(app);
        let Some(e) = garde.get(&id) else {
            return Err("epingle inconnue".into());
        };
        ChargeEpingle {
            id,
            image_largeur: e.pixels.largeur,
            image_hauteur: e.pixels.hauteur,
            largeur: e.largeur,
            hauteur: e.hauteur,
            echelle: e.echelle,
            opacite: e.opacite,
        }
    };
    theme::envoyer_a(app, &fenetre::etiquette(id));
    fenetre::montrer(app, id);
    Ok(charge)
}

/// Pixels bruts RGBA de la capture épinglée `id`.
pub fn image(app: &AppHandle, id: u64) -> Result<Vec<u8>, String> {
    let pixels = epingles(app)
        .get(&id)
        .map(|e| Arc::clone(&e.pixels))
        .ok_or_else(|| "epingle inconnue".to_string())?;
    Ok(pixels.rgba.clone())
}

/// Redimensionne la capture épinglée `id` selon `facteur` (molette).
pub fn redimensionner(app: &AppHandle, id: u64, facteur: f64) -> Result<TailleEpingle, String> {
    let ecran = fenetre::ecran_courant(app, Some(id));
    let taille = {
        let mut garde = epingles(app);
        let Some(e) = garde.get_mut(&id) else {
            return Err("epingle inconnue".into());
        };
        e.echelle_ecran = ecran.echelle;
        let Some(t) = epingle::redimensionner(
            e.pixels.largeur,
            e.pixels.hauteur,
            e.echelle_ecran,
            e.echelle,
            facteur,
            ecran.zone_utile.largeur,
            ecran.zone_utile.hauteur,
        ) else {
            return Err("facteur invalide".into());
        };
        e.largeur = t.largeur;
        e.hauteur = t.hauteur;
        e.echelle = t.echelle;
        t
    };
    fenetre::redimensionner(app, id, taille.largeur, taille.hauteur);
    Ok(taille)
}

/// Met à jour l'opacité retenue pour la capture épinglée `id` (rend la valeur bornée).
pub fn opacite(app: &AppHandle, id: u64, opacite: f64) -> Result<f64, String> {
    let Some(bornee) = epingle::ajuster_opacite(opacite) else {
        return Err("opacite invalide".into());
    };
    let mut garde = epingles(app);
    let Some(e) = garde.get_mut(&id) else {
        return Err("epingle inconnue".into());
    };
    e.opacite = bornee;
    Ok(bornee)
}

/// Prépare `(pixels, demande)` pour copier la capture épinglée `id` dans le presse-papiers.
pub fn preparer_copie(app: &AppHandle, id: u64) -> Result<(Arc<Pixels>, sortie::Demande), String> {
    let (pixels, transparence) = epingles(app)
        .get(&id)
        .map(|e| (Arc::clone(&e.pixels), e.transparence))
        .ok_or_else(|| "epingle inconnue".to_string())?;
    let base = demande_sortie(app, transparence);
    let demande = sortie::Demande {
        copier: true,
        enregistrer: false,
        ranger_recents: false,
        maintenant: chrono::Local::now(),
        transparence,
        ..base
    };
    Ok((pixels, demande))
}

/// Copie les pixels d'une capture épinglée (exécutée hors du fil principal).
pub fn copier_pixels(id: u64, pixels: &Pixels, demande: &sortie::Demande) -> Result<(), String> {
    let bilan = sortir_ou_echec(id, pixels, demande);
    match bilan.copie {
        Etape::Faite => {
            log::info!("epingle copiee {id}");
            Ok(())
        }
        Etape::Echec(c) => {
            log::warn!("epingle copie impossible {id}: {c}");
            Err(c)
        }
        Etape::NonDemandee => Err("copie non demandee".into()),
    }
}

/// Ferme la capture épinglée `id`.
pub fn fermer(app: &AppHandle, id: u64) {
    retirer(app, id);
    fenetre::detruire(app, id);
}

/// Identifiants natifs des fenêtres épinglées ouvertes (pour `nos_fenetres`).
pub fn identifiants(app: &AppHandle) -> Vec<isize> {
    epingles(app)
        .values()
        .filter_map(|e| e.identifiant)
        .collect()
}

/// Ferme toutes les captures épinglées (à l'arrêt de l'application).
pub fn fermer_tout(app: &AppHandle) {
    let ids: Vec<u64> = {
        let mut garde = epingles(app);
        garde.drain().map(|(id, _)| id).collect()
    };
    for id in ids {
        fenetre::detruire(app, id);
    }
}
