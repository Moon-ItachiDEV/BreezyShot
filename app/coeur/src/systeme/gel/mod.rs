//! Le Gel : photographier tout le bureau et l'afficher figé dans une fenêtre native.
//!
//! Façade commune : `windows.rs` la réalise (GDI, écrans, fenêtres visibles, fenêtre native
//! de l'écran figé), `autre.rs` donne des réponses neutres ailleurs.
//! Les pixels restent dans une image du système ; ils ne sortent d'ici que par
//! `ImageFigee::lire`, au format `Pixels`, et seulement pour la zone demandée.
//!
//! Tout se compte en pixels réels, dans le repère du bureau virtuel de Windows (l'écran
//! principal commence en (0, 0) ; un écran à sa gauche a des `x` négatifs).

use std::sync::Arc;
use std::time::Duration;

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Rectangle en pixels réels. `x`, `y` : coin haut gauche ; le bord droit et le bord bas
/// sont exclus (`x + largeur` est le premier pixel dehors).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub largeur: i32,
    pub hauteur: i32,
}

/// Borne un calcul fait en `i64` dans les `i32`.
fn borner(v: i64) -> i32 {
    v.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

impl Rect {
    /// Rectangle à partir de ses bords gauche, haut, droit (exclu) et bas (exclu).
    /// Des bords inversés donnent un rectangle vide.
    pub fn depuis_bords(gauche: i32, haut: i32, droite: i32, bas: i32) -> Rect {
        Rect {
            x: gauche,
            y: haut,
            largeur: borner((droite as i64 - gauche as i64).max(0)),
            hauteur: borner((bas as i64 - haut as i64).max(0)),
        }
    }

    /// Bord droit (exclu), calculé sans débordement.
    pub fn droite(&self) -> i64 {
        self.x as i64 + self.largeur as i64
    }

    /// Bord bas (exclu), calculé sans débordement.
    pub fn bas(&self) -> i64 {
        self.y as i64 + self.hauteur as i64
    }

    /// Vrai si le rectangle ne contient aucun pixel.
    pub fn est_vide(&self) -> bool {
        self.largeur <= 0 || self.hauteur <= 0
    }

    /// Vrai si le pixel `(x, y)` est dans le rectangle.
    pub fn contient(&self, point: (i32, i32)) -> bool {
        let (px, py) = (point.0 as i64, point.1 as i64);
        !self.est_vide()
            && px >= self.x as i64
            && px < self.droite()
            && py >= self.y as i64
            && py < self.bas()
    }

    /// Vrai si `autre` est entièrement dans ce rectangle (un rectangle vide n'est dans rien).
    pub fn englobe(&self, autre: Rect) -> bool {
        !autre.est_vide()
            && autre.x >= self.x
            && autre.y >= self.y
            && autre.droite() <= self.droite()
            && autre.bas() <= self.bas()
    }

    /// Partie commune des deux rectangles ; `None` si elle est vide.
    pub fn intersection(&self, autre: Rect) -> Option<Rect> {
        if self.est_vide() || autre.est_vide() {
            return None;
        }
        let gauche = self.x.max(autre.x);
        let haut = self.y.max(autre.y);
        let droite = self.droite().min(autre.droite());
        let bas = self.bas().min(autre.bas());
        if droite <= gauche as i64 || bas <= haut as i64 {
            return None;
        }
        Some(Rect {
            x: gauche,
            y: haut,
            largeur: borner(droite - gauche as i64),
            hauteur: borner(bas - haut as i64),
        })
    }
}

/// Un écran, tel que le système le décrit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EcranInfo {
    /// Tout l'écran, en pixels réels du bureau virtuel.
    pub rect: Rect,
    /// L'écran sans la barre des tâches.
    pub zone_utile: Rect,
    /// Échelle d'affichage (1.0 = 100 %, 1.5 = 150 %).
    pub echelle: f64,
    /// L'écran principal.
    pub principal: bool,
}

/// Une fenêtre visible d'une autre app. La liste va du premier plan vers le fond.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FenetreInfo {
    /// Le vrai bord, sans l'ombre (`DWMWA_EXTENDED_FRAME_BOUNDS`), en pixels réels.
    pub rect: Rect,
    /// Jamais écrit au journal.
    pub titre: String,
    pub coins: Coins,
    /// Identifiant système de la fenêtre (Windows : le HWND ; 0 ailleurs). Sert à la
    /// capture défilante (0.6) pour chercher la page qui défile dans cette fenêtre.
    pub fenetre: isize,
}

/// Forme des coins d'une fenêtre, approchée : Windows ne donne pas le rayon des coins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coins {
    Droits,
    /// Rayon de 8 pixels à 100 % (Windows 11).
    Arrondis,
    /// Rayon de 4 pixels à 100 % (menus, petites fenêtres).
    PetitsArrondis,
}

/// Pixels lus dans l'image figée : 4 octets par pixel (rouge, vert, bleu, alpha = 255,
/// sauf après `capture::decoupe::arrondir`), ligne par ligne de haut en bas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pixels {
    pub largeur: u32,
    pub hauteur: u32,
    pub rgba: Vec<u8>,
}

/// La photo du bureau, gardée en mémoire par le système (opaque).
///
/// Une seule image existe à la fois : une nouvelle photo remplace la précédente (l'image
/// est réutilisée tant que la taille du bureau ne change pas). Une `ImageFigee` dépassée
/// par une photo plus récente refuse de se lire ou de se montrer, plutôt que de rendre
/// les pixels d'une autre photo.
#[derive(Debug)]
pub struct ImageFigee {
    interne: plateforme::Image,
}

impl ImageFigee {
    /// Le bureau virtuel photographié.
    pub fn rect(&self) -> Rect {
        plateforme::rect_image(&self.interne)
    }

    /// Durée de la copie du bureau (le `BitBlt` seul).
    pub fn duree_photo(&self) -> Duration {
        plateforme::duree_image(&self.interne)
    }

    /// Lit une zone (en coordonnées du bureau). La zone est d'abord coupée au bureau ;
    /// une zone vide ou entièrement dehors est une erreur. Utilisable depuis n'importe
    /// quel fil.
    pub fn lire(&self, zone: Rect) -> Result<Pixels, String> {
        plateforme::lire(&self.interne, zone)
    }
}

/// Vrai si le Gel existe sur ce système.
pub fn disponible() -> bool {
    plateforme::DISPONIBLE
}

/// Photographie tout le bureau virtuel (tous les écrans d'un coup). Utilisable depuis
/// n'importe quel fil. Erreur tant que l'écran figé est montré : on ne remplace jamais
/// l'image affichée. Hors Windows : erreur « non disponible sur ce système ».
pub fn figer_bureau() -> Result<ImageFigee, String> {
    plateforme::figer_bureau().map(|interne| ImageFigee { interne })
}

/// Rend la mémoire de l'image du bureau (elle reste allouée entre deux captures pour la
/// vitesse). Cache d'abord l'écran figé. Utilisable depuis n'importe quel fil.
pub fn liberer_image() {
    plateforme::liberer_image()
}

/// Les écrans, triés de gauche à droite puis de haut en bas. N'importe quel fil.
pub fn ecrans() -> Vec<EcranInfo> {
    let mut liste = plateforme::ecrans();
    trier_ecrans(&mut liste);
    liste
}

/// Les fenêtres visibles des autres apps, du premier plan vers le fond. N'importe quel fil.
pub fn fenetres_visibles() -> Vec<FenetreInfo> {
    plateforme::fenetres_visibles()
}

/// Crée la fenêtre native de l'écran figé, cachée. À appeler une fois, sur le fil
/// principal (celui de la boucle d'événements, qui distribuera ses messages). Un second
/// appel sur le même fil ne fait rien.
pub fn preparer_ecran_fige() -> Result<(), String> {
    plateforme::preparer_ecran_fige()
}

/// Montre l'écran figé par-dessus tout le bureau, peint tout de suite, sans prendre le
/// focus. Fil principal seulement (celui de `preparer_ecran_fige`), sinon erreur.
pub fn montrer_ecran_fige(image: &ImageFigee) -> Result<(), String> {
    plateforme::montrer_ecran_fige(&image.interne)
}

/// Cache l'écran figé. Sûr à tout moment et depuis n'importe quel fil : hors du fil
/// principal, la demande est postée sans attendre.
pub fn cacher_ecran_fige() {
    plateforme::cacher_ecran_fige()
}

/// Détruit la fenêtre de l'écran figé (arrêt de l'app). N'importe quel fil : hors du fil
/// principal, la fermeture est postée.
pub fn detruire_ecran_fige() {
    plateforme::detruire_ecran_fige()
}

/// À appeler une fois par l'Application, depuis n'importe quel fil. Tant que l'écran figé
/// est montré, un fil à part vérifie toutes les 500 ms que le fil de la fenêtre répond
/// (`SendMessageTimeoutW`, `WM_NULL`, `SMTO_ABORTIFHUNG`, 1 s). Après `delai` sans
/// réponse, il appelle `au_blocage` (une seule fois par blocage), depuis ce fil à part.
/// Au repos, le fil de garde dort jusqu'au prochain affichage. Un second appel remplace
/// le délai et le rappel. Hors Windows : ne fait rien.
pub fn garder_ecran_fige(delai: Duration, au_blocage: impl Fn() + Send + 'static) {
    plateforme::garder_ecran_fige(delai, Box::new(au_blocage))
}

/// Ce qui a changé sur le bureau pendant que l'écran figé était montré.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Changement {
    /// Écrans branchés, débranchés, déplacés ou changés de résolution.
    Ecrans,
    /// La session a été verrouillée.
    SessionVerrouillee,
}

/// À appeler une fois par l'Application, depuis n'importe quel fil. Quand les écrans
/// changent ou que la session se verrouille pendant que l'écran figé est montré, celui-ci
/// se cache, puis `rappel` est appelé sur le fil de la fenêtre figée (le fil principal),
/// sans aucun verrou tenu. Rien n'est appelé si l'écran figé était caché. Un second appel
/// remplace le rappel. Hors Windows : ne fait rien.
pub fn quand_bureau_change(rappel: impl Fn(Changement) + Send + Sync + 'static) {
    plateforme::quand_bureau_change(Arc::new(rappel))
}

/// Vrai si la fenêtre de l'écran figé reçoit le verrouillage de session
/// (`WTSRegisterSessionNotification` a réussi à sa création). Hors Windows : faux.
pub fn surveillance_session() -> bool {
    plateforme::surveillance_session()
}

/// Le nombre de verrouillages de session vus depuis le lancement, que l'écran figé soit
/// montré ou caché (la fenêtre figée reste créée et inscrite) : la capture défilante le
/// relit pendant le défilement (R9). Ne compte que si `surveillance_session()`. Hors
/// Windows : 0.
pub fn verrouillages_de_session() -> u64 {
    plateforme::verrouillages_de_session()
}

/// Le nombre de mises en veille annoncées (`WM_POWERBROADCAST` / `PBT_APMSUSPEND`) reçues
/// par la fenêtre de l'écran figé depuis le lancement, qu'il soit montré ou caché : le fil
/// `film` le relit à chacun de ses tours (contrat 0.7, F34, sur le modèle de
/// `verrouillages_de_session`). N'importe quel fil. Hors Windows : 0.
///
/// Socle : toujours 0 (chantier A).
pub fn mises_en_veille() -> u64 {
    plateforme::mises_en_veille()
}

/// Pose le rappel de la fin de session Windows (`WM_QUERYENDSESSION` reçu par la fenêtre
/// de l'écran figé : déconnexion, arrêt ou redémarrage) : il est appelé sur le fil de la
/// fenêtre et doit rendre la main en 3 s au plus (il ferme le fichier d'un film, F35). Un
/// second appel remplace le rappel. Hors Windows : ne fait rien.
///
/// Socle : sans effet, le rappel n'est jamais appelé (chantier A).
pub fn quand_session_finit(rappel: Box<dyn Fn() + Send + Sync>) {
    plateforme::quand_session_finit(rappel)
}

/// Vrai tant que l'écran figé est montré : marqué « montré » (dès `montrer_ecran_fige`,
/// jusqu'à la demande de le cacher) ou encore visible (masquage posté, pas encore traité).
/// La garde de `figer_bureau`, pour les autres prises de l'écran (`vivant`). Porte sur la
/// fenêtre, pas sur la dernière image composée : juste après le masquage, l'écran peut
/// encore montrer l'image figée une ou deux périodes. N'importe quel fil. Hors Windows : faux.
pub fn ecran_fige_montre() -> bool {
    plateforme::ecran_fige_montre()
}

/// Le bureau virtuel (tous les écrans), en pixels réels. Erreur hors Windows ou si le
/// système rend une taille nulle. N'importe quel fil.
pub fn bureau_virtuel() -> Result<Rect, String> {
    plateforme::bureau_virtuel()
}

/// Verrou de séance des tests qui touchent à l'écran (fenêtres, écran figé, objets GDI) :
/// partagé par les tests de `gel` et de `vivant`, un seul à la fois dans le processus.
#[cfg(all(test, windows))]
pub(crate) static VERROU_ECRAN: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Poignée système de la fenêtre de l'écran figé, pour poser la page de sélection juste
/// au-dessus. `None` tant qu'elle n'est pas créée. N'importe quel fil.
pub fn poignee_ecran_fige() -> Option<isize> {
    plateforme::poignee_ecran_fige()
}

/// Le pointeur de la souris, lu juste après la photo (0.4), pour être posé dans les pixels
/// découpés. Les deux images sont en RGBA, ligne par ligne de haut en bas : le pointeur
/// peint sur un fond noir, puis sur un fond blanc (leur différence donne la transparence et
/// les parties inversées). Jamais écrit au journal (ni forme ni nom).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pointeur {
    /// Position de la pointe moins le point chaud, en pixels réels du bureau.
    pub origine: (i32, i32),
    pub largeur: u32,
    pub hauteur: u32,
    pub sur_noir: Vec<u8>,
    pub sur_blanc: Vec<u8>,
}

/// Lit le pointeur tel que Windows le montre (`GetCursorInfo`, `GetIconInfo`, puis
/// `DrawIconEx` sur noir et sur blanc ; un pointeur animé donne sa première image).
/// `Ok(None)` : pointeur caché (écran tactile, saisie). Erreur : un texte court, sans chemin
/// ni titre. N'importe quel fil. Hors Windows : `Ok(None)`.
pub fn lire_pointeur() -> Result<Option<Pointeur>, String> {
    plateforme::lire_pointeur()
}

/// Valeurs de `CURSORINFO.flags` (winuser.h), recopiées pour la règle pure ; un test
/// Windows les compare à celles de la crate `windows`.
pub const POINTEUR_MONTRE: u32 = 0x1;
pub const POINTEUR_SUPPRIME: u32 = 0x2;

/// Vrai si Windows montre le pointeur : `CURSOR_SHOWING` posé et `CURSOR_SUPPRESSED`
/// absent (écran tactile ou stylet : le pointeur est caché).
pub fn pointeur_montre(drapeaux: u32) -> bool {
    drapeaux & POINTEUR_MONTRE != 0 && drapeaux & POINTEUR_SUPPRIME == 0
}

/// Plus grand côté accepté pour un pointeur (Windows va jusqu'à 256 en Accessibilité).
pub const POINTEUR_COTE_MAX: i32 = 512;

/// Taille d'un pointeur d'après ses bitmaps (`GetObjectW`) : celle de l'image en couleurs
/// si elle existe, sinon le masque seul, qui empile le masque ET et le masque XOR (moitié
/// de sa hauteur). `None` si la taille est nulle ou dépasse `POINTEUR_COTE_MAX`.
pub fn taille_pointeur(masque: (i32, i32), couleurs: Option<(i32, i32)>) -> Option<(u32, u32)> {
    let (largeur, hauteur) = match couleurs {
        Some(t) => t,
        None => (masque.0, masque.1 / 2),
    };
    let bon = |v: i32| v > 0 && v <= POINTEUR_COTE_MAX;
    (bon(largeur) && bon(hauteur)).then_some((largeur as u32, hauteur as u32))
}

/// Ordre des écrans : de gauche à droite, puis de haut en bas.
pub fn trier_ecrans(ecrans: &mut [EcranInfo]) {
    ecrans.sort_by_key(|e| (e.rect.x, e.rect.y));
}

/// Échelle d'un écran à partir de son nombre de points par pouce (96 = 100 %).
/// Une valeur nulle (inconnue) donne 1.0.
pub fn echelle_depuis_dpi(dpi: u32) -> f64 {
    if dpi == 0 {
        1.0
    } else {
        dpi as f64 / 96.0
    }
}

/// Ce que le système dit d'une fenêtre, pour décider si on la garde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EtatFenetre {
    pub visible: bool,
    pub reduite: bool,
    /// Masquée par Windows (`DWMWA_CLOAKED`) : autre bureau, app suspendue…
    pub masquee: bool,
    pub de_breezyshot: bool,
    /// Fenêtre en calque (`WS_EX_LAYERED`).
    pub calque: bool,
    /// `WS_EX_TRANSPARENT`. Avec `calque`, les clics la traversent : voile invisible
    /// d'une autre app. Seul, il ne rend pas une fenêtre de premier niveau traversable.
    pub transparente: bool,
    /// Le bureau lui-même (`est_classe_du_bureau`).
    pub bureau: bool,
}

/// Règle du mode « fenêtre » : visible, non réduite, non masquée, pas à BreezyShot, ni
/// traversée par les clics (calque et transparente à la fois), ni le bureau, et d'une
/// taille non nulle. La barre des tâches reste une fenêtre comme une autre.
pub fn garder_fenetre(etat: EtatFenetre, rect: Rect) -> bool {
    etat.visible
        && !etat.reduite
        && !etat.masquee
        && !etat.de_breezyshot
        && !(etat.calque && etat.transparente)
        && !etat.bureau
        && !rect.est_vide()
}

/// Vrai pour les classes de fenêtre du bureau (le fond d'écran et ses icônes).
pub fn est_classe_du_bureau(classe: &str) -> bool {
    classe == "Progman" || classe == "WorkerW"
}

/// Première build de Windows 11 (coins arrondis par DWM).
pub const BUILD_WINDOWS_11: u32 = 22000;

/// Valeurs de `DWM_WINDOW_CORNER_PREFERENCE` (dwmapi.h), recopiées ici pour la règle pure ;
/// un test Windows les compare à celles de la crate `windows`.
pub const PREFERENCE_DEFAUT: i32 = 0;
pub const PREFERENCE_NE_PAS_ARRONDIR: i32 = 1;
pub const PREFERENCE_ARRONDIR: i32 = 2;
pub const PREFERENCE_ARRONDIR_PEU: i32 = 3;

/// Ce que le système dit d'une fenêtre, pour approcher la forme de ses coins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EtatCoins {
    /// Build de Windows ; `None` si inconnue.
    pub build: Option<u32>,
    /// Agrandie (`IsZoomed`).
    pub agrandie: bool,
    /// Ancrée sur un bord ou un quart d'écran (`IsWindowArranged`).
    pub ancree: bool,
    /// `DWMWA_WINDOW_CORNER_PREFERENCE` lue ; `None` si la lecture a échoué.
    pub preference: Option<i32>,
    /// `WS_CAPTION` entier ou `WS_THICKFRAME`.
    pub a_un_cadre: bool,
}

/// Règle des coins (approximation assumée) :
/// 1. build inconnue ou d'avant Windows 11 : droits ;
/// 2. agrandie ou ancrée : droits ;
/// 3. préférence lue : « ne pas arrondir » → droits, « arrondir » → arrondis,
///    « arrondir peu » → petits arrondis ; « défaut », valeur inconnue ou lecture
///    impossible : point 4 ;
/// 4. arrondis si la fenêtre a un cadre, sinon droits (barre des tâches, plein écran…).
pub fn coins_de(etat: EtatCoins) -> Coins {
    if etat.build.is_none_or(|b| b < BUILD_WINDOWS_11) || etat.agrandie || etat.ancree {
        return Coins::Droits;
    }
    match etat.preference {
        Some(PREFERENCE_NE_PAS_ARRONDIR) => Coins::Droits,
        Some(PREFERENCE_ARRONDIR) => Coins::Arrondis,
        Some(PREFERENCE_ARRONDIR_PEU) => Coins::PetitsArrondis,
        _ if etat.a_un_cadre => Coins::Arrondis,
        _ => Coins::Droits,
    }
}

/// Convertit sur place des pixels BGRA (ordre de Windows) en RGBA, alpha forcé à 255
/// (GDI le laisse à 0). Les octets qui ne forment pas un pixel entier restent tels quels.
pub fn bgra_en_rgba(tampon: &mut [u8]) {
    for px in tampon.chunks_exact_mut(4) {
        px.swap(0, 2);
        px[3] = 255;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, largeur: i32, hauteur: i32) -> Rect {
        Rect {
            x,
            y,
            largeur,
            hauteur,
        }
    }

    #[test]
    fn intersection_et_bords() {
        let a = r(-1920, 0, 1920, 1080);
        let b = r(-100, 500, 300, 300);
        assert_eq!(a.intersection(b), Some(r(-100, 500, 100, 300)));
        assert_eq!(
            a.intersection(r(0, 0, 10, 10)),
            None,
            "bords qui se touchent"
        );
        assert_eq!(a.intersection(r(0, 0, 0, 0)), None);
        assert!(a.contient((-1920, 0)));
        assert!(!a.contient((0, 0)));
        assert!(a.englobe(r(-1920, 0, 1920, 1080)));
        assert!(!a.englobe(r(-1920, 0, 1921, 1080)));
        assert_eq!(Rect::depuis_bords(10, 10, 5, 20), r(10, 10, 0, 10));
    }

    #[test]
    fn pas_de_debordement() {
        let grand = r(i32::MAX - 1, i32::MAX - 1, i32::MAX, i32::MAX);
        assert!(grand.contient((i32::MAX, i32::MAX)));
        let i = grand.intersection(r(0, 0, i32::MAX, i32::MAX));
        assert_eq!(i, Some(r(i32::MAX - 1, i32::MAX - 1, 1, 1)));
        assert_eq!(
            Rect::depuis_bords(i32::MIN, 0, i32::MAX, 1).largeur,
            i32::MAX
        );
    }

    #[test]
    fn pointeur_montre_ou_cache() {
        assert!(pointeur_montre(POINTEUR_MONTRE));
        assert!(!pointeur_montre(0));
        assert!(!pointeur_montre(POINTEUR_SUPPRIME));
        assert!(!pointeur_montre(POINTEUR_MONTRE | POINTEUR_SUPPRIME));
    }

    #[test]
    fn taille_du_pointeur() {
        assert_eq!(taille_pointeur((32, 32), Some((32, 32))), Some((32, 32)));
        assert_eq!(taille_pointeur((48, 48), Some((48, 48))), Some((48, 48)));
        // Masque seul (pointeur monochrome) : la moitié de sa hauteur.
        assert_eq!(taille_pointeur((32, 64), None), Some((32, 32)));
        assert_eq!(taille_pointeur((32, 1), None), None);
        assert_eq!(taille_pointeur((0, 0), Some((0, 0))), None);
        assert_eq!(taille_pointeur((32, 32), Some((-32, 32))), None);
        assert_eq!(taille_pointeur((32, 32), Some((513, 32))), None);
        assert_eq!(
            taille_pointeur((32, 32), Some((512, 512))),
            Some((512, 512))
        );
    }

    #[test]
    fn conversion_bgra_rgba() {
        // Deux pixels : bleu pur puis (b=1, v=2, r=3), alpha à 0 comme le laisse GDI,
        // et deux octets en trop.
        let mut t = vec![255, 0, 0, 0, 1, 2, 3, 0, 9, 9];
        bgra_en_rgba(&mut t);
        assert_eq!(t, vec![0, 0, 255, 255, 3, 2, 1, 255, 9, 9]);
    }

    #[test]
    fn ecrans_tries() {
        let e = |x, y| EcranInfo {
            rect: r(x, y, 100, 100),
            zone_utile: r(x, y, 100, 90),
            echelle: 1.0,
            principal: x == 0 && y == 0,
        };
        let mut liste = vec![e(1920, 0), e(0, 0), e(-1280, 200), e(0, -1080)];
        trier_ecrans(&mut liste);
        let ordre: Vec<(i32, i32)> = liste.iter().map(|e| (e.rect.x, e.rect.y)).collect();
        assert_eq!(ordre, vec![(-1280, 200), (0, -1080), (0, 0), (1920, 0)]);
    }

    #[test]
    fn echelle() {
        assert_eq!(echelle_depuis_dpi(96), 1.0);
        assert_eq!(echelle_depuis_dpi(144), 1.5);
        assert_eq!(echelle_depuis_dpi(0), 1.0);
    }

    #[test]
    fn regle_des_fenetres() {
        let ok = EtatFenetre {
            visible: true,
            reduite: false,
            masquee: false,
            de_breezyshot: false,
            calque: false,
            transparente: false,
            bureau: false,
        };
        let taille = r(0, 0, 10, 10);
        assert!(garder_fenetre(ok, taille));
        assert!(garder_fenetre(ok, r(-3000, -200, 1, 1)));
        // Rectangle vide : jamais gardée.
        assert!(!garder_fenetre(ok, r(0, 0, 0, 10)));
        assert!(!garder_fenetre(ok, r(0, 0, 10, 0)));
        assert!(!garder_fenetre(ok, r(0, 0, -5, 10)));
        // Calque seul, ou transparente seule : les clics ne la traversent pas.
        assert!(garder_fenetre(EtatFenetre { calque: true, ..ok }, taille));
        assert!(garder_fenetre(
            EtatFenetre {
                transparente: true,
                ..ok
            },
            taille
        ));
        let refusees = [
            EtatFenetre {
                visible: false,
                ..ok
            },
            EtatFenetre {
                reduite: true,
                ..ok
            },
            EtatFenetre {
                masquee: true,
                ..ok
            },
            EtatFenetre {
                de_breezyshot: true,
                ..ok
            },
            EtatFenetre {
                calque: true,
                transparente: true,
                ..ok
            },
            EtatFenetre { bureau: true, ..ok },
        ];
        for etat in refusees {
            assert!(!garder_fenetre(etat, taille), "{etat:?}");
        }
    }

    #[test]
    fn classes_du_bureau() {
        assert!(est_classe_du_bureau("Progman"));
        assert!(est_classe_du_bureau("WorkerW"));
        // La barre des tâches reste une fenêtre comme une autre.
        assert!(!est_classe_du_bureau("Shell_TrayWnd"));
        assert!(!est_classe_du_bureau("progman"));
        assert!(!est_classe_du_bureau(""));
    }

    #[test]
    fn regle_des_coins() {
        let base = EtatCoins {
            build: Some(26100),
            agrandie: false,
            ancree: false,
            preference: None,
            a_un_cadre: true,
        };
        // 4. Par défaut : selon le cadre.
        assert_eq!(coins_de(base), Coins::Arrondis);
        assert_eq!(
            coins_de(EtatCoins {
                a_un_cadre: false,
                ..base
            }),
            Coins::Droits
        );
        // 1. Avant Windows 11, ou build inconnue.
        for build in [None, Some(19045), Some(BUILD_WINDOWS_11 - 1)] {
            let etat = EtatCoins {
                build,
                preference: Some(PREFERENCE_ARRONDIR),
                ..base
            };
            assert_eq!(coins_de(etat), Coins::Droits, "{etat:?}");
        }
        assert_eq!(
            coins_de(EtatCoins {
                build: Some(BUILD_WINDOWS_11),
                ..base
            }),
            Coins::Arrondis
        );
        // 2. Agrandie ou ancrée : droits, quelle que soit la préférence.
        for (agrandie, ancree) in [(true, false), (false, true), (true, true)] {
            let etat = EtatCoins {
                agrandie,
                ancree,
                preference: Some(PREFERENCE_ARRONDIR),
                ..base
            };
            assert_eq!(coins_de(etat), Coins::Droits, "{etat:?}");
        }
        // 3. Préférence lue, avec ou sans cadre.
        for a_un_cadre in [true, false] {
            let avec = |preference| EtatCoins {
                preference,
                a_un_cadre,
                ..base
            };
            assert_eq!(
                coins_de(avec(Some(PREFERENCE_NE_PAS_ARRONDIR))),
                Coins::Droits
            );
            assert_eq!(coins_de(avec(Some(PREFERENCE_ARRONDIR))), Coins::Arrondis);
            assert_eq!(
                coins_de(avec(Some(PREFERENCE_ARRONDIR_PEU))),
                Coins::PetitsArrondis
            );
            let selon_cadre = if a_un_cadre {
                Coins::Arrondis
            } else {
                Coins::Droits
            };
            assert_eq!(coins_de(avec(Some(PREFERENCE_DEFAUT))), selon_cadre);
            assert_eq!(coins_de(avec(Some(42))), selon_cadre);
            assert_eq!(coins_de(avec(None)), selon_cadre);
        }
    }
}
