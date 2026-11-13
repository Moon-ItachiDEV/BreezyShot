//! Capture défilante (contrat 0.6) : les décisions du cœur, en fonctions pures (§4.2, §5,
//! §8, §10). `geste_defilante.rs` fait les gestes (fils, fenêtre de contrôle, sortie) ;
//! `fenetre_defilante.rs` reçoit les quatre commandes de la page de contrôle.
//!
//! Les sous-états du défilement (`Conduite`, `perdue`) ne sont pas des états de
//! l'automate : ils ne changent rien à ce que les autres demandes peuvent faire.
//!
//! La course (`Course`, `suite`) décide, image après image, de ce que le fil `defilante`
//! fait : un pas, une prise, un message, l'aperçu, un repli de moyen, la fin. Le moyen de
//! défilement se choisit sur les deux premières commandes (R3, §18) : une avance mesurée
//! sur les images le garde pour toute la capture ; deux images immobiles l'abandonnent
//! pour le suivant ; après les trois, la main passe à la molette de l'utilisateur, sans
//! rien montrer (D8, U5).

use std::time::{Duration, Instant};

use super::editeur;
pub use crate::defilement::RECOUVREMENT_MIN;
use crate::defilement::{Fixes, Limite, Mesure};
use crate::systeme::defiler::{Moyen, PageTrouvee, Position};
use crate::systeme::gel::EcranInfo;
use crate::systeme::Rect;

/// Vrai quand la capture défilante est construite (chantier D) : avant, elle n'est jamais
/// une action construite (ni menu, ni raccourci, ni `--capturer`), même avec une licence.
/// Hors Windows, `systeme::vivant::disponible()` reste faux : la fonction y est absente.
pub const CONSTRUITE: bool = true;

/// Côté minimal d'une zone, en pixels logiques : une zone plus petite est agrandie autour
/// de son centre (D3) ; une page proposée plus petite n'est pas proposée (D2).
pub const COTE_MIN_LOGIQUE: f64 = 120.0;

/// Au plus tant de pages proposées à la Sélection (D2).
pub const PAGES_MAX: usize = 32;

/// Les pages se cherchent dans tant de fenêtres, du premier plan vers le fond (S4).
pub const FENETRES_FOUILLEES: usize = 8;

/// Attente après la première image avant le premier pas automatique (valeur du design).
pub const DEPART_AUTO: Duration = Duration::from_millis(450);

/// Arrêt en bas de la page avant la fin seule (valeur du design, D15).
pub const FIN_EN_BAS: Duration = Duration::from_millis(350);

/// Intervalle des prises une fois la main reprise à la molette (D9).
pub const PRISE_MAIN: Duration = Duration::from_millis(50);

/// Intervalle des prises après un pas, jusqu'à deux images égales (D6).
pub const PRISE_POSEE: Duration = Duration::from_millis(33);

/// Attente maximale d'une page posée après un pas (D6, S5).
pub const ATTENTE_POSEE_MAX: Duration = Duration::from_millis(400);

/// Pas minimal, en pixels réels (D6).
pub const PAS_MIN: i32 = 40;

/// Échéance globale d'une défilante (D20).
pub const DUREE_MAX: Duration = Duration::from_secs(10 * 60);

/// En `Main`, sans aucune image nouvelle pendant ce délai, la capture finit (D20).
pub const MAIN_SANS_IMAGE_MAX: Duration = Duration::from_secs(2 * 60);

/// Sans page de contrôle prête dans ce délai après le relâchement, abandon (D4, R1).
pub const DELAI_PAGE: Duration = Duration::from_secs(2);

/// L'aperçu en direct part au plus tant de fois par seconde (§9) : un envoi toutes les…
pub const APERCU_INTERVALLE: Duration = Duration::from_millis(100);

/// Prises en échec de suite avant la fin (D19).
pub const PRISES_EN_ECHEC_MAX: u32 = 5;

/// Position lue à partir de laquelle la page est en bas (S5, D15), en pour cent.
pub const EN_BAS_POUR_CENT: f64 = 99.9;

/// En `Auto`, tant d'images `Perdue` de suite font remonter ; au-delà, la main passe à la
/// molette sans rien montrer (la page va trop loin à chaque pas, même réduit).
pub const PERDUES_AUTO_MAX: u32 = 4;

/// En `Auto`, tant d'images `Ambigue` de suite comptent pour une image immobile (une page
/// unie plus haute que la zone ne fait pas défiler sans fin).
pub const AMBIGUES_MAX: u32 = 6;

/// Texte de la pilule de la page de contrôle.
pub const LIBELLE_TERMINER: &str = "Terminer";

/// Message « trop vite » (D10), montré en haut de l'écran.
pub const TEXTE_TROP_VITE: &str = "Trop vite. Remontez un peu.";

/// Titre du bandeau quand la page de contrôle n'est pas prête à temps (R1, §18).
pub const TITRE_LANCEMENT_IMPOSSIBLE: &str = "Impossible de lancer la capture défilante";

/// Largeur de l'aperçu en direct, en pixels logiques (jeton `defilante-apercu-largeur`).
pub const APERCU_LARGEUR_LOGIQUE: f64 = 84.0;
/// Écart entre la zone et l'aperçu, en pixels logiques (jeton `defilante-ecart`).
pub const ECART_LOGIQUE: f64 = 12.0;
/// Marge gardée au bord de la zone utile de l'écran, en pixels logiques (design).
pub const MARGE_LOGIQUE: f64 = 8.0;
/// Écart entre la zone et la pilule, en pixels logiques (design).
pub const ECART_PILULE_LOGIQUE: f64 = 10.0;
/// Distance du message au haut de l'écran, en pixels logiques (design).
pub const HAUT_MESSAGE_LOGIQUE: f64 = 14.0;
/// Taille de la pilule « Terminer » prévue pour la placer, en pixels logiques (bouton de
/// 32 px, 4 px de marge, 1 px de bord ; « Terminer » à 14 px) : la page rapporte ensuite
/// la vraie (`defilante_zone`).
pub const PILULE_LARGEUR_LOGIQUE: f64 = 110.0;
pub const PILULE_HAUTEUR_LOGIQUE: f64 = 42.0;
/// Hauteur gardée à l'aperçu quand la zone touche le bas de l'écran, en pixels logiques.
pub const APERCU_HAUTEUR_MIN_LOGIQUE: f64 = 40.0;

/// L'échelle d'un écran, sûre (1.0 si elle n'est pas un nombre positif).
fn echelle(ecran: &EcranInfo) -> f64 {
    if ecran.echelle.is_finite() && ecran.echelle > 0.0 {
        ecran.echelle
    } else {
        1.0
    }
}

/// `logique` pixels logiques en pixels réels sur un écran d'échelle `s`.
fn reels(logique: f64, s: f64) -> i32 {
    (logique * s).round() as i32
}

/// Bord droit et bas, bornés aux `i32` (les rectangles des écrans y tiennent).
fn droite(r: Rect) -> i32 {
    r.droite().clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

fn bas(r: Rect) -> i32 {
    r.bas().clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

/// Porte `[debut, debut + long)` à `min` autour de son centre, dans `[borne, fin)`.
fn etendre(debut: i32, long: i32, min: i32, borne: i32, fin: i32) -> (i32, i32) {
    if long >= min {
        return (debut, long);
    }
    let place = (fin as i64 - borne as i64).max(0);
    let min = (min as i64).min(place);
    let depart = (2 * debut as i64 + long as i64 - min).div_euclid(2);
    let depart = depart.clamp(borne as i64, (fin as i64 - min).max(borne as i64));
    (depart as i32, min as i32)
}

/// La zone `zone` (pixels réels) portée à `round(COTE_MIN_LOGIQUE × échelle)` sur chaque
/// côté trop petit, autour de son centre, sans sortir de `ecran` (D3). Rend la zone et si
/// elle a été agrandie.
pub fn agrandir(zone: Rect, ecran: &EcranInfo) -> (Rect, bool) {
    let min = reels(COTE_MIN_LOGIQUE, echelle(ecran));
    let e = ecran.rect;
    let (x, largeur) = etendre(zone.x, zone.largeur, min, e.x, droite(e));
    let (y, hauteur) = etendre(zone.y, zone.hauteur, min, e.y, bas(e));
    let nouvelle = Rect {
        x,
        y,
        largeur,
        hauteur,
    };
    (nouvelle, nouvelle != zone)
}

/// Surface commune de deux rectangles, en pixels.
fn surface_commune(a: Rect, b: Rect) -> i64 {
    a.intersection(b)
        .map(|r| r.largeur as i64 * r.hauteur as i64)
        .unwrap_or(0)
}

/// L'écran qui contient le plus de la surface de `rect` ; `None` s'il n'en touche aucun.
pub fn ecran_de(rect: Rect, ecrans: &[EcranInfo]) -> Option<EcranInfo> {
    ecrans
        .iter()
        .map(|e| (surface_commune(rect, e.rect), e))
        .filter(|(s, _)| *s > 0)
        // Le premier des écrans à surface égale (`max_by_key` garde le dernier).
        .fold(
            None,
            |meilleur: Option<(i64, &EcranInfo)>, (s, e)| match meilleur {
                Some((m, _)) if m >= s => meilleur,
                _ => Some((s, e)),
            },
        )
        .map(|(_, e)| *e)
}

/// Les pages à proposer à la Sélection (D2) : chacune coupée à l'écran qui contient le
/// plus de sa surface, gardée si ses deux côtés ont `round(COTE_MIN_LOGIQUE × échelle)`
/// pixels sur cet écran ; au plus `PAGES_MAX`, dans l'ordre reçu. Rend le rectangle et la
/// fenêtre de chaque page.
pub fn pages_proposees(trouvees: &[PageTrouvee], ecrans: &[EcranInfo]) -> Vec<(Rect, isize)> {
    trouvees
        .iter()
        .filter_map(|p| {
            let ecran = ecran_de(p.rect, ecrans)?;
            let coupee = p.rect.intersection(ecran.rect)?;
            let min = reels(COTE_MIN_LOGIQUE, echelle(&ecran));
            (coupee.largeur >= min && coupee.hauteur >= min).then_some((coupee, p.fenetre))
        })
        .take(PAGES_MAX)
        .collect()
}

/// Où poser l'aperçu en direct, la pilule « Terminer » et le message, en pixels réels du
/// bureau (§10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Placement {
    /// L'aperçu : son rectangle, la hauteur étant la hauteur maximale.
    pub apercu: Rect,
    /// Le point haut gauche de la pilule.
    pub pilule: (i32, i32),
    /// Le point haut centre du message.
    pub message: (i32, i32),
}

/// Vrai si `a` et `b` se recouvrent, `b` grossi de `marge` de chaque côté.
fn se_touchent(a: Rect, b: Rect, marge: i32) -> bool {
    let b = Rect {
        x: b.x - marge,
        y: b.y - marge,
        largeur: b.largeur + 2 * marge,
        hauteur: b.hauteur + 2 * marge,
    };
    a.intersection(b).is_some()
}

/// Placement de l'aperçu, de la pilule et du message autour de `zone` sur `ecran` (§10,
/// pur, pixels réels). Tout reste dans la zone utile de l'écran (barre des tâches exclue),
/// à `MARGE_LOGIQUE` de ses bords. Aperçu à droite de la zone, sinon à gauche, sinon dedans
/// à droite ; son haut est celui de la zone, borné ; sa hauteur va au plus jusqu'à la marge
/// du bas. Pilule centrée sous la zone, sinon au-dessus, sinon dedans en bas ; jamais sur
/// l'aperçu. Message centré en haut de la zone utile.
pub fn placer(zone: Rect, ecran: &EcranInfo) -> Placement {
    let s = echelle(ecran);
    let utile = if ecran.zone_utile.est_vide() {
        ecran.rect
    } else {
        ecran.zone_utile
    };
    let marge = reels(MARGE_LOGIQUE, s);
    let (gauche, haut) = (utile.x + marge, utile.y + marge);
    let (fin_x, fin_y) = (droite(utile) - marge, bas(utile) - marge);

    // L'aperçu.
    let largeur = reels(APERCU_LARGEUR_LOGIQUE, s);
    let ecart = reels(ECART_LOGIQUE, s);
    let mut ax = droite(zone) + ecart;
    if ax + largeur > fin_x {
        ax = zone.x - ecart - largeur;
        if ax < gauche {
            ax = droite(zone) - ecart - largeur;
        }
    }
    let ax = ax.clamp(gauche, (fin_x - largeur).max(gauche));
    let haut_max = (fin_y - reels(APERCU_HAUTEUR_MIN_LOGIQUE, s)).max(haut);
    let ay = zone.y.clamp(haut, haut_max);
    let apercu = Rect {
        x: ax,
        y: ay,
        largeur,
        hauteur: (fin_y - ay).max(0),
    };

    // La pilule.
    let (pl, ph) = (
        reels(PILULE_LARGEUR_LOGIQUE, s),
        reels(PILULE_HAUTEUR_LOGIQUE, s),
    );
    let ecart_pilule = reels(ECART_PILULE_LOGIQUE, s);
    let borne_x = |x: i32| x.clamp(gauche, (fin_x - pl).max(gauche));
    let mut px = borne_x(zone.x + zone.largeur / 2 - pl / 2);
    let mut py = bas(zone) + ecart_pilule;
    if py + ph > fin_y {
        py = zone.y - ecart_pilule - ph;
        if py < haut {
            py = bas(zone) - ecart_pilule - ph;
        }
    }
    let py = py.clamp(haut, (fin_y - ph).max(haut));
    let pilule = Rect {
        x: px,
        y: py,
        largeur: pl,
        hauteur: ph,
    };
    if se_touchent(pilule, apercu, marge) {
        let a_gauche = apercu.x - marge - pl;
        px = if a_gauche >= gauche {
            a_gauche
        } else {
            borne_x(droite(apercu) + marge)
        };
    }

    Placement {
        apercu,
        pilule: (px, py),
        message: (
            utile.x + utile.largeur / 2,
            utile.y + reels(HAUT_MESSAGE_LOGIQUE, s),
        ),
    }
}

/// Le rectangle de la pilule rapporté par la page (pixels CSS relatifs à la fenêtre posée
/// sur `ecran`), en pixels réels du bureau ; `None` s'il n'est pas un rectangle.
pub fn pilule_reelle(
    ecran: &EcranInfo,
    x: f64,
    y: f64,
    largeur: f64,
    hauteur: f64,
) -> Option<Rect> {
    let s = echelle(ecran);
    let valeurs = [x, y, largeur, hauteur];
    if valeurs.iter().any(|v| !v.is_finite() || v.abs() > 1.0e6) || largeur <= 0.0 || hauteur <= 0.0
    {
        return None;
    }
    let gauche = ecran.rect.x + (x * s).floor() as i32;
    let haut = ecran.rect.y + (y * s).floor() as i32;
    let droite = ecran.rect.x + ((x + largeur) * s).ceil() as i32;
    let bas = ecran.rect.y + ((y + hauteur) * s).ceil() as i32;
    let r = Rect::depuis_bords(gauche, haut, droite, bas);
    (!r.est_vide()).then_some(r)
}

/// La taille maximale de l'image assemblée (D13) : celle de l'éditeur
/// (`editeur::COTE_MAX` de haut, `editeur::PIXELS_MAX` en tout). L'assemblage applique les
/// deux à la largeur de la zone : pour `largeur` pixels, la hauteur atteinte est
/// `hauteur_max(largeur)`.
pub fn limite(largeur: u32) -> Limite {
    let _ = largeur;
    Limite {
        hauteur: editeur::COTE_MAX,
        pixels: editeur::PIXELS_MAX,
    }
}

/// La hauteur maximale d'une image de `largeur` pixels sous `limite` (D13).
pub fn hauteur_max(largeur: u32) -> u32 {
    let l = limite(largeur);
    (l.hauteur as u64).min(l.pixels / largeur.max(1) as u64) as u32
}

/// Le pas automatique pour une hauteur utile (zone moins les bandes fixes) : sa moitié,
/// jamais moins de `PAS_MIN` (D6).
pub fn pas_de(hauteur_utile: u32) -> i32 {
    ((hauteur_utile / 2).min(i32::MAX as u32) as i32).max(PAS_MIN)
}

/// La hauteur utile d'une zone de `hauteur` pixels avec ses bandes fixes.
pub fn hauteur_utile(hauteur: u32, fixes: Fixes) -> u32 {
    hauteur.saturating_sub(fixes.haut.saturating_add(fixes.bas))
}

/// Le décalage attendu d'un pas, en pixels, d'après deux positions lues (UI Automation)
/// et la hauteur utile de la zone (D10 : départage une mesure ambiguë). `None` sans les
/// deux positions, ou si la part visible n'est pas lisible.
pub fn decalage_attendu(
    avant: Option<Position>,
    apres: Option<Position>,
    hauteur_utile: u32,
) -> Option<i32> {
    let (a, b) = (avant?, apres?);
    let vue = b.vue;
    if !(vue.is_finite() && vue > 0.0 && vue < 100.0)
        || !a.pour_cent.is_finite()
        || !b.pour_cent.is_finite()
        || a.pour_cent < 0.0
        || b.pour_cent < 0.0
    {
        return None;
    }
    let h = hauteur_utile as f64;
    let parcours = h * 100.0 / vue - h;
    Some(((b.pour_cent - a.pour_cent) / 100.0 * parcours).round() as i32)
}

/// Vrai si les écrans ont changé (branchés, débranchés, déplacés, autre résolution ou
/// autre échelle) : le rectangle de la zone ne veut plus rien dire (R9).
/// L'ordre des deux listes ne compte pas.
pub fn ecrans_changes(avant: &[EcranInfo], apres: &[EcranInfo]) -> bool {
    let trier = |liste: &[EcranInfo]| {
        let mut l = liste.to_vec();
        crate::systeme::gel::trier_ecrans(&mut l);
        l
    };
    let (avant, apres) = (trier(avant), trier(apres));
    avant.len() != apres.len()
        || avant
            .iter()
            .zip(&apres)
            .any(|(a, b)| a.rect != b.rect || a.echelle != b.echelle)
}

/// Qui fait défiler la page (§4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conduite {
    /// `DEPART_AUTO` après la première image.
    Attente,
    /// L'app défile.
    Auto,
    /// Un cran de l'utilisateur est arrivé : définitif pour cette capture.
    Main,
}

/// Comment une défilante finit (D14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fin {
    Bouton,
    Entree,
    EnBas,
    Limite,
}

impl Fin {
    /// Nom écrit au journal : `bouton`, `entree`, `en_bas`, `limite`.
    pub fn nom(self) -> &'static str {
        match self {
            Fin::Bouton => "bouton",
            Fin::Entree => "entree",
            Fin::EnBas => "en_bas",
            Fin::Limite => "limite",
        }
    }
}

/// La course d'une capture : conduite, « trop vite », pas, moyen choisi, fin rendue. Le
/// nombre de moyens n'est pas compté ici : le défileur le sait (`abandonner()` rend
/// `None` quand il n'y en a plus) et le fil appelle alors `sans_moyen`.
#[derive(Debug, Clone, PartialEq)]
pub struct Course {
    conduite: Conduite,
    perdue: bool,
    fin: Option<Fin>,
    debut: Instant,
    /// La dernière image qui a apporté des lignes (ou le passage en `Main`).
    derniere_nouvelle: Instant,
    pas: i32,
    /// Le pas a déjà été réduit de moitié (une fois au plus, D10).
    pas_reduit: bool,
    /// Le dernier pas demandé (négatif : une remontée).
    dernier_pas: i32,
    /// Le moyen en cours a fait avancer la page : gardé pour toute la capture (R3).
    moyen_choisi: bool,
    /// Images immobiles de suite après un pas (D8, D15).
    immobiles: u32,
    /// Images ambiguës de suite en `Auto`, sans que la position lue bouge.
    ambigues: u32,
    /// Images perdues en `Auto` depuis la dernière avance ou image immobile (chaque fois,
    /// l'app remonte d'un demi-pas).
    perdues_auto: u32,
    /// La dernière position lue, en pour cent.
    derniere_position: Option<f64>,
    /// Le bas semble atteint depuis ce moment (D15).
    en_bas_depuis: Option<Instant>,
}

impl Course {
    /// Une course qui commence à `debut` (la première image).
    pub fn nouvelle(debut: Instant) -> Course {
        Course {
            conduite: Conduite::Attente,
            perdue: false,
            fin: None,
            debut,
            derniere_nouvelle: debut,
            pas: PAS_MIN,
            pas_reduit: false,
            dernier_pas: 0,
            moyen_choisi: false,
            immobiles: 0,
            ambigues: 0,
            perdues_auto: 0,
            derniere_position: None,
            en_bas_depuis: None,
        }
    }

    pub fn conduite(&self) -> Conduite {
        self.conduite
    }

    /// Vrai pendant que « Trop vite » est montré.
    pub fn perdue(&self) -> bool {
        self.perdue
    }

    /// La fin rendue, s'il y en a une (une seule par capture).
    pub fn fin(&self) -> Option<Fin> {
        self.fin
    }

    /// Le pas en cours, en pixels réels.
    pub fn pas(&self) -> i32 {
        self.pas
    }

    /// Règle le pas d'après la hauteur utile de la zone (D6) ; un pas déjà réduit le reste.
    pub fn regler_pas(&mut self, hauteur_utile: u32) {
        let pas = pas_de(hauteur_utile);
        self.pas = if self.pas_reduit {
            (pas / 2).max(PAS_MIN)
        } else {
            pas
        };
    }

    /// Plus aucun moyen de défilement (aucun défileur, `abandonner()` qui rend `None`, ou
    /// un pas retenu sans moyen) : la main passe à la molette de l'utilisateur, sans rien
    /// montrer (D8), et les images continuent d'être prises.
    pub fn sans_moyen(&mut self, maintenant: Instant) -> Vec<Geste> {
        if self.fin.is_some() || self.conduite == Conduite::Main {
            return Vec::new();
        }
        self.passer_main(maintenant);
        vec![Geste::Prendre(PRISE_MAIN)]
    }

    /// Le défileur est passé à un autre moyen de lui-même : ses deux premières commandes
    /// décideront de nouveau (R3).
    pub fn moyen_change(&mut self) {
        self.moyen_choisi = false;
        self.immobiles = 0;
        self.ambigues = 0;
    }

    /// Finit la course par `fin` si elle ne l'est pas déjà (la première fin gagne). Vrai si
    /// c'est cette fin qui compte.
    pub fn finir(&mut self, fin: Fin) -> bool {
        if self.fin.is_some() {
            return false;
        }
        self.fin = Some(fin);
        true
    }

    fn passer_main(&mut self, maintenant: Instant) {
        self.conduite = Conduite::Main;
        self.derniere_nouvelle = maintenant;
        self.en_bas_depuis = None;
    }

    fn descendre(&mut self, pas: i32) -> Geste {
        self.dernier_pas = pas;
        Geste::Descendre(pas)
    }
}

/// Ce que le fil `defilante` voit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Vu {
    /// Une image mesurée, et la position lue s'il y en a une.
    Image(Mesure, Option<Position>),
    /// Un cran de l'utilisateur ; `dans_zone` : le pointeur était dans la zone.
    Molette { dans_zone: bool },
    /// « Terminer ».
    Terminer,
    /// Entrée globale.
    Entree,
    /// Le temps passe.
    Temps(Instant),
}

/// Ce que le fil `defilante` doit faire.
#[derive(Debug, Clone, PartialEq)]
pub enum Geste {
    /// Un pas de défilement, en pixels réels (négatif : remonter). Le fil prend ensuite des
    /// images jusqu'à la page posée (D6) et rend la dernière, mesurée (`Vu::Image`).
    Descendre(i32),
    /// Prendre une image au plus tard dans ce délai, puis la rendre mesurée (`Vu::Image`).
    Prendre(Duration),
    /// Montrer ce message en haut de l'écran, ou le cacher (`None`).
    Message(Option<&'static str>),
    /// Envoyer l'aperçu en direct.
    Apercu,
    /// Abandonner le moyen de défilement pour le suivant (D8) ; sans suivant, le fil
    /// appelle `Course::sans_moyen`.
    Repli,
    /// Finir la capture.
    Finir(Fin),
}

/// Vrai si la position lue dit « en bas » (S5).
fn en_bas_lu(position: Option<Position>) -> bool {
    position.is_some_and(|p| p.pour_cent.is_finite() && p.pour_cent >= EN_BAS_POUR_CENT)
}

fn finir(course: &mut Course, fin: Fin) -> Vec<Geste> {
    if course.finir(fin) {
        vec![Geste::Finir(fin)]
    } else {
        Vec::new()
    }
}

/// La suite de la course après `vu` (pure, §4.2, D6 à D15, D20). Une course finie ne
/// rend plus rien : une seule fin par capture.
pub fn suite(course: &mut Course, vu: Vu, maintenant: Instant) -> Vec<Geste> {
    if course.fin.is_some() {
        return Vec::new();
    }
    if maintenant.saturating_duration_since(course.debut) >= DUREE_MAX {
        return finir(course, Fin::Limite);
    }
    if course.conduite == Conduite::Main
        && maintenant.saturating_duration_since(course.derniere_nouvelle) >= MAIN_SANS_IMAGE_MAX
    {
        return finir(course, Fin::Limite);
    }
    match vu {
        Vu::Terminer => finir(course, Fin::Bouton),
        Vu::Entree => finir(course, Fin::Entree),
        Vu::Molette { dans_zone: false } => Vec::new(),
        Vu::Molette { dans_zone: true } => {
            if course.conduite != Conduite::Main {
                // Définitif : aucun pas n'est plus demandé ; un pas en route finit (D9).
                course.passer_main(maintenant);
            }
            // Chaque cran : une image au plus tard dans `PRISE_MAIN`.
            vec![Geste::Prendre(PRISE_MAIN)]
        }
        Vu::Temps(_) => match course.conduite {
            Conduite::Attente
                if maintenant.saturating_duration_since(course.debut) >= DEPART_AUTO =>
            {
                course.conduite = Conduite::Auto;
                vec![course.descendre(course.pas)]
            }
            _ => Vec::new(),
        },
        Vu::Image(mesure, position) => match course.conduite {
            // Aucune image n'est demandée pendant l'attente : rien à en faire.
            Conduite::Attente => Vec::new(),
            Conduite::Auto => suite_auto(course, mesure, position, maintenant),
            Conduite::Main => suite_main(course, mesure, position, maintenant),
        },
    }
}

/// Une image après un pas automatique.
fn suite_auto(
    course: &mut Course,
    mesure: Mesure,
    position: Option<Position>,
    maintenant: Instant,
) -> Vec<Geste> {
    let en_bas = en_bas_lu(position);
    let lue = position.map(|p| p.pour_cent).filter(|p| p.is_finite());
    let bouge =
        matches!((lue, course.derniere_position), (Some(a), Some(b)) if (a - b).abs() > 1e-6);
    if lue.is_some() {
        course.derniere_position = lue;
    }
    if let Some(depuis) = course.en_bas_depuis {
        if matches!(mesure, Mesure::Immobile | Mesure::Ambigue) && !bouge {
            let ecoule = maintenant.saturating_duration_since(depuis);
            if ecoule >= FIN_EN_BAS {
                return finir(course, Fin::EnBas);
            }
            return vec![Geste::Prendre(FIN_EN_BAS - ecoule)];
        }
        // La page a bougé pendant l'arrêt (image paresseuse, rechargement) : pas en bas.
        course.en_bas_depuis = None;
    }
    let descente = course.dernier_pas > 0;
    match mesure {
        Mesure::Avance(_) => {
            course.immobiles = 0;
            course.ambigues = 0;
            course.perdues_auto = 0;
            course.moyen_choisi = true;
            course.derniere_nouvelle = maintenant;
            vec![Geste::Apercu, course.descendre(course.pas)]
        }
        // Un pas vers le bas qui fait remonter la page ne choisit pas le moyen : sans
        // effet utile tant que le moyen n'est pas choisi.
        Mesure::Recul(_) if descente && !course.moyen_choisi => {
            course.ambigues = 0;
            immobile_auto(course, false, maintenant)
        }
        Mesure::Recul(_) => {
            // Après une remontée, ou une page qui recharge.
            course.immobiles = 0;
            course.ambigues = 0;
            vec![course.descendre(course.pas)]
        }
        Mesure::Immobile => {
            course.ambigues = 0;
            course.perdues_auto = 0;
            immobile_auto(course, en_bas, maintenant)
        }
        Mesure::Ambigue => {
            if en_bas {
                course.ambigues = 0;
                return immobile_auto(course, true, maintenant);
            }
            if bouge {
                // La position avance : la page défile, la vue est seulement unie.
                course.ambigues = 0;
                course.immobiles = 0;
                course.moyen_choisi = true;
                return vec![course.descendre(course.pas)];
            }
            course.ambigues += 1;
            if course.ambigues >= AMBIGUES_MAX {
                course.ambigues = 0;
                return immobile_auto(course, false, maintenant);
            }
            vec![course.descendre(course.pas)]
        }
        Mesure::Perdue => {
            course.immobiles = 0;
            course.ambigues = 0;
            course.moyen_choisi = true;
            course.perdues_auto += 1;
            if course.perdues_auto > PERDUES_AUTO_MAX {
                // La page saute trop loin à chaque pas : la molette de l'utilisateur.
                course.passer_main(maintenant);
                return vec![Geste::Prendre(PRISE_MAIN)];
            }
            // Remonter d'un demi-pas, et réduire le pas de moitié, une fois au plus (D10).
            let demi = (course.pas / 2).max(1);
            if !course.pas_reduit {
                course.pas_reduit = true;
                course.pas = (course.pas / 2).max(PAS_MIN);
            }
            vec![course.descendre(-demi)]
        }
    }
}

/// Une image immobile après un pas (ou comptée comme telle).
fn immobile_auto(course: &mut Course, en_bas: bool, maintenant: Instant) -> Vec<Geste> {
    if en_bas {
        course.immobiles = 0;
        course.en_bas_depuis = Some(maintenant);
        return vec![Geste::Prendre(FIN_EN_BAS)];
    }
    course.immobiles += 1;
    if course.immobiles < 2 {
        return vec![course.descendre(course.pas)];
    }
    course.immobiles = 0;
    if course.moyen_choisi {
        // Deux images immobiles après deux pas d'un moyen qui marche : le bas (D15).
        course.en_bas_depuis = Some(maintenant);
        return vec![Geste::Prendre(FIN_EN_BAS)];
    }
    // Les deux premières commandes de ce moyen sans effet : le suivant (R3, D8). Le fil
    // passe en `Main` s'il n'y en a plus (`sans_moyen`).
    vec![Geste::Repli, course.descendre(course.pas)]
}

/// Une image prise pendant que la molette de l'utilisateur conduit.
fn suite_main(
    course: &mut Course,
    mesure: Mesure,
    position: Option<Position>,
    maintenant: Instant,
) -> Vec<Geste> {
    let mut gestes = Vec::new();
    if course.perdue {
        match mesure {
            Mesure::Avance(_) | Mesure::Immobile | Mesure::Recul(_) => {
                // La dernière image gardée est retrouvée : la capture reprend là.
                course.perdue = false;
                gestes.push(Geste::Message(None));
            }
            Mesure::Perdue | Mesure::Ambigue => return vec![Geste::Prendre(PRISE_MAIN)],
        }
    }
    match mesure {
        Mesure::Avance(_) => {
            course.derniere_nouvelle = maintenant;
            course.en_bas_depuis = None;
            gestes.push(Geste::Apercu);
        }
        Mesure::Perdue => {
            course.perdue = true;
            course.en_bas_depuis = None;
            gestes.push(Geste::Message(Some(TEXTE_TROP_VITE)));
        }
        // Sans position lue, aucune fin seule en `Main` (D15).
        Mesure::Immobile if en_bas_lu(position) => {
            let depuis = *course.en_bas_depuis.get_or_insert(maintenant);
            if maintenant.saturating_duration_since(depuis) >= FIN_EN_BAS {
                return finir(course, Fin::EnBas);
            }
        }
        Mesure::Immobile | Mesure::Recul(_) | Mesure::Ambigue => course.en_bas_depuis = None,
    }
    gestes.push(Geste::Prendre(PRISE_MAIN));
    gestes
}

/// Le décalage attendu d'une image mesurée contre la dernière image gardée (D10, décision
/// du responsable) : le décalage observé, d'après la position lue à l'image gardée et
/// maintenant, convertie avec la hauteur de la vue (`hauteur_vue`, pixels) ; une position
/// inchangée donne 0. Sans positions lisibles, les pixels commandés depuis l'image gardée
/// (`commande`), seulement comme indice (`None` si rien n'a été commandé).
pub fn attendu(
    gardee: Option<Position>,
    maintenant: Option<Position>,
    hauteur_vue: u32,
    commande: i32,
) -> Option<i32> {
    decalage_attendu(gardee, maintenant, hauteur_vue).or((commande != 0).then_some(commande))
}

/// Ce qu'un pas de `pas` pixels fait au plus, d'après l'avance par cran du moyen
/// (`Defileur::avance_par_cran`, B) : un cran au moins, puis des crans entiers ; `partiel`
/// (message posté) : un pas plus petit qu'un cran part en cran partiel. Sans avance connue,
/// le pas lui-même. Un indice pour l'attendu (D10), jamais une mesure.
pub fn avance_commandee(pas: i32, cran: Option<u32>, partiel: bool) -> i32 {
    let Some(cran) = cran
        .filter(|&c| c > 0)
        .map(|c| c.min(i32::MAX as u32) as i32)
    else {
        return pas;
    };
    let crans = pas.unsigned_abs() as i32 / cran;
    let avance = if crans > 0 {
        crans.saturating_mul(cran)
    } else if partiel {
        pas.unsigned_abs() as i32
    } else {
        cran
    };
    avance * pas.signum()
}

/// Vrai si un cran du moyen en cours fait plus que la zone ne peut recouvrir : chaque pas
/// perdrait des lignes, le moyen est laissé (B). `Message` envoie des crans partiels : il
/// n'est jamais laissé pour cela.
pub fn cran_trop_grand(moyen: Option<Moyen>, cran: Option<u32>, hauteur_utile: u32) -> bool {
    let (Some(moyen), Some(cran)) = (moyen, cran) else {
        return false;
    };
    if moyen == Moyen::Message || hauteur_utile == 0 {
        return false;
    }
    let recouvrement = RECOUVREMENT_MIN.min(hauteur_utile / 4);
    cran > hauteur_utile - recouvrement
}

/// L'avance à rendre au défileur après un pas vers le bas parti (`Defileur::constater`,
/// B), mesurée contre l'image d'avant ce pas : `Avance(n)` donne `n`, une image immobile
/// 0 (après un cran partiel, cela coupe les crans partiels de ce moyen). Rien pour une
/// mesure qui ne dit pas l'avance (recul, perdue, ambiguë).
pub fn avance_constatee(mesure: Mesure) -> Option<u32> {
    match mesure {
        Mesure::Avance(n) => Some(n),
        Mesure::Immobile => Some(0),
        Mesure::Recul(_) | Mesure::Perdue | Mesure::Ambigue => None,
    }
}

/// Ce que le fil principal doit faire à une étape du lancement (R1, D4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Etape {
    Rien,
    /// Poser la fenêtre et envoyer `defilante:montrer`.
    Montrer,
    /// Abandonner sans image (« Impossible de lancer la capture défilante »).
    Echouer,
    /// Première pilule : montrer la fenêtre, cacher l'écran figé et la sélection dans le
    /// même tour, lancer le fil.
    Partir,
    /// Une pilule de plus : seulement la poser.
    Placer,
}

/// Le lancement d'une défilante, de la visée à la fin (pur, R1, D4) : chaque arrivée
/// (page prête, fenêtre créée, relâchement, pilule, filet, fin) rend ce qu'il faut faire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Lancement {
    /// `defilante_pret` reçu.
    pub prete: bool,
    /// La fenêtre de contrôle existe.
    pub controle: bool,
    /// La création a échoué pour de bon.
    pub ratee: bool,
    /// La zone est choisie (Défilement).
    pub lancee: bool,
    /// `defilante:montrer` envoyé.
    pub montrer_envoye: bool,
    /// La fenêtre est montrée (l'écran figé caché).
    pub montree: bool,
    /// Le filet ou un échec a pris la main.
    pub abandonnee: bool,
    /// La fin est décidée.
    pub finie: bool,
}

impl Lancement {
    fn envoyer(&mut self) -> Etape {
        if self.prete
            && self.controle
            && self.lancee
            && !self.montrer_envoye
            && !self.montree
            && !self.abandonnee
        {
            self.montrer_envoye = true;
            Etape::Montrer
        } else {
            Etape::Rien
        }
    }

    fn en_attente(&self) -> bool {
        self.lancee && !self.montree && !self.abandonnee
    }

    /// `defilante_pret` (une page rechargée avant d'être montrée reçoit de nouveau sa
    /// charge).
    pub fn page_prete(&mut self) -> Etape {
        self.prete = true;
        if self.en_attente() {
            self.montrer_envoye = false;
        }
        self.envoyer()
    }

    /// La fenêtre de contrôle est créée.
    pub fn controle_cree(&mut self) -> Etape {
        self.controle = true;
        self.envoyer()
    }

    /// La création a échoué pour de bon : abandon si la zone est déjà choisie, sinon au
    /// relâchement.
    pub fn creation_ratee(&mut self) -> Etape {
        self.ratee = true;
        if self.en_attente() {
            self.abandonnee = true;
            Etape::Echouer
        } else {
            Etape::Rien
        }
    }

    /// La zone est choisie.
    pub fn relache(&mut self) -> Etape {
        self.lancee = true;
        if self.ratee {
            self.abandonnee = true;
            return Etape::Echouer;
        }
        self.envoyer()
    }

    /// `defilante_zone`.
    pub fn pilule(&mut self) -> Etape {
        if !self.lancee || !self.controle || self.abandonnee || self.finie {
            Etape::Rien
        } else if self.montree {
            Etape::Placer
        } else {
            self.montree = true;
            Etape::Partir
        }
    }

    /// Le filet de `DELAI_PAGE` : vrai s'il abandonne (fenêtre pas encore montrée).
    pub fn filet(&mut self) -> bool {
        if self.en_attente() && !self.finie {
            self.abandonnee = true;
            true
        } else {
            false
        }
    }

    /// Un échec après la pilule (fenêtre non montrée) : vrai s'il compte.
    pub fn echouer(&mut self) -> bool {
        if self.lancee && !self.abandonnee && !self.finie {
            self.abandonnee = true;
            true
        } else {
            false
        }
    }

    /// La fin est décidée : vrai la première fois.
    pub fn finir(&mut self) -> bool {
        if self.lancee && !self.finie && !self.abandonnee {
            self.finie = true;
            true
        } else {
            false
        }
    }

    /// Terminer, Entrée et les charges vers la page passent.
    pub fn ouverte(&self) -> bool {
        self.lancee && !self.finie && !self.abandonnee
    }
}

/// Pourquoi une défilante est annulée (D18) : rien n'est gardé, aucun message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Annulation {
    Echap,
    Ecrans,
    Session,
    Arret,
}

impl Annulation {
    pub fn nom(self) -> &'static str {
        match self {
            Annulation::Echap => "echap",
            Annulation::Ecrans => "ecrans",
            Annulation::Session => "session",
            Annulation::Arret => "arret",
        }
    }
}

/// Pourquoi une défilante ne peut pas se faire (D19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Impossible {
    Prise,
    Page,
    Fil,
    Memoire,
}

impl Impossible {
    pub fn nom(self) -> &'static str {
        match self {
            Impossible::Prise => "prise",
            Impossible::Page => "page",
            Impossible::Fil => "fil",
            Impossible::Memoire => "memoire",
        }
    }
}

/// Millisecondes entières d'une durée, pour le journal.
fn ms(d: Duration) -> u128 {
    d.as_millis()
}

/// `defilante pages <n> en <ms> ms` (D2).
pub fn ligne_pages(n: usize, duree: Duration) -> String {
    format!("defilante pages {n} en {} ms", ms(duree))
}

/// `defilante zone <x>,<y> <l>x<h> <trace|page> agrandie <oui|non>` (D3).
pub fn ligne_zone(zone: Rect, page: bool, agrandie: bool) -> String {
    format!(
        "defilante zone {},{} {}x{} {} agrandie {}",
        zone.x,
        zone.y,
        zone.largeur,
        zone.hauteur,
        if page { "page" } else { "trace" },
        if agrandie { "oui" } else { "non" }
    )
}

/// `defilante page prete en <ms> ms` (R1 : depuis la demande de création).
pub fn ligne_page_prete(duree: Duration) -> String {
    format!("defilante page prete en {} ms", ms(duree))
}

/// `defilante premiere image <l>x<h> en <ms> ms (relache <ms>)`.
pub fn ligne_premiere_image(
    largeur: u32,
    hauteur: u32,
    prise: Duration,
    depuis_relache: Duration,
) -> String {
    format!(
        "defilante premiere image {largeur}x{hauteur} en {} ms (relache {})",
        ms(prise),
        ms(depuis_relache)
    )
}

/// `defilante moyen <automation|molette|message|aucun>`.
pub fn ligne_moyen(moyen: Option<Moyen>) -> String {
    format!("defilante moyen {}", moyen.map_or("aucun", Moyen::nom))
}

/// `defilante repli: <moyen> (<moyen> sans effet)`, ou `defilante repli: molette` quand il
/// ne reste que la molette de l'utilisateur (D8).
pub fn ligne_repli(ancien: Option<Moyen>, nouveau: Option<Moyen>) -> String {
    match (nouveau, ancien) {
        (Some(n), Some(a)) => format!("defilante repli: {} ({} sans effet)", n.nom(), a.nom()),
        (Some(n), None) => format!("defilante repli: {}", n.nom()),
        (None, _) => "defilante repli: molette".into(),
    }
}

/// La main passe à la molette de l'utilisateur (D9).
pub const LIGNE_MAIN: &str = "defilante main (molette)";
/// « Trop vite » montré, puis caché (D10).
pub const LIGNE_TROP_VITE: &str = "defilante trop vite";
pub const LIGNE_REPRIS: &str = "defilante repris";

/// `defilante fixes haut <n> bas <n>` (D11).
pub fn ligne_fixes(fixes: Fixes) -> String {
    format!("defilante fixes haut {} bas {}", fixes.haut, fixes.bas)
}

/// `defilante fin <bouton|entree|en_bas|limite> images <n> gardees <k> perdues <p>` (§8).
pub fn ligne_fin(fin: Fin, n: u32, gardees: u32, perdues: u32) -> String {
    format!(
        "defilante fin {} images {n} gardees {gardees} perdues {perdues}",
        fin.nom()
    )
}

/// `defilante assemblee <l>x<h> en <ms> ms`.
pub fn ligne_assemblee(largeur: u32, hauteur: u32, duree: Duration) -> String {
    format!(
        "defilante assemblee {largeur}x{hauteur} en {} ms",
        ms(duree)
    )
}

/// `defilante annulee <echap|ecrans|session|arret>` (D18).
pub fn ligne_annulee(cause: Annulation) -> String {
    format!("defilante annulee {}", cause.nom())
}

/// `defilante impossible: <prise|page|fil|memoire>` (D19).
pub fn ligne_impossible(cause: Impossible) -> String {
    format!("defilante impossible: {}", cause.nom())
}

/// `chrono defilante relache-premiere <ms> pas-median <ms> mesure-mediane <ms>
/// fin-assemblee <ms>`.
pub fn ligne_chrono(
    relache_premiere: Duration,
    pas_median: Duration,
    mesure_mediane: Duration,
    fin_assemblee: Duration,
) -> String {
    format!(
        "chrono defilante relache-premiere {} pas-median {} mesure-mediane {} fin-assemblee {}",
        ms(relache_premiere),
        ms(pas_median),
        ms(mesure_mediane),
        ms(fin_assemblee)
    )
}

/// La médiane de durées (zéro sans durée) ; la plus basse des deux du milieu.
pub fn mediane(durees: &[Duration]) -> Duration {
    let mut triees = durees.to_vec();
    triees.sort_unstable();
    match triees.len() {
        0 => Duration::ZERO,
        n => triees[(n - 1) / 2],
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

    fn ecran(rect: Rect, utile: Rect, echelle: f64) -> EcranInfo {
        EcranInfo {
            rect,
            zone_utile: utile,
            echelle,
            principal: rect.x == 0 && rect.y == 0,
        }
    }

    /// 1920 × 1080, barre des tâches en bas (48 px logiques).
    fn ecran_simple(echelle: f64) -> EcranInfo {
        let barre = (48.0 * echelle).round() as i32;
        ecran(r(0, 0, 1920, 1080), r(0, 0, 1920, 1080 - barre), echelle)
    }

    fn pos(pour_cent: f64) -> Option<Position> {
        Some(Position {
            pour_cent,
            vue: 20.0,
        })
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    // ----- agrandir -----

    #[test]
    fn petite_zone_agrandie_autour_de_son_centre() {
        let e = ecran_simple(1.0);
        let (z, oui) = agrandir(r(500, 400, 80, 60), &e);
        assert!(oui);
        assert_eq!(z, r(480, 370, 120, 120));
        // Grande zone : inchangée.
        let (z, oui) = agrandir(r(10, 20, 300, 200), &e);
        assert!(!oui);
        assert_eq!(z, r(10, 20, 300, 200));
    }

    #[test]
    fn agrandie_a_l_echelle_de_l_ecran() {
        for (echelle, cote) in [(1.0, 120), (1.25, 150), (1.5, 180), (1.75, 210), (2.0, 240)] {
            let (z, oui) = agrandir(r(800, 500, 40, 300), &ecran_simple(echelle));
            assert!(oui);
            assert_eq!((z.largeur, z.hauteur), (cote, 300), "{echelle}");
            // Centre gardé (à un pixel près).
            assert!(((z.x + z.largeur / 2) - 820).abs() <= 1, "{echelle}");
        }
    }

    #[test]
    fn agrandie_sans_sortir_de_son_ecran() {
        // Collée au coin haut gauche d'un écran aux coordonnées négatives.
        let e = ecran(r(-2560, -300, 2560, 1440), r(-2560, -300, 2560, 1392), 1.5);
        let (z, oui) = agrandir(r(-2560, -300, 30, 20), &e);
        assert!(oui);
        assert_eq!(z, r(-2560, -300, 180, 180));
        // Collée au coin bas droit.
        let (z, _) = agrandir(r(-10, 1130, 10, 10), &e);
        assert_eq!(z, r(-180, 960, 180, 180));
        assert!(e.rect.englobe(z));
    }

    // ----- pages proposées -----

    #[test]
    fn pages_coupees_a_leur_ecran_et_triees() {
        let ecrans = [
            ecran(r(0, 0, 1920, 1080), r(0, 0, 1920, 1032), 1.0),
            ecran(r(1920, 0, 1920, 1080), r(1920, 0, 1920, 1032), 2.0),
        ];
        let p = |rect, fenetre| PageTrouvee { rect, fenetre };
        let trouvees = [
            // Surtout sur le premier écran : coupée à lui.
            p(r(1000, 100, 1200, 500), 11),
            // Trop petite sur l'écran à 200 % (200 < 240).
            p(r(2000, 100, 200, 600), 12),
            // Hors de tout écran.
            p(r(5000, 0, 400, 400), 13),
            // Sur le second écran, assez grande.
            p(r(2000, 100, 300, 300), 14),
        ];
        let pages = pages_proposees(&trouvees, &ecrans);
        assert_eq!(
            pages,
            vec![(r(1000, 100, 920, 500), 11), (r(2000, 100, 300, 300), 14)]
        );
    }

    #[test]
    fn au_plus_trente_deux_pages() {
        let ecrans = [ecran_simple(1.0)];
        let trouvees: Vec<_> = (0..50)
            .map(|i| PageTrouvee {
                rect: r(i, i, 500, 500),
                fenetre: i as isize,
            })
            .collect();
        let pages = pages_proposees(&trouvees, &ecrans);
        assert_eq!(pages.len(), PAGES_MAX);
        assert_eq!(pages[0].1, 0);
        assert_eq!(pages[31].1, 31);
    }

    #[test]
    fn ecran_de_la_plus_grande_surface() {
        let ecrans = [
            ecran(r(0, 0, 100, 100), r(0, 0, 100, 100), 1.0),
            ecran(r(100, 0, 100, 100), r(100, 0, 100, 100), 1.0),
        ];
        assert_eq!(ecran_de(r(80, 0, 40, 10), &ecrans).unwrap().rect.x, 0);
        assert_eq!(ecran_de(r(90, 0, 40, 10), &ecrans).unwrap().rect.x, 100);
        assert!(ecran_de(r(300, 0, 10, 10), &ecrans).is_none());
    }

    // ----- placer -----

    fn verifier_placement(zone: Rect, e: &EcranInfo) -> Placement {
        let p = placer(zone, e);
        let s = e.echelle;
        let marge = (8.0 * s).round() as i32;
        let utile = e.zone_utile;
        let dedans = Rect {
            x: utile.x + marge,
            y: utile.y + marge,
            largeur: utile.largeur - 2 * marge,
            hauteur: utile.hauteur - 2 * marge,
        };
        let pilule = Rect {
            x: p.pilule.0,
            y: p.pilule.1,
            largeur: (PILULE_LARGEUR_LOGIQUE * s).round() as i32,
            hauteur: (PILULE_HAUTEUR_LOGIQUE * s).round() as i32,
        };
        assert_eq!(p.apercu.largeur, (84.0 * s).round() as i32, "{zone:?} {s}");
        assert!(
            dedans.englobe(p.apercu),
            "apercu {:?} {zone:?} {s}",
            p.apercu
        );
        assert!(dedans.englobe(pilule), "pilule {pilule:?} {zone:?} {s}");
        assert!(
            p.apercu.intersection(pilule).is_none(),
            "pilule sur l'apercu {zone:?} {s}"
        );
        assert!(utile.contient(p.message), "{zone:?} {s}");
        p
    }

    #[test]
    fn placement_ordinaire_a_100_pour_cent() {
        let e = ecran_simple(1.0);
        let p = verifier_placement(r(400, 200, 600, 450), &e);
        // Aperçu à 12 px à droite, haut de la zone, jusqu'à 8 px du bas de la zone utile.
        assert_eq!(p.apercu, r(1012, 200, 84, 1032 - 8 - 200));
        // Pilule centrée, 10 px sous la zone.
        assert_eq!(p.pilule, (400 + 300 - 55, 660));
        // Message centré, 14 px sous le haut.
        assert_eq!(p.message, (960, 14));
    }

    #[test]
    fn placement_de_100_a_200_pour_cent() {
        for s in [1.0_f64, 1.25, 1.5, 1.75, 2.0] {
            let e = ecran_simple(s);
            let p = verifier_placement(r(400, 200, 600, 450), &e);
            assert_eq!(p.apercu.x, 1000 + (12.0 * s).round() as i32, "{s}");
            assert_eq!(p.pilule.1, 650 + (10.0 * s).round() as i32, "{s}");
            assert_eq!(p.message.1, (14.0 * s).round() as i32, "{s}");
        }
    }

    #[test]
    fn sans_place_a_droite_a_gauche_puis_dedans() {
        let e = ecran_simple(1.0);
        // Collée au bord droit : aperçu à gauche.
        let p = verifier_placement(r(1300, 200, 620, 400), &e);
        assert_eq!(p.apercu.x, 1300 - 12 - 84);
        // Tout l'écran : aperçu dedans, à droite.
        let p = verifier_placement(r(0, 0, 1920, 1032), &e);
        assert_eq!(p.apercu.x, 1920 - 12 - 84);
        assert_eq!(p.apercu.y, 8);
    }

    #[test]
    fn pilule_au_dessus_puis_dedans() {
        let e = ecran_simple(1.0);
        // Zone collée au bas de la zone utile : pilule au-dessus.
        let p = verifier_placement(r(400, 600, 600, 432), &e);
        assert_eq!(p.pilule.1, 600 - 10 - 42);
        // Zone de toute la hauteur : pilule dedans, en bas.
        let p = verifier_placement(r(400, 0, 600, 1032), &e);
        assert_eq!(p.pilule.1, 1032 - 10 - 42);
    }

    #[test]
    fn zone_collee_aux_bords_et_ecrans_negatifs() {
        for s in [1.0_f64, 1.25, 1.5, 2.0] {
            let barre = (48.0 * s).round() as i32;
            let e = ecran(
                r(-2560, -1440, 2560, 1440),
                r(-2560, -1440, 2560, 1440 - barre),
                s,
            );
            for zone in [
                r(-2560, -1440, 2560, 1440 - barre),
                r(-2560, -1440, 300, 300),
                r(-300, -1440 + 200, 300, 400),
                r(-2560, -400 - barre, 600, 400),
                r(-300, -300 - barre, 300, 300),
            ] {
                verifier_placement(zone, &e);
            }
        }
    }

    #[test]
    fn barre_des_taches_a_gauche() {
        for s in [1.0_f64, 1.5, 2.0] {
            let barre = (48.0 * s).round() as i32;
            let e = ecran(r(0, 0, 1920, 1080), r(barre, 0, 1920 - barre, 1080), s);
            // Zone collée à la barre : l'aperçu ne va pas dessous.
            let p = verifier_placement(r(barre, 100, 1920 - barre - 50, 600), &e);
            assert!(p.apercu.x >= barre + (8.0 * s).round() as i32, "{s}");
            assert_eq!(p.message.0, barre + (1920 - barre) / 2);
        }
    }

    #[test]
    fn pilule_jamais_sur_l_apercu() {
        let e = ecran_simple(1.0);
        // Zone étroite au bord droit : aperçu dedans, pilule poussée à côté.
        for x in [1700, 1780, 1800] {
            verifier_placement(r(x, 100, 1920 - x, 900), &e);
        }
    }

    #[test]
    fn pilule_de_la_page_en_pixels_reels() {
        let e = ecran(r(-1920, 0, 1920, 1080), r(-1920, 0, 1920, 1032), 1.5);
        assert_eq!(
            pilule_reelle(&e, 100.0, 200.0, 90.5, 42.0),
            Some(r(-1920 + 150, 300, 136, 63))
        );
        assert_eq!(pilule_reelle(&e, f64::NAN, 0.0, 10.0, 10.0), None);
        assert_eq!(pilule_reelle(&e, 0.0, 0.0, 0.0, 10.0), None);
    }

    // ----- limite -----

    #[test]
    fn limite_de_l_editeur() {
        assert_eq!(
            limite(600),
            Limite {
                hauteur: 16_384,
                pixels: 25_000_000
            }
        );
        assert_eq!(limite(2560), limite(600));
        assert_eq!(hauteur_max(600), 16_384);
        assert_eq!(hauteur_max(2560), 9_765);
        assert_eq!(hauteur_max(1526), 16_382);
        assert_eq!(hauteur_max(0), 16_384);
        assert!(hauteur_max(3000) as u64 * 3000 <= 25_000_000);
    }

    #[test]
    fn pas_moitie_de_la_hauteur_utile() {
        assert_eq!(pas_de(450), 225);
        assert_eq!(pas_de(60), PAS_MIN);
        assert_eq!(hauteur_utile(450, Fixes { haut: 40, bas: 10 }), 400);
        assert_eq!(hauteur_utile(30, Fixes { haut: 40, bas: 10 }), 0);
    }

    #[test]
    fn decalage_attendu_d_apres_les_positions() {
        // Page cinq fois plus haute que la vue : 400 px utiles, parcours 1 600 px.
        let a = Some(Position {
            pour_cent: 10.0,
            vue: 20.0,
        });
        let b = Some(Position {
            pour_cent: 20.0,
            vue: 20.0,
        });
        assert_eq!(decalage_attendu(a, b, 400), Some(160));
        assert_eq!(decalage_attendu(b, a, 400), Some(-160));
        assert_eq!(decalage_attendu(None, b, 400), None);
        let illisible = Some(Position {
            pour_cent: -1.0,
            vue: 100.0,
        });
        assert_eq!(decalage_attendu(illisible, illisible, 400), None);
    }

    #[test]
    fn ecrans_changes_compare_rectangles_et_echelles() {
        let a = [ecran_simple(1.0)];
        assert!(!ecrans_changes(&a, &a));
        assert!(ecrans_changes(&a, &[ecran_simple(1.25)]));
        assert!(ecrans_changes(&a, &[]));
        let mut b = a;
        b[0].rect.largeur = 2560;
        assert!(ecrans_changes(&a, &b));
        // Le même bureau, listé dans un autre ordre : rien n'a changé.
        let gauche = ecran(r(-1920, 0, 1920, 1080), r(-1920, 0, 1920, 1032), 1.0);
        let deux = [ecran_simple(1.0), gauche];
        assert!(!ecrans_changes(&deux, &[gauche, ecran_simple(1.0)]));
    }

    // ----- suite -----

    /// Une course passée en `Auto` (premier pas demandé) à `t0 + 450 ms`.
    fn en_auto(t0: Instant) -> Course {
        let mut c = Course::nouvelle(t0);
        c.regler_pas(400);
        assert_eq!(
            suite(&mut c, Vu::Temps(t0 + DEPART_AUTO), t0 + DEPART_AUTO),
            vec![Geste::Descendre(200)]
        );
        c
    }

    #[test]
    fn attente_de_450_ms_avant_le_premier_pas() {
        let t0 = Instant::now();
        let mut c = Course::nouvelle(t0);
        c.regler_pas(450);
        assert_eq!(c.conduite(), Conduite::Attente);
        assert!(suite(&mut c, Vu::Temps(t0 + ms(449)), t0 + ms(449)).is_empty());
        assert_eq!(c.conduite(), Conduite::Attente);
        assert_eq!(
            suite(&mut c, Vu::Temps(t0 + ms(450)), t0 + ms(450)),
            vec![Geste::Descendre(225)]
        );
        assert_eq!(c.conduite(), Conduite::Auto);
        // Un seul départ.
        assert!(suite(&mut c, Vu::Temps(t0 + ms(500)), t0 + ms(500)).is_empty());
    }

    #[test]
    fn pas_apres_pas_tant_que_la_page_avance() {
        let t0 = Instant::now();
        let mut c = en_auto(t0);
        for i in 1..5 {
            let t = t0 + ms(500 + i * 50);
            assert_eq!(
                suite(&mut c, Vu::Image(Mesure::Avance(200), None), t),
                vec![Geste::Apercu, Geste::Descendre(200)]
            );
        }
        // Le pas suit les bandes fixes.
        c.regler_pas(300);
        assert_eq!(c.pas(), 150);
        c.regler_pas(10);
        assert_eq!(c.pas(), PAS_MIN);
    }

    #[test]
    fn repli_jusqu_a_ce_que_le_defileur_n_ait_plus_de_moyen() {
        let t0 = Instant::now();
        let mut c = en_auto(t0);
        let t = t0 + ms(600);
        let immobile = Vu::Image(Mesure::Immobile, None);
        for _ in 0..4 {
            // Première commande sans effet : la seconde.
            assert_eq!(suite(&mut c, immobile, t), vec![Geste::Descendre(200)]);
            // Seconde sans effet : le moyen suivant ; la course ne compte pas les moyens.
            assert_eq!(
                suite(&mut c, immobile, t),
                vec![Geste::Repli, Geste::Descendre(200)]
            );
            assert_eq!(c.conduite(), Conduite::Auto);
        }
        // Le défileur n'en a plus : la molette de l'utilisateur, sans message.
        assert_eq!(c.sans_moyen(t), vec![Geste::Prendre(PRISE_MAIN)]);
        assert_eq!(c.conduite(), Conduite::Main);
        assert!(!c.perdue());
        assert!(c.sans_moyen(t).is_empty());
        // Puis les images suivent la molette ; pas de fin seule sans position.
        for _ in 0..20 {
            assert_eq!(suite(&mut c, immobile, t), vec![Geste::Prendre(PRISE_MAIN)]);
        }
        assert_eq!(c.fin(), None);
    }

    #[test]
    fn moyen_change_par_le_defileur() {
        let t0 = Instant::now();
        let mut c = en_auto(t0);
        let t = t0 + ms(600);
        let immobile = Vu::Image(Mesure::Immobile, None);
        suite(&mut c, Vu::Image(Mesure::Avance(200), None), t);
        c.moyen_change();
        // Le nouveau moyen n'est pas choisi : deux images immobiles font un repli.
        assert_eq!(suite(&mut c, immobile, t), vec![Geste::Descendre(200)]);
        assert_eq!(
            suite(&mut c, immobile, t),
            vec![Geste::Repli, Geste::Descendre(200)]
        );
    }

    #[test]
    fn recul_sur_un_pas_vers_le_bas_ne_choisit_pas_le_moyen() {
        let t0 = Instant::now();
        let mut c = en_auto(t0);
        let t = t0 + ms(600);
        let recul = Vu::Image(Mesure::Recul(200), None);
        assert_eq!(suite(&mut c, recul, t), vec![Geste::Descendre(200)]);
        assert_eq!(
            suite(&mut c, recul, t),
            vec![Geste::Repli, Geste::Descendre(200)]
        );
        // Après une remontée voulue, un recul est attendu : on redescend.
        let mut c = en_auto(t0);
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Perdue, None), t),
            vec![Geste::Descendre(-100)]
        );
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Recul(20), None), t),
            vec![Geste::Descendre(100)]
        );
    }

    #[test]
    fn moyen_garde_des_qu_il_fait_avancer() {
        let t0 = Instant::now();
        let mut c = en_auto(t0);
        let t = t0 + ms(600);
        let immobile = Vu::Image(Mesure::Immobile, None);
        // Première commande sans effet, seconde qui avance : moyen choisi.
        assert_eq!(suite(&mut c, immobile, t), vec![Geste::Descendre(200)]);
        suite(&mut c, Vu::Image(Mesure::Avance(200), None), t);
        // Ensuite deux images immobiles disent « en bas », jamais un repli.
        assert_eq!(suite(&mut c, immobile, t), vec![Geste::Descendre(200)]);
        assert_eq!(suite(&mut c, immobile, t), vec![Geste::Prendre(FIN_EN_BAS)]);
        // 350 ms sans autre avance : la fin seule.
        assert_eq!(
            suite(&mut c, immobile, t + ms(100)),
            vec![Geste::Prendre(ms(250))]
        );
        assert_eq!(
            suite(&mut c, immobile, t + FIN_EN_BAS),
            vec![Geste::Finir(Fin::EnBas)]
        );
        assert_eq!(c.fin(), Some(Fin::EnBas));
    }

    #[test]
    fn en_bas_par_la_position_lue() {
        let t0 = Instant::now();
        let mut c = en_auto(t0);
        let t = t0 + ms(600);
        suite(&mut c, Vu::Image(Mesure::Avance(200), pos(80.0)), t);
        // Une image immobile et 99,9 % : l'arrêt commence tout de suite.
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Immobile, pos(99.9)), t),
            vec![Geste::Prendre(FIN_EN_BAS)]
        );
        // Une avance pendant l'arrêt (image paresseuse) : on repart.
        assert_eq!(
            suite(
                &mut c,
                Vu::Image(Mesure::Avance(50), pos(97.0)),
                t + ms(100)
            ),
            vec![Geste::Apercu, Geste::Descendre(200)]
        );
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Immobile, pos(100.0)), t + ms(200)),
            vec![Geste::Prendre(FIN_EN_BAS)]
        );
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Immobile, pos(100.0)), t + ms(550)),
            vec![Geste::Finir(Fin::EnBas)]
        );
    }

    #[test]
    fn reprise_par_la_molette_definitive() {
        let t0 = Instant::now();
        let mut c = en_auto(t0);
        let t = t0 + ms(600);
        // Hors de la zone : ne compte pas.
        assert!(suite(&mut c, Vu::Molette { dans_zone: false }, t).is_empty());
        assert_eq!(c.conduite(), Conduite::Auto);
        assert_eq!(
            suite(&mut c, Vu::Molette { dans_zone: true }, t),
            vec![Geste::Prendre(PRISE_MAIN)]
        );
        assert_eq!(c.conduite(), Conduite::Main);
        // Le pas en route finit : son image est ajoutée, aucun pas n'est plus demandé.
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Avance(200), None), t + ms(30)),
            vec![Geste::Apercu, Geste::Prendre(PRISE_MAIN)]
        );
        for vu in [
            Vu::Image(Mesure::Immobile, None),
            Vu::Image(Mesure::Recul(30), None),
            Vu::Image(Mesure::Ambigue, None),
            Vu::Temps(t + ms(10_000)),
        ] {
            let gestes = suite(&mut c, vu, t + ms(100));
            assert!(
                !gestes.iter().any(|g| matches!(g, Geste::Descendre(_))),
                "{vu:?}"
            );
        }
        // Chaque cran demande une image.
        assert_eq!(
            suite(&mut c, Vu::Molette { dans_zone: true }, t),
            vec![Geste::Prendre(PRISE_MAIN)]
        );
        assert_eq!(c.conduite(), Conduite::Main);
    }

    #[test]
    fn molette_pendant_l_attente() {
        let t0 = Instant::now();
        let mut c = Course::nouvelle(t0);
        assert_eq!(
            suite(&mut c, Vu::Molette { dans_zone: true }, t0 + ms(100)),
            vec![Geste::Prendre(PRISE_MAIN)]
        );
        // Plus de départ automatique.
        assert!(suite(&mut c, Vu::Temps(t0 + ms(900)), t0 + ms(900)).is_empty());
        assert_eq!(c.conduite(), Conduite::Main);
    }

    #[test]
    fn perdue_puis_reprise_en_main() {
        let t0 = Instant::now();
        let mut c = Course::nouvelle(t0);
        let t = t0 + ms(100);
        suite(&mut c, Vu::Molette { dans_zone: true }, t);
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Perdue, None), t),
            vec![
                Geste::Message(Some(TEXTE_TROP_VITE)),
                Geste::Prendre(PRISE_MAIN)
            ]
        );
        assert!(c.perdue());
        // Toujours perdue, ou ambiguë : rien de plus (le message reste).
        for m in [Mesure::Perdue, Mesure::Ambigue] {
            assert_eq!(
                suite(&mut c, Vu::Image(m, None), t),
                vec![Geste::Prendre(PRISE_MAIN)]
            );
        }
        // Retrouvée (en remontant) : message caché, la capture reprend là.
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Recul(10), None), t),
            vec![Geste::Message(None), Geste::Prendre(PRISE_MAIN)]
        );
        assert!(!c.perdue());
        // Perdue de nouveau puis retrouvée en avançant : message caché et lignes ajoutées.
        suite(&mut c, Vu::Image(Mesure::Perdue, None), t);
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Avance(20), None), t),
            vec![
                Geste::Message(None),
                Geste::Apercu,
                Geste::Prendre(PRISE_MAIN)
            ]
        );
    }

    #[test]
    fn auto_qui_remonte_et_reduit_son_pas_une_fois() {
        let t0 = Instant::now();
        let mut c = en_auto(t0);
        let t = t0 + ms(600);
        let perdue = Vu::Image(Mesure::Perdue, None);
        // Rien de montré ; remonter d'un demi-pas, pas réduit de moitié.
        assert_eq!(suite(&mut c, perdue, t), vec![Geste::Descendre(-100)]);
        assert!(!c.perdue());
        assert_eq!(c.pas(), 100);
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Avance(100), None), t),
            vec![Geste::Apercu, Geste::Descendre(100)]
        );
        // Une seconde fois : on remonte encore, le pas n'est plus réduit.
        assert_eq!(suite(&mut c, perdue, t), vec![Geste::Descendre(-50)]);
        assert_eq!(c.pas(), 100);
        c.regler_pas(400);
        assert_eq!(c.pas(), 100);
        // Toujours perdue : au-delà de `PERDUES_AUTO_MAX`, la molette, sans message.
        for _ in 1..PERDUES_AUTO_MAX {
            assert_eq!(suite(&mut c, perdue, t), vec![Geste::Descendre(-50)]);
        }
        assert_eq!(suite(&mut c, perdue, t), vec![Geste::Prendre(PRISE_MAIN)]);
        assert_eq!(c.conduite(), Conduite::Main);
        assert!(!c.perdue());
    }

    #[test]
    fn recul_puis_perdue_en_auto() {
        let t0 = Instant::now();
        let mut c = en_auto(t0);
        let t = t0 + ms(600);
        // Le moyen est choisi ; une perdue fait remonter d'un demi-pas.
        suite(&mut c, Vu::Image(Mesure::Avance(200), None), t);
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Perdue, None), t),
            vec![Geste::Descendre(-100)]
        );
        // Le recul après la remontée : on redescend du pas réduit.
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Recul(100), None), t),
            vec![Geste::Descendre(100)]
        );
        // Encore perdue : un demi-pas du pas réduit, sans le réduire une seconde fois.
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Perdue, None), t),
            vec![Geste::Descendre(-50)]
        );
        assert_eq!(c.pas(), 100);
        assert_eq!(c.conduite(), Conduite::Auto);
        assert!(!c.perdue());
    }

    #[test]
    fn avance_d_un_pas_d_apres_le_cran() {
        assert_eq!(avance_commandee(200, None, false), 200);
        assert_eq!(avance_commandee(200, Some(0), false), 200);
        // Des crans entiers, un au moins.
        assert_eq!(avance_commandee(200, Some(60), false), 180);
        assert_eq!(avance_commandee(40, Some(60), false), 60);
        // Message : un cran partiel pour un pas plus petit qu'un cran.
        assert_eq!(avance_commandee(40, Some(60), true), 40);
        assert_eq!(avance_commandee(200, Some(60), true), 180);
        // Vers le haut.
        assert_eq!(avance_commandee(-100, Some(60), false), -60);
        assert_eq!(avance_commandee(-20, Some(60), true), -20);
    }

    #[test]
    fn cran_trop_grand_pour_la_zone() {
        // 450 lignes utiles : au plus 450 - 48.
        assert!(!cran_trop_grand(Some(Moyen::Molette), Some(402), 450));
        assert!(cran_trop_grand(Some(Moyen::Molette), Some(403), 450));
        assert!(cran_trop_grand(Some(Moyen::Automation), Some(403), 450));
        assert!(!cran_trop_grand(Some(Moyen::Message), Some(1_000), 450));
        assert!(!cran_trop_grand(Some(Moyen::Molette), None, 450));
        assert!(!cran_trop_grand(None, Some(1_000), 450));
        // Petite zone : un quart de recouvrement.
        assert!(!cran_trop_grand(Some(Moyen::Molette), Some(75), 100));
        assert!(cran_trop_grand(Some(Moyen::Molette), Some(76), 100));
    }

    #[test]
    fn avance_rendue_au_defileur() {
        assert_eq!(avance_constatee(Mesure::Avance(120)), Some(120));
        assert_eq!(avance_constatee(Mesure::Immobile), Some(0));
        assert_eq!(avance_constatee(Mesure::Recul(3)), None);
        assert_eq!(avance_constatee(Mesure::Perdue), None);
        assert_eq!(avance_constatee(Mesure::Ambigue), None);
    }

    #[test]
    fn ambigue_l_image_suivante_decide() {
        let t0 = Instant::now();
        let mut c = en_auto(t0);
        let t = t0 + ms(600);
        let ambigue = Vu::Image(Mesure::Ambigue, None);
        // Rien d'ajouté (pas d'aperçu), le pas suivant part.
        assert_eq!(suite(&mut c, ambigue, t), vec![Geste::Descendre(200)]);
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Avance(400), None), t),
            vec![Geste::Apercu, Geste::Descendre(200)]
        );
        // Une page unie sans fin compte comme immobile au bout de `AMBIGUES_MAX`.
        for _ in 1..AMBIGUES_MAX {
            assert_eq!(suite(&mut c, ambigue, t), vec![Geste::Descendre(200)]);
        }
        assert_eq!(suite(&mut c, ambigue, t), vec![Geste::Descendre(200)]);
        for _ in 1..AMBIGUES_MAX {
            suite(&mut c, ambigue, t);
        }
        assert_eq!(suite(&mut c, ambigue, t), vec![Geste::Prendre(FIN_EN_BAS)]);
        // Ambiguë et en bas lu : l'arrêt aussi.
        let mut c = en_auto(t0);
        suite(&mut c, Vu::Image(Mesure::Avance(200), None), t);
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Ambigue, pos(100.0)), t),
            vec![Geste::Prendre(FIN_EN_BAS)]
        );
    }

    #[test]
    fn en_main_la_fin_seule_demande_la_position() {
        let t0 = Instant::now();
        let mut c = Course::nouvelle(t0);
        let t = t0 + ms(100);
        suite(&mut c, Vu::Molette { dans_zone: true }, t);
        let immobile_en_bas = Vu::Image(Mesure::Immobile, pos(100.0));
        assert_eq!(
            suite(&mut c, immobile_en_bas, t),
            vec![Geste::Prendre(PRISE_MAIN)]
        );
        // Un recul remet l'arrêt à zéro.
        suite(&mut c, Vu::Image(Mesure::Recul(5), pos(98.0)), t + ms(200));
        suite(&mut c, immobile_en_bas, t + ms(300));
        assert_eq!(
            suite(&mut c, immobile_en_bas, t + ms(600)),
            vec![Geste::Prendre(PRISE_MAIN)]
        );
        assert_eq!(
            suite(&mut c, immobile_en_bas, t + ms(650)),
            vec![Geste::Finir(Fin::EnBas)]
        );
    }

    #[test]
    fn fins_et_une_seule_fin() {
        let t0 = Instant::now();
        for (vu, fin) in [(Vu::Terminer, Fin::Bouton), (Vu::Entree, Fin::Entree)] {
            let mut c = en_auto(t0);
            assert_eq!(suite(&mut c, vu, t0 + ms(700)), vec![Geste::Finir(fin)]);
            assert_eq!(c.fin(), Some(fin));
            // Plus rien ensuite, ni une seconde fin.
            for autre in [
                Vu::Terminer,
                Vu::Entree,
                Vu::Molette { dans_zone: true },
                Vu::Image(Mesure::Avance(10), None),
                Vu::Temps(t0 + DUREE_MAX),
            ] {
                assert!(suite(&mut c, autre, t0 + DUREE_MAX).is_empty());
            }
            assert!(!c.finir(Fin::Limite));
            assert_eq!(c.fin(), Some(fin));
        }
        // La limite rendue par le fil (assemblage plein) compte comme fin.
        let mut c = en_auto(t0);
        assert!(c.finir(Fin::Limite));
        assert!(suite(&mut c, Vu::Terminer, t0 + ms(800)).is_empty());
        assert_eq!(c.fin(), Some(Fin::Limite));
    }

    #[test]
    fn echeances_globales() {
        let t0 = Instant::now();
        // Dix minutes : fin `limite`, quelle que soit la conduite.
        let mut c = en_auto(t0);
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Avance(10), None), t0 + DUREE_MAX),
            vec![Geste::Finir(Fin::Limite)]
        );
        // En `Main`, deux minutes sans image nouvelle.
        let mut c = Course::nouvelle(t0);
        suite(&mut c, Vu::Molette { dans_zone: true }, t0 + ms(10));
        suite(&mut c, Vu::Image(Mesure::Avance(10), None), t0 + ms(1000));
        assert_eq!(
            suite(
                &mut c,
                Vu::Temps(t0 + ms(1000) + MAIN_SANS_IMAGE_MAX - ms(1)),
                t0 + ms(1000) + MAIN_SANS_IMAGE_MAX - ms(1)
            ),
            Vec::<Geste>::new()
        );
        assert_eq!(
            suite(
                &mut c,
                Vu::Temps(t0 + ms(1000) + MAIN_SANS_IMAGE_MAX),
                t0 + ms(1000) + MAIN_SANS_IMAGE_MAX
            ),
            vec![Geste::Finir(Fin::Limite)]
        );
        // En `Auto`, aucune échéance de deux minutes.
        let mut c = en_auto(t0);
        assert!(suite(&mut c, Vu::Temps(t0 + ms(200_000)), t0 + ms(200_000)).is_empty());
    }

    #[test]
    fn sans_moyen_la_main_tout_de_suite_et_les_images_continuent() {
        let t0 = Instant::now();
        let mut c = Course::nouvelle(t0);
        assert_eq!(c.sans_moyen(t0), vec![Geste::Prendre(PRISE_MAIN)]);
        assert_eq!(c.conduite(), Conduite::Main);
        assert!(suite(&mut c, Vu::Temps(t0 + ms(500)), t0 + ms(500)).is_empty());
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Immobile, None), t0 + ms(550)),
            vec![Geste::Prendre(PRISE_MAIN)]
        );
        assert_eq!(
            suite(&mut c, Vu::Molette { dans_zone: true }, t0 + ms(560)),
            vec![Geste::Prendre(PRISE_MAIN)]
        );
    }

    #[test]
    fn perdue_ou_recul_pendant_l_arret_en_bas() {
        let t0 = Instant::now();
        let t = t0 + ms(600);
        for (m, attendu) in [
            (Mesure::Perdue, Geste::Descendre(-100)),
            (Mesure::Recul(30), Geste::Descendre(200)),
        ] {
            let mut c = en_auto(t0);
            suite(&mut c, Vu::Image(Mesure::Avance(200), None), t);
            assert_eq!(
                suite(&mut c, Vu::Image(Mesure::Immobile, pos(100.0)), t),
                vec![Geste::Prendre(FIN_EN_BAS)]
            );
            // La page a bougé : pas en bas, même après 350 ms.
            assert_eq!(
                suite(&mut c, Vu::Image(m, None), t + FIN_EN_BAS),
                vec![attendu],
                "{m:?}"
            );
            assert_eq!(c.fin(), None);
        }
    }

    #[test]
    fn ambigue_avec_une_position_qui_avance() {
        let t0 = Instant::now();
        let mut c = en_auto(t0);
        let t = t0 + ms(600);
        for i in 0..(3 * AMBIGUES_MAX) {
            assert_eq!(
                suite(&mut c, Vu::Image(Mesure::Ambigue, pos(i as f64)), t),
                vec![Geste::Descendre(200)],
                "{i}"
            );
        }
        assert_eq!(c.fin(), None);
    }

    #[test]
    fn dix_minutes_pendant_l_attente() {
        let t0 = Instant::now();
        let mut c = Course::nouvelle(t0);
        assert_eq!(
            suite(&mut c, Vu::Temps(t0 + DUREE_MAX), t0 + DUREE_MAX),
            vec![Geste::Finir(Fin::Limite)]
        );
    }

    #[test]
    fn attendu_observe_sinon_commande() {
        let p = |pour_cent| {
            Some(Position {
                pour_cent,
                vue: 20.0,
            })
        };
        // Observé : 10 % d'un parcours de 1 600 px.
        assert_eq!(attendu(p(10.0), p(20.0), 400, 225), Some(160));
        // Position inchangée : 0, jamais le pas commandé.
        assert_eq!(attendu(p(10.0), p(10.0), 400, 225), Some(0));
        // Sans position : les pixels commandés depuis l'image gardée, comme indice.
        assert_eq!(attendu(None, p(10.0), 400, 450), Some(450));
        assert_eq!(attendu(None, None, 400, 0), None);
    }

    // ----- lancement -----

    #[test]
    fn lancement_page_prete_avant_le_relachement() {
        let mut l = Lancement::default();
        assert_eq!(l.controle_cree(), Etape::Rien);
        assert_eq!(l.page_prete(), Etape::Rien);
        assert_eq!(l.relache(), Etape::Montrer);
        assert_eq!(l.pilule(), Etape::Partir);
        assert_eq!(l.pilule(), Etape::Placer);
        // Le filet arrive trop tard : rien.
        assert!(!l.filet());
        assert!(l.ouverte());
        assert!(l.finir());
        assert!(!l.finir());
        assert_eq!(l.pilule(), Etape::Rien);
        assert!(!l.ouverte());
    }

    #[test]
    fn lancement_page_prete_apres_le_relachement() {
        let mut l = Lancement::default();
        assert_eq!(l.relache(), Etape::Rien);
        // Page prête avant la fenêtre rangée (création qui pompe la boucle).
        assert_eq!(l.page_prete(), Etape::Rien);
        assert_eq!(l.controle_cree(), Etape::Montrer);
        // Page rechargée avant d'être montrée : la charge repart.
        assert_eq!(l.page_prete(), Etape::Montrer);
        assert_eq!(l.pilule(), Etape::Partir);
        assert_eq!(l.page_prete(), Etape::Rien);
    }

    #[test]
    fn lancement_filet_contre_la_pilule() {
        let mut l = Lancement::default();
        l.controle_cree();
        l.relache();
        assert!(l.filet());
        // Une pilule tardive ne montre plus rien, une seconde voie n'échoue pas deux fois.
        assert_eq!(l.pilule(), Etape::Rien);
        assert!(!l.filet());
        assert!(!l.echouer());
        assert!(!l.finir());
        assert!(!l.ouverte());
    }

    #[test]
    fn lancement_creation_ratee() {
        // Pendant la visée : l'échec attend le relâchement.
        let mut l = Lancement::default();
        assert_eq!(l.creation_ratee(), Etape::Rien);
        assert_eq!(l.relache(), Etape::Echouer);
        assert!(!l.filet());
        // Après le relâchement : tout de suite.
        let mut l = Lancement::default();
        l.relache();
        assert_eq!(l.creation_ratee(), Etape::Echouer);
        assert_eq!(l.pilule(), Etape::Rien);
        // Fenêtre non montrée après la pilule : un seul échec.
        let mut l = Lancement::default();
        l.controle_cree();
        l.page_prete();
        l.relache();
        assert_eq!(l.pilule(), Etape::Partir);
        assert!(l.echouer());
        assert!(!l.echouer());
        assert!(!l.filet());
    }

    // ----- journal -----

    #[test]
    fn lignes_du_journal() {
        assert_eq!(ligne_pages(3, ms(42)), "defilante pages 3 en 42 ms");
        assert_eq!(
            ligne_zone(r(-10, 20, 120, 130), false, true),
            "defilante zone -10,20 120x130 trace agrandie oui"
        );
        assert_eq!(
            ligne_zone(r(0, 0, 600, 450), true, false),
            "defilante zone 0,0 600x450 page agrandie non"
        );
        assert_eq!(ligne_page_prete(ms(660)), "defilante page prete en 660 ms");
        assert_eq!(
            ligne_premiere_image(600, 450, ms(32), ms(180)),
            "defilante premiere image 600x450 en 32 ms (relache 180)"
        );
        assert_eq!(ligne_moyen(Some(Moyen::Message)), "defilante moyen message");
        assert_eq!(ligne_moyen(None), "defilante moyen aucun");
        assert_eq!(
            ligne_repli(Some(Moyen::Message), Some(Moyen::Automation)),
            "defilante repli: automation (message sans effet)"
        );
        assert_eq!(
            ligne_repli(Some(Moyen::Molette), None),
            "defilante repli: molette"
        );
        assert_eq!(LIGNE_MAIN, "defilante main (molette)");
        assert_eq!(LIGNE_TROP_VITE, "defilante trop vite");
        assert_eq!(LIGNE_REPRIS, "defilante repris");
        assert_eq!(
            ligne_fixes(Fixes { haut: 40, bas: 0 }),
            "defilante fixes haut 40 bas 0"
        );
        assert_eq!(
            ligne_fin(Fin::EnBas, 30, 27, 1),
            "defilante fin en_bas images 30 gardees 27 perdues 1"
        );
        for (f, nom) in [
            (Fin::Bouton, "bouton"),
            (Fin::Entree, "entree"),
            (Fin::EnBas, "en_bas"),
            (Fin::Limite, "limite"),
        ] {
            assert_eq!(f.nom(), nom);
        }
        assert_eq!(
            ligne_assemblee(600, 6000, ms(120)),
            "defilante assemblee 600x6000 en 120 ms"
        );
        for (a, ligne) in [
            (Annulation::Echap, "defilante annulee echap"),
            (Annulation::Ecrans, "defilante annulee ecrans"),
            (Annulation::Session, "defilante annulee session"),
            (Annulation::Arret, "defilante annulee arret"),
        ] {
            assert_eq!(ligne_annulee(a), ligne);
        }
        for (i, ligne) in [
            (Impossible::Prise, "defilante impossible: prise"),
            (Impossible::Page, "defilante impossible: page"),
            (Impossible::Fil, "defilante impossible: fil"),
            (Impossible::Memoire, "defilante impossible: memoire"),
        ] {
            assert_eq!(ligne_impossible(i), ligne);
        }
        assert_eq!(
            ligne_chrono(ms(300), ms(80), ms(9), ms(110)),
            "chrono defilante relache-premiere 300 pas-median 80 mesure-mediane 9 fin-assemblee 110"
        );
    }

    #[test]
    fn mediane_des_durees() {
        assert_eq!(mediane(&[]), Duration::ZERO);
        assert_eq!(mediane(&[ms(5)]), ms(5));
        assert_eq!(mediane(&[ms(9), ms(1), ms(5)]), ms(5));
        assert_eq!(mediane(&[ms(9), ms(1), ms(5), ms(7)]), ms(5));
    }
}
