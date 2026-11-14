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
use crate::defilement::{Attendu, Fixes, Limite, Mesure};
use crate::systeme::defiler::{self, Moyen, PageTrouvee, Position};
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
/// Images `Vu::ImmobileLu` de suite au plus petit pas après lesquelles elles comptent comme
/// des images immobiles ordinaires : une vue au contenu fixe dont l'application dit qu'elle
/// défile ne tourne pas jusqu'à `DUREE_MAX` (N4).
pub const IMMOBILES_LUS_MAX: u32 = 3;

/// Texte de la pilule de la page de contrôle.
pub const LIBELLE_TERMINER: &str = "Terminer";

/// Message « trop vite » (D10), montré en haut de l'écran.
pub const TEXTE_TROP_VITE: &str = "Trop vite. Remontez un peu.";

/// Message montré quand l'app ne peut plus suivre une page qui a changé de hauteur
/// (relecture des correctifs, C1) : l'image s'arrête à la dernière image gardée, et la
/// capture attend la molette ou « Terminer ». Même place et même forme que « Trop vite ».
pub const TEXTE_PAGE_CHANGEE: &str = "La page a changé. L'image s'arrête ici.";

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
pub const PILULE_LARGEUR_LOGIQUE: f64 = crate::fenetres::defilante::TAILLE_PILULE_LOGIQUE.0;
pub const PILULE_HAUTEUR_LOGIQUE: f64 = crate::fenetres::defilante::TAILLE_PILULE_LOGIQUE.1;
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
    /// Un cran de l'utilisateur dans la zone a été vu : « Trop vite » ne parle que de sa
    /// molette, jamais d'un passage en `Main` décidé par l'app (contrat §3, D10, U5).
    molette_vue: bool,
    /// Un message est montré (« Trop vite », ou « La page a changé »), caché quand la
    /// dernière image gardée est retrouvée.
    message_montre: bool,
    /// Les positions de l'image gardée et de la dernière image d'un pas sont lues, mais le lu
    /// est impossible entre les deux (la page ou la vue ont changé de hauteur, `attendu`) :
    /// une image qui ne recolle pas ne se rattrapera pas, et ne conclut jamais au bas
    /// (relecture des correctifs, C1). Réglé par le fil à chaque image d'un pas.
    lu_perdu: bool,
    fin: Option<Fin>,
    debut: Instant,
    /// La dernière image qui a apporté des lignes (ou le passage en `Main`).
    derniere_nouvelle: Instant,
    pas: i32,
    /// Le pas d'après la hauteur utile (D6), avant les réductions.
    pas_plein: i32,
    /// Réductions de moitié gardées (images perdues, D10).
    reductions: u32,
    /// Réductions de moitié pour retrouver du texte dans le recouvrement (images
    /// ambiguës) ; défaites à la première avance.
    reductions_ambigues: u32,
    /// `Vu::ImmobileLu` de suite au plus petit pas (N4).
    immobiles_lus: u32,
    /// Le bas est à vérifier au plus petit pas (deux images immobiles sans position lue).
    bas_a_verifier: bool,
    /// Le dernier pas demandé (négatif : une remontée).
    dernier_pas: i32,
    /// Le moyen en cours a fait avancer la page : gardé pour toute la capture (R3).
    moyen_choisi: bool,
    /// Le moyen en cours a déjà donné une avance mesurée (pas seulement des pertes).
    avance_vue: bool,
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
            molette_vue: false,
            message_montre: false,
            lu_perdu: false,
            fin: None,
            debut,
            derniere_nouvelle: debut,
            pas: PAS_MIN,
            pas_plein: PAS_MIN,
            reductions: 0,
            reductions_ambigues: 0,
            immobiles_lus: 0,
            bas_a_verifier: false,
            dernier_pas: 0,
            moyen_choisi: false,
            avance_vue: false,
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

    /// Règle le pas d'après la hauteur utile de la zone (D6) ; un pas réduit le reste.
    pub fn regler_pas(&mut self, hauteur_utile: u32) {
        self.pas_plein = pas_de(hauteur_utile);
        self.recalculer_pas();
    }

    fn recalculer_pas(&mut self) {
        let n = (self.reductions + self.reductions_ambigues).min(30);
        self.pas = if self.bas_a_verifier {
            PAS_MIN
        } else {
            (self.pas_plein >> n).max(PAS_MIN)
        };
    }

    /// Vrai si le pas est déjà au plus petit (`PAS_MIN`).
    fn au_plus_petit(&self) -> bool {
        self.pas <= PAS_MIN
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

    /// Dit si le lu est impossible entre l'image gardée et l'image du pas qui vient d'être
    /// mesurée (`lu_perdu`), avant que celle-ci soit montrée à la course.
    pub fn regler_lu_perdu(&mut self, perdu: bool) {
        self.lu_perdu = perdu;
    }

    /// Le défileur est passé à un autre moyen de lui-même : ses deux premières commandes
    /// décideront de nouveau (R3).
    pub fn moyen_change(&mut self) {
        self.moyen_choisi = false;
        self.avance_vue = false;
        self.immobiles = 0;
        self.ambigues = 0;
        self.perdues_auto = 0;
        self.reductions = 0;
        self.reductions_ambigues = 0;
        self.recalculer_pas();
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
    /// Une image mesurée `Immobile` avec un lu non nul (`Attendu::Lu(e)`, e ≠ 0) : sur une
    /// grille parfaitement répétitive, un lu multiple de la période (C, troisième
    /// relecture N4). Elle ne compte jamais pour le bas, sauf position lue en bas.
    ImmobileLu(Option<Position>),
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
    Repli(Raison),
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
            course.molette_vue = true;
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
            Conduite::Auto => {
                course.immobiles_lus = 0;
                suite_auto(course, mesure, position, maintenant)
            }
            Conduite::Main => suite_main(course, mesure, position, maintenant),
        },
        Vu::ImmobileLu(position) => match course.conduite {
            Conduite::Attente => Vec::new(),
            Conduite::Auto => suite_auto_immobile_lu(course, position, maintenant),
            // En `Main`, la fin ne vient que de la position lue : comme une image immobile.
            Conduite::Main => suite_main(course, Mesure::Immobile, position, maintenant),
        },
    }
}

/// L'image d'un pas, pour la course : `Immobile` avec un lu non nul devient
/// `Vu::ImmobileLu` (N4). Fonction pure.
pub fn vu_image(mesure: Mesure, attendu: Attendu, position: Option<Position>) -> Vu {
    match (mesure, attendu) {
        (Mesure::Immobile, Attendu::Lu(e)) if e != 0 => Vu::ImmobileLu(position),
        _ => Vu::Image(mesure, position),
    }
}

/// Une image immobile après un pas, avec un lu non nul (N4) : la page a bougé de `e`, mais
/// l'image est la même (grille répétitive, `e` multiple de la période). Ce n'est pas le
/// bas : le pas est réduit de moitié, on remonte de la moitié du dernier mouvement pour
/// sortir du multiple, et on réessaie. Au plus petit pas, on redescend. Seule une position
/// lue en bas (≥ 99,9 %) en fait une image immobile ordinaire, ou `IMMOBILES_LUS_MAX`
/// de suite au plus petit pas (contenu fixe, la vue dit qu'elle défile).
fn suite_auto_immobile_lu(
    course: &mut Course,
    position: Option<Position>,
    maintenant: Instant,
) -> Vec<Geste> {
    if course.au_plus_petit() {
        course.immobiles_lus = course.immobiles_lus.saturating_add(1);
    }
    if en_bas_lu(position) || course.immobiles_lus >= IMMOBILES_LUS_MAX {
        return suite_auto(course, Mesure::Immobile, position, maintenant);
    }
    if let Some(lue) = position.map(|p| p.pour_cent).filter(|p| p.is_finite()) {
        course.derniere_position = Some(lue);
    }
    course.en_bas_depuis = None;
    course.immobiles = 0;
    course.ambigues = 0;
    // Le lu dit que la page a bougé : le moyen marche.
    course.moyen_choisi = true;
    if !course.au_plus_petit() {
        course.reductions_ambigues += 1;
        course.recalculer_pas();
        let recul = (course.dernier_pas.abs() / 2).max(1);
        return vec![course.descendre(-recul)];
    }
    vec![course.descendre(course.pas)]
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
        // Une image ambiguë sans lu possible ne recolle pas : elle ne confirme pas le bas.
        let confirme = match mesure {
            Mesure::Immobile => true,
            Mesure::Ambigue => !course.lu_perdu,
            _ => false,
        };
        if confirme && !bouge {
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
            course.avance_vue = true;
            course.derniere_nouvelle = maintenant;
            if course.reductions_ambigues > 0 || course.bas_a_verifier {
                // Du texte est retrouvé : le pas d'avant les images ambiguës, ou d'avant la
                // vérification du bas.
                course.reductions_ambigues = 0;
                course.bas_a_verifier = false;
                course.recalculer_pas();
            }
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
        // Après une remontée (image perdue ou ambiguë), revenir sur l'image gardée ne dit
        // rien du moyen ni du bas : on redescend, sans rien compter.
        Mesure::Immobile if !descente => {
            course.ambigues = 0;
            vec![course.descendre(course.pas)]
        }
        Mesure::Immobile => {
            course.ambigues = 0;
            course.perdues_auto = 0;
            if !en_bas
                && position.is_none()
                && course.moyen_choisi
                && course.immobiles >= 1
                && !course.au_plus_petit()
            {
                // Sans position lue, deux images immobiles ne suffisent pas : sur une liste,
                // un vrai décalage hors de portée donne aussi `Immobile` (C, sans lu). Un
                // pas au plus petit doit le redire avant de conclure au bas.
                course.bas_a_verifier = true;
                course.recalculer_pas();
                return vec![course.descendre(course.pas)];
            }
            immobile_auto(course, en_bas, maintenant)
        }
        // Sans lu possible depuis l'image gardée (page ou vue changées), une image ambiguë
        // ne recolle pas, et rien ne la rattrapera : ni descendre (tout ce qui est passé
        // serait perdu), ni le bas (l'image serait tronquée en silence, C1).
        Mesure::Ambigue if course.lu_perdu => sans_lu_ne_recolle_pas(course, maintenant),
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
            if !course.au_plus_petit() {
                // Le recouvrement n'a plus assez de texte (page clairsemée, C) : remonter de
                // la moitié du dernier mouvement, pas réduit de moitié, et réessayer.
                course.reductions_ambigues += 1;
                course.recalculer_pas();
                let recul = (course.dernier_pas.abs() / 2).max(1);
                return vec![course.descendre(-recul)];
            }
            // Au plus petit pas, sans position qui avance : compté vers « immobile ».
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
            // Remonter d'un demi-pas (D10).
            let demi = (course.pas / 2).max(1);
            if !course.au_plus_petit() {
                // D'abord réduire le pas de moitié, jusqu'au plus petit : une animation ou
                // un pas trop grand ne passent pas tout de suite à la main.
                course.reductions += 1;
                course.recalculer_pas();
                return vec![course.descendre(-demi)];
            }
            course.perdues_auto += 1;
            if course.perdues_auto > PERDUES_AUTO_MAX && course.lu_perdu {
                // Trop loin sans lu possible : la page a changé, et l'app le dit.
                return decrocher(course, maintenant);
            }
            if course.perdues_auto > PERDUES_AUTO_MAX {
                // La page saute trop loin même au plus petit pas : la molette de
                // l'utilisateur.
                course.passer_main(maintenant);
                return vec![Geste::Prendre(PRISE_MAIN)];
            }
            if !course.avance_vue && course.perdues_auto >= 2 {
                // Ce moyen n'a jamais fait qu'aller trop loin, même au pas réduit (un cran
                // plus grand que la zone, B) : le suivant, qui remonte d'abord.
                course.moyen_change();
                return vec![Geste::Repli(Raison::TropLoin), course.descendre(-demi)];
            }
            vec![course.descendre(-demi)]
        }
    }
}

/// Une image `Ambigue` en `Auto` alors que le lu est impossible depuis l'image gardée
/// (`lu_perdu`, C1) : d'abord réduire le pas et remonter de la moitié du dernier
/// mouvement (la mesure sans lu peut recoller plus près de l'image gardée, sur du contenu) ;
/// au plus petit pas, la capture s'arrête en le disant (`decrocher`). Jamais compté comme
/// immobile : la fin `en_bas` ne vient pas d'ici.
fn sans_lu_ne_recolle_pas(course: &mut Course, maintenant: Instant) -> Vec<Geste> {
    course.immobiles = 0;
    course.ambigues = 0;
    if !course.au_plus_petit() {
        course.reductions_ambigues += 1;
        course.recalculer_pas();
        let recul = (course.dernier_pas.abs() / 2).max(1);
        return vec![course.descendre(-recul)];
    }
    decrocher(course, maintenant)
}

/// L'app ne peut plus suivre la page (elle a changé de hauteur, C1) : la main passe à la
/// molette de l'utilisateur, la page est perdue et « La page a changé » est montré. Si la
/// molette ramène la dernière image gardée, le message part et la capture reprend là
/// (comme après « Trop vite ») ; sinon « Terminer » rend l'image jusque-là, l'utilisateur
/// étant prévenu qu'elle s'arrête.
fn decrocher(course: &mut Course, maintenant: Instant) -> Vec<Geste> {
    course.passer_main(maintenant);
    course.perdue = true;
    course.message_montre = true;
    vec![
        Geste::Message(Some(TEXTE_PAGE_CHANGEE)),
        Geste::Prendre(PRISE_MAIN),
    ]
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
    vec![
        Geste::Repli(Raison::SansEffet),
        course.descendre(course.pas),
    ]
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
            Mesure::Avance(_) | Mesure::Immobile => {
                // La dernière image gardée est retrouvée : la capture reprend là.
                course.perdue = false;
                if course.message_montre {
                    course.message_montre = false;
                    gestes.push(Geste::Message(None));
                }
            }
            // Encore perdue : « Trop vite » si la molette de l'utilisateur a roulé depuis un
            // passage en `Main` décidé par l'app (la page était déjà hors de portée, sans
            // rien dire) ; il roule et la page reste trop loin : c'est le seul indice qu'il
            // faut remonter. Sans molette, rien.
            Mesure::Perdue if course.molette_vue && !course.message_montre => {
                course.message_montre = true;
                return vec![
                    Geste::Message(Some(TEXTE_TROP_VITE)),
                    Geste::Prendre(PRISE_MAIN),
                ];
            }
            // Un recul peut être faux sur une page perdue (liste hors de portée, C) : il ne
            // cache pas « Trop vite ».
            Mesure::Recul(_) | Mesure::Perdue | Mesure::Ambigue => {
                return vec![Geste::Prendre(PRISE_MAIN)]
            }
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
            // Seulement si la molette de l'utilisateur a conduit : un passage en `Main` décidé
            // par l'app reste silencieux (l'aperçu qui ne grandit pas suffit).
            if course.molette_vue {
                course.message_montre = true;
                gestes.push(Geste::Message(Some(TEXTE_TROP_VITE)));
            }
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
/// maintenant, en pixels réels de la vue qui défile (`defiler::decalage_px`, B) ; une
/// position inchangée donne `Lu(0)`. `Lu` seulement si les deux positions ont été lues ;
/// sinon, ou si la vue ou la page ont changé de hauteur entre les deux (vue redimensionnée,
/// image paresseuse, chargement à l'infini : les pour cent ne se comparent plus), `Aucun`,
/// sauf depuis le haut de la page où le lu reste exact : l'image est mesurée sans lu, et la
/// position gardée repart juste à la prochaine image gardée ; d'ici là, une image qui ne
/// recolle pas ne conclut jamais au bas (`lu_perdu`, C1). Les pixels commandés ne sont jamais
/// passés à la mesure (C, seconde reprise R1 et R3). Dans une vue unie, un `Lu(e)` non nul
/// fait rendre `Avance(e)` (C, complément) : il ne vient donc que d'une vraie lecture.
pub fn attendu(gardee: Option<Position>, maintenant: Option<Position>) -> Attendu {
    match (gardee, maintenant) {
        (Some(g), Some(m)) => defiler::decalage_px(&g, &m).map_or(Attendu::Aucun, Attendu::Lu),
        _ => Attendu::Aucun,
    }
}

/// Vrai si les deux positions sont lues mais que le lu est impossible entre elles (la page
/// ou la vue ont changé de hauteur, `attendu` rend `Aucun`) : `Course::regler_lu_perdu`.
/// Fonction pure.
pub fn lu_perdu(gardee: Option<Position>, maintenant: Option<Position>) -> bool {
    matches!((gardee, maintenant), (Some(g), Some(m)) if defiler::decalage_px(&g, &m).is_none())
}

/// Bornes du facteur d'étalonnage du lu (décision du responsable, relecture d'ensemble
/// point 3) : un rectangle de vue qui compte un en-tête ou une barre fausse le lu d'un
/// facteur constant, de quelques pour cent ; au-delà de ces bornes, ce n'est plus un biais.
pub const ETALON_MIN: f64 = 0.5;
pub const ETALON_MAX: f64 = 2.0;
/// `Ambigue` de suite avec un lu non étalonné avant de remesurer sans lu.
pub const AMBIGUES_LUES_MAX: u32 = 2;

/// Le facteur d'étalonnage du lu d'après une avance mesurée sur du contenu (la mesure sans
/// lu la redit) : `avance / lu`, borné à [`ETALON_MIN`, `ETALON_MAX`]. Rien pour un lu nul
/// ou négatif, ou une avance nulle. Fonction pure.
pub fn etalon(avance: u32, lu: i32) -> Option<f64> {
    (lu > 0 && avance > 0)
        .then(|| (f64::from(avance) / f64::from(lu)).clamp(ETALON_MIN, ETALON_MAX))
}

/// Le lu corrigé par l'étalonnage de la capture, s'il y en a un. Fonction pure.
pub fn lu_etalonne(attendu: Attendu, etalon: Option<f64>) -> Attendu {
    match (attendu, etalon) {
        (Attendu::Lu(e), Some(k)) => {
            let v = (f64::from(e) * k).round();
            if v.abs() < f64::from(i32::MAX) {
                Attendu::Lu(v as i32)
            } else {
                Attendu::Aucun
            }
        }
        _ => attendu,
    }
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

    /// Une fenêtre de contrôle restée d'une capture précédente est reprise
    /// (`Controle::retrouver`, R1) : sa page est déjà chargée et écoute, elle n'enverra
    /// plus `defilante_pret`.
    pub fn controle_retrouve(&mut self) -> Etape {
        self.prete = true;
        self.controle_cree()
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
/// Pourquoi un moyen est laissé (journal).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Raison {
    /// Deux pas sans effet (R3, D8).
    SansEffet,
    /// Le défileur l'a quitté de lui-même : il ne peut plus servir (B).
    Impossible,
    /// Un cran fait plus que la zone ne recouvre (B).
    CranTropGrand,
    /// Chaque pas, même réduit, va trop loin (image perdue), sans jamais d'avance.
    TropLoin,
}

impl Raison {
    /// Nom écrit au journal : `sans effet`, `impossible`, `cran trop grand`, `trop loin`.
    pub fn nom(self) -> &'static str {
        match self {
            Raison::SansEffet => "sans effet",
            Raison::Impossible => "impossible",
            Raison::CranTropGrand => "cran trop grand",
            Raison::TropLoin => "trop loin",
        }
    }
}

pub fn ligne_repli(ancien: Option<Moyen>, nouveau: Option<Moyen>, raison: Raison) -> String {
    match (nouveau, ancien) {
        (Some(n), Some(a)) => {
            format!(
                "defilante repli: {} ({} {})",
                n.nom(),
                a.nom(),
                raison.nom()
            )
        }
        (Some(n), None) => format!("defilante repli: {}", n.nom()),
        (None, _) => "defilante repli: molette".into(),
    }
}

/// La main passe à la molette de l'utilisateur (D9).
pub const LIGNE_MAIN: &str = "defilante main (molette)";
/// « Trop vite » montré, puis caché (D10).
pub const LIGNE_TROP_VITE: &str = "defilante trop vite";
pub const LIGNE_REPRIS: &str = "defilante repris";
/// « La page a changé » montré (C1).
pub const LIGNE_PAGE_CHANGEE: &str = "defilante page changee";

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
            hauteur_vue: 400,
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
        assert_eq!(p.pilule, (400 + 300 - 50, 660));
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
                vec![Geste::Repli(Raison::SansEffet), Geste::Descendre(200)]
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
            vec![Geste::Repli(Raison::SansEffet), Geste::Descendre(200)]
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
            vec![Geste::Repli(Raison::SansEffet), Geste::Descendre(200)]
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
    fn immobile_avec_un_lu_non_nul_n_est_pas_le_bas() {
        // N4 : grille parfaitement répétitive, le lu exact (une période) donne `Immobile`.
        assert_eq!(
            vu_image(Mesure::Immobile, Attendu::Lu(200), pos(50.0)),
            Vu::ImmobileLu(pos(50.0))
        );
        assert_eq!(
            vu_image(Mesure::Immobile, Attendu::Lu(0), None),
            Vu::Image(Mesure::Immobile, None)
        );
        assert_eq!(
            vu_image(Mesure::Immobile, Attendu::Aucun, None),
            Vu::Image(Mesure::Immobile, None)
        );
        assert_eq!(
            vu_image(Mesure::Avance(200), Attendu::Lu(200), None),
            Vu::Image(Mesure::Avance(200), None)
        );
        let t0 = Instant::now();
        let mut c = en_auto(t0);
        let t = t0 + ms(600);
        suite(&mut c, Vu::Image(Mesure::Avance(200), None), t);
        // Dès le premier pas, puis à chaque fois : pas réduit de moitié et remontée de la
        // moitié du dernier mouvement, jamais « en bas » ni un repli.
        assert_eq!(
            suite(&mut c, Vu::ImmobileLu(pos(40.0)), t),
            vec![Geste::Descendre(-100)]
        );
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Recul(100), pos(35.0)), t),
            vec![Geste::Descendre(100)]
        );
        assert_eq!(
            suite(&mut c, Vu::ImmobileLu(pos(40.0)), t),
            vec![Geste::Descendre(-50)]
        );
        // Puis jusqu'au plus petit pas (40) et au-delà : toujours un pas, jamais compté.
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Recul(50), pos(37.0)), t),
            vec![Geste::Descendre(50)]
        );
        for _ in 0..10 {
            let g = suite(&mut c, Vu::ImmobileLu(pos(40.0)), t);
            assert!(matches!(g[..], [Geste::Descendre(_)]), "{g:?}");
            suite(&mut c, Vu::Image(Mesure::Recul(1), pos(39.0)), t);
        }
        assert_eq!(c.fin(), None);
        // Le texte retrouvé rend le pas plein.
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Avance(40), pos(41.0)), t),
            vec![Geste::Apercu, Geste::Descendre(200)]
        );
        // Une position lue en bas : la fin, comme une image immobile.
        assert_eq!(
            suite(&mut c, Vu::ImmobileLu(pos(99.95)), t),
            vec![Geste::Prendre(FIN_EN_BAS)]
        );
        assert_eq!(
            suite(&mut c, Vu::ImmobileLu(pos(99.95)), t + FIN_EN_BAS),
            vec![Geste::Finir(Fin::EnBas)]
        );
        // Sans position en bas, deux `ImmobileLu` de suite ne finissent jamais, alors que
        // deux images immobiles sans lu (ou avec un lu nul) le font.
        let mut c = en_auto(t0);
        suite(&mut c, Vu::Image(Mesure::Avance(200), None), t);
        for _ in 0..2 {
            let g = suite(&mut c, Vu::ImmobileLu(None), t);
            assert!(matches!(g[..], [Geste::Descendre(n)] if n < 0), "{g:?}");
        }
        assert_eq!(c.fin(), None);
    }

    #[test]
    fn immobiles_lus_bornes_au_plus_petit_pas() {
        // Contenu fixe dans une vue dont l'application dit qu'elle défile : chaque pas lu
        // donne `Immobile`. Le pas est réduit jusqu'au plus petit (40), puis, après
        // `IMMOBILES_LUS_MAX` de suite à ce pas, ce sont des images immobiles ordinaires.
        let t0 = Instant::now();
        let mut c = en_auto(t0);
        let t = t0 + ms(600);
        suite(&mut c, Vu::Image(Mesure::Avance(200), None), t);
        let lu = Vu::ImmobileLu(None);
        assert_eq!(suite(&mut c, lu, t), vec![Geste::Descendre(-100)]);
        assert_eq!(suite(&mut c, lu, t), vec![Geste::Descendre(-50)]);
        assert_eq!(suite(&mut c, lu, t), vec![Geste::Descendre(-25)]);
        // Au plus petit pas : deux fois on redescend.
        assert_eq!(suite(&mut c, lu, t), vec![Geste::Descendre(40)]);
        assert_eq!(suite(&mut c, lu, t), vec![Geste::Descendre(40)]);
        // Une autre image entre deux remet le compte à zéro.
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Recul(5), None), t),
            vec![Geste::Descendre(40)]
        );
        assert_eq!(suite(&mut c, lu, t), vec![Geste::Descendre(40)]);
        assert_eq!(suite(&mut c, lu, t), vec![Geste::Descendre(40)]);
        // La troisième de suite : une image immobile (la première des deux du bas).
        assert_eq!(suite(&mut c, lu, t), vec![Geste::Descendre(40)]);
        assert_eq!(suite(&mut c, lu, t), vec![Geste::Prendre(FIN_EN_BAS)]);
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Immobile, None), t + FIN_EN_BAS),
            vec![Geste::Finir(Fin::EnBas)]
        );
        assert_eq!(c.fin(), Some(Fin::EnBas));
    }

    #[test]
    fn sans_position_le_bas_est_verifie_au_plus_petit_pas() {
        // C, point ouvert : sur une liste, sans lu, un vrai décalage hors de portée donne
        // `Immobile`. Deux images immobiles sans position ne concluent pas au bas tout de
        // suite : un pas au plus petit doit le redire.
        let t0 = Instant::now();
        let mut c = en_auto(t0);
        let t = t0 + ms(600);
        let immobile = Vu::Image(Mesure::Immobile, None);
        suite(&mut c, Vu::Image(Mesure::Avance(200), None), t);
        assert_eq!(suite(&mut c, immobile, t), vec![Geste::Descendre(200)]);
        assert_eq!(suite(&mut c, immobile, t), vec![Geste::Descendre(PAS_MIN)]);
        // Le petit pas avance : ce n'était pas le bas, le pas plein revient.
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Avance(40), None), t),
            vec![Geste::Apercu, Geste::Descendre(200)]
        );
        assert_eq!(c.fin(), None);
        // Le petit pas redit `Immobile` : le bas.
        assert_eq!(suite(&mut c, immobile, t), vec![Geste::Descendre(200)]);
        assert_eq!(suite(&mut c, immobile, t), vec![Geste::Descendre(PAS_MIN)]);
        assert_eq!(suite(&mut c, immobile, t), vec![Geste::Prendre(FIN_EN_BAS)]);
        // Avec une position lue (qui ne dit pas le bas), deux images immobiles suffisent.
        let mut c = en_auto(t0);
        suite(&mut c, Vu::Image(Mesure::Avance(200), pos(40.0)), t);
        let lue = Vu::Image(Mesure::Immobile, pos(40.0));
        assert_eq!(suite(&mut c, lue, t), vec![Geste::Descendre(200)]);
        assert_eq!(suite(&mut c, lue, t), vec![Geste::Prendre(FIN_EN_BAS)]);
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
        // Ensuite deux images immobiles disent « en bas », jamais un repli ; sans position
        // lue, un pas au plus petit le redit d'abord.
        assert_eq!(suite(&mut c, immobile, t), vec![Geste::Descendre(200)]);
        assert_eq!(suite(&mut c, immobile, t), vec![Geste::Descendre(PAS_MIN)]);
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
    fn sans_lu_une_image_ambigue_ne_conclut_jamais_au_bas() {
        // Relecture des correctifs, C1 : la page a changé de hauteur depuis l'image gardée
        // (`lu_perdu`), et l'image d'un pas est ambiguë, la position déjà lue au bas. Avant :
        // fin `en_bas`, l'image tronquée sans rien dire. Maintenant : pas réduit et remontée
        // jusqu'au plus petit pas, puis la main à la molette avec « La page a changé ».
        let t0 = Instant::now();
        let mut c = en_auto(t0);
        let t = t0 + ms(600);
        suite(&mut c, Vu::Image(Mesure::Avance(200), pos(80.0)), t);
        c.regler_lu_perdu(true);
        let mut gestes = Vec::new();
        for i in 0..10 {
            let g = suite(
                &mut c,
                Vu::Image(Mesure::Ambigue, pos(100.0)),
                t + ms(50 * i),
            );
            assert!(!g.iter().any(|g| matches!(g, Geste::Finir(_))), "{g:?}");
            assert!(!g.contains(&Geste::Prendre(FIN_EN_BAS)), "{g:?}");
            let fini = c.conduite() == Conduite::Main;
            gestes.push(g);
            if fini {
                break;
            }
        }
        // 200 → 100 → 50 → 40 : trois remontées, puis le message.
        assert_eq!(
            gestes,
            vec![
                vec![Geste::Descendre(-100)],
                vec![Geste::Descendre(-50)],
                vec![Geste::Descendre(-25)],
                vec![
                    Geste::Message(Some(TEXTE_PAGE_CHANGEE)),
                    Geste::Prendre(PRISE_MAIN)
                ],
            ]
        );
        assert!(c.perdue());
        // En `Main`, encore perdue : rien d'autre, et jamais de fin seule.
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Ambigue, pos(100.0)), t + ms(600)),
            vec![Geste::Prendre(PRISE_MAIN)]
        );
        assert_eq!(
            suite(
                &mut c,
                Vu::Image(Mesure::Immobile, pos(100.0)),
                t + ms(2000)
            ),
            vec![Geste::Message(None), Geste::Prendre(PRISE_MAIN)],
            "la dernière image gardée retrouvée : le message part"
        );
        assert_eq!(c.fin(), None);
    }

    #[test]
    fn sans_lu_une_image_ambigue_ne_confirme_pas_l_arret_en_bas() {
        let t0 = Instant::now();
        let mut c = en_auto(t0);
        let t = t0 + ms(600);
        suite(&mut c, Vu::Image(Mesure::Avance(200), pos(80.0)), t);
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Immobile, pos(100.0)), t),
            vec![Geste::Prendre(FIN_EN_BAS)]
        );
        // Pendant l'arrêt, la page change et l'image ne recolle plus : pas de fin.
        c.regler_lu_perdu(true);
        let g = suite(&mut c, Vu::Image(Mesure::Ambigue, pos(100.0)), t + ms(400));
        assert_eq!(g, vec![Geste::Descendre(-100)]);
        assert_eq!(c.fin(), None);
        // Le même arrêt avec le lu possible finit comme avant.
        let mut c = en_auto(t0);
        suite(&mut c, Vu::Image(Mesure::Avance(200), pos(80.0)), t);
        suite(&mut c, Vu::Image(Mesure::Immobile, pos(100.0)), t);
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Ambigue, pos(100.0)), t + ms(400)),
            vec![Geste::Finir(Fin::EnBas)]
        );
    }

    #[test]
    fn sans_lu_les_images_perdues_le_disent() {
        // Le cas voisin de C1 : sans lu possible, la suite est perdue même au plus petit pas.
        // La main passe à la molette en le disant (sans le lu perdu : sans rien dire, D8).
        let passe = |lu_perdu: bool| {
            let t0 = Instant::now();
            let mut c = en_auto(t0);
            let t = t0 + ms(600);
            suite(&mut c, Vu::Image(Mesure::Avance(200), pos(50.0)), t);
            c.regler_lu_perdu(lu_perdu);
            let mut n = 0;
            loop {
                let g = suite(&mut c, Vu::Image(Mesure::Perdue, pos(60.0)), t);
                if c.conduite() == Conduite::Main {
                    return (g, c.perdue());
                }
                n += 1;
                assert!(n < 20);
            }
        };
        assert_eq!(
            passe(true),
            (
                vec![
                    Geste::Message(Some(TEXTE_PAGE_CHANGEE)),
                    Geste::Prendre(PRISE_MAIN)
                ],
                true
            )
        );
        assert_eq!(passe(false), (vec![Geste::Prendre(PRISE_MAIN)], false));
    }

    #[test]
    fn lu_perdu_seulement_si_les_deux_positions_sont_lues() {
        let lecture = |pour_cent: f64, vue: f64| {
            Some(Position {
                pour_cent,
                vue,
                hauteur_vue: 300,
            })
        };
        assert!(!lu_perdu(None, lecture(10.0, 10.0)));
        assert!(!lu_perdu(lecture(10.0, 10.0), None));
        assert!(!lu_perdu(lecture(10.0, 10.0), lecture(20.0, 10.0)));
        // La page a changé de hauteur (10 % puis 9 % de vue).
        assert!(lu_perdu(lecture(10.0, 10.0), lecture(20.0, 9.0)));
        // Depuis le haut, le lu reste possible.
        assert!(!lu_perdu(lecture(0.0, 10.0), lecture(20.0, 9.0)));
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
    fn trop_vite_seulement_apres_la_molette() {
        // Auto, une avance, puis des pertes au plus petit pas jusqu'au passage en `Main`
        // décidé par l'app : aucune perte ne montre « Trop vite » sans cran de l'utilisateur.
        let t0 = Instant::now();
        let mut c = en_auto(t0);
        let t = t0 + ms(600);
        suite(&mut c, Vu::Image(Mesure::Avance(200), None), t);
        let mut n = 0;
        while c.conduite() == Conduite::Auto {
            let g = suite(&mut c, Vu::Image(Mesure::Perdue, None), t);
            assert!(!g.iter().any(|g| matches!(g, Geste::Message(_))), "{g:?}");
            n += 1;
            assert!(n < 20);
        }
        let g = suite(&mut c, Vu::Image(Mesure::Perdue, None), t);
        assert_eq!(g, vec![Geste::Prendre(PRISE_MAIN)]);
        assert!(c.perdue());
        // Retrouvée : rien à cacher, rien n'a été montré.
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Avance(40), None), t),
            vec![Geste::Apercu, Geste::Prendre(PRISE_MAIN)]
        );
        // Puis l'utilisateur fait défiler, trop loin : le message.
        suite(&mut c, Vu::Molette { dans_zone: true }, t);
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Perdue, None), t),
            vec![
                Geste::Message(Some(TEXTE_TROP_VITE)),
                Geste::Prendre(PRISE_MAIN)
            ]
        );
    }

    #[test]
    fn trop_vite_apres_un_passage_en_main_silencieux_si_la_molette_roule() {
        // Relecture finale, P3 : l'app passe en `Main` d'elle-même (plus de moyen) alors
        // que la page est déjà hors de portée de la dernière image gardée.
        let t0 = Instant::now();
        let mut c = Course::nouvelle(t0);
        c.regler_pas(600);
        assert_eq!(c.sans_moyen(t0), vec![Geste::Prendre(PRISE_MAIN)]);
        let message = |g: &[Geste]| g.iter().any(|g| matches!(g, Geste::Message(Some(_))));
        // Sans molette de l'utilisateur, rien n'est montré, aussi longtemps qu'elle reste
        // perdue (un cran hors de la zone ne compte pas).
        for i in 1..=5 {
            let g = suite(&mut c, Vu::Image(Mesure::Perdue, None), t0 + ms(50 * i));
            assert_eq!(g, vec![Geste::Prendre(PRISE_MAIN)], "image {i}");
        }
        assert!(c.perdue());
        assert!(suite(&mut c, Vu::Molette { dans_zone: false }, t0 + ms(300)).is_empty());
        let g = suite(&mut c, Vu::Image(Mesure::Perdue, None), t0 + ms(350));
        assert!(!message(&g), "{g:?}");
        // L'utilisateur roule dans la zone, la page reste trop loin : « Trop vite », une
        // fois.
        suite(&mut c, Vu::Molette { dans_zone: true }, t0 + ms(400));
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Perdue, None), t0 + ms(450)),
            vec![
                Geste::Message(Some(TEXTE_TROP_VITE)),
                Geste::Prendre(PRISE_MAIN)
            ]
        );
        suite(&mut c, Vu::Molette { dans_zone: true }, t0 + ms(500));
        for g in [
            suite(&mut c, Vu::Image(Mesure::Perdue, None), t0 + ms(550)),
            suite(&mut c, Vu::Image(Mesure::Ambigue, None), t0 + ms(600)),
            suite(&mut c, Vu::Image(Mesure::Recul(30), None), t0 + ms(650)),
        ] {
            assert_eq!(g, vec![Geste::Prendre(PRISE_MAIN)]);
        }
        // Il remonte assez : la dernière image gardée est retrouvée, le message part.
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Avance(40), None), t0 + ms(700)),
            vec![
                Geste::Message(None),
                Geste::Apercu,
                Geste::Prendre(PRISE_MAIN)
            ]
        );
        assert!(!c.perdue());
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
        // Un recul peut être faux sur une page perdue (C) : le message reste.
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Recul(10), None), t),
            vec![Geste::Prendre(PRISE_MAIN)]
        );
        assert!(c.perdue());
        // Retrouvée (en remontant jusqu'à l'image gardée) : message caché, la capture
        // reprend là.
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Immobile, None), t),
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
    fn auto_qui_remonte_et_reduit_son_pas_jusqu_au_plus_petit() {
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
        // Les pertes réduisent encore le pas, jusqu'au plus petit (animation, C).
        assert_eq!(suite(&mut c, perdue, t), vec![Geste::Descendre(-50)]);
        assert_eq!(c.pas(), 50);
        assert_eq!(suite(&mut c, perdue, t), vec![Geste::Descendre(-25)]);
        assert_eq!(c.pas(), PAS_MIN);
        c.regler_pas(400);
        assert_eq!(c.pas(), PAS_MIN);
        // Au plus petit pas, toujours perdue : au-delà de `PERDUES_AUTO_MAX`, la molette,
        // sans message.
        for _ in 0..PERDUES_AUTO_MAX {
            assert_eq!(suite(&mut c, perdue, t), vec![Geste::Descendre(-20)]);
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
        // Encore perdue : un demi-pas du pas réduit, réduit encore.
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Perdue, None), t),
            vec![Geste::Descendre(-50)]
        );
        assert_eq!(c.pas(), 50);
        assert_eq!(c.conduite(), Conduite::Auto);
        assert!(!c.perdue());
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
    fn perdue_des_le_premier_pas_le_moyen_suivant() {
        let t0 = Instant::now();
        let mut c = en_auto(t0);
        let t = t0 + ms(600);
        let perdue = Vu::Image(Mesure::Perdue, None);
        // Le premier pas va trop loin : remonter, pas réduit.
        assert_eq!(suite(&mut c, perdue, t), vec![Geste::Descendre(-100)]);
        // Revenue sur l'image gardée après une remontée : on redescend, sans compter.
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Immobile, None), t),
            vec![Geste::Descendre(100)]
        );
        assert_eq!(suite(&mut c, perdue, t), vec![Geste::Descendre(-50)]);
        assert_eq!(suite(&mut c, perdue, t), vec![Geste::Descendre(-25)]);
        // Au plus petit pas, encore trop loin, sans jamais d'avance : le moyen suivant.
        assert_eq!(suite(&mut c, perdue, t), vec![Geste::Descendre(-20)]);
        assert_eq!(
            suite(&mut c, perdue, t),
            vec![Geste::Repli(Raison::TropLoin), Geste::Descendre(-20)]
        );
        assert_eq!(c.conduite(), Conduite::Auto);
        // Le suivant repart du pas plein.
        assert_eq!(c.pas(), 200);
        assert_eq!(suite(&mut c, perdue, t), vec![Geste::Descendre(-100)]);
    }

    #[test]
    fn perdue_puis_remontee_immobile_finit_en_main() {
        let t0 = Instant::now();
        let mut c = en_auto(t0);
        let t = t0 + ms(600);
        let perdue = Vu::Image(Mesure::Perdue, None);
        let immobile = Vu::Image(Mesure::Immobile, None);
        // Le moyen a fait avancer la page une fois.
        suite(&mut c, Vu::Image(Mesure::Avance(200), None), t);
        // Puis chaque pas perd l'image ; chaque remontée revient sur l'image gardée. Trois
        // pertes réduisent le pas (200, 100, 50, 40), puis `PERDUES_AUTO_MAX` sont comptées.
        for _ in 0..(3 + PERDUES_AUTO_MAX) {
            let g = suite(&mut c, perdue, t);
            assert!(matches!(g[..], [Geste::Descendre(p)] if p < 0), "{g:?}");
            let g = suite(&mut c, immobile, t);
            assert!(matches!(g[..], [Geste::Descendre(p)] if p > 0), "{g:?}");
        }
        assert_eq!(suite(&mut c, perdue, t), vec![Geste::Prendre(PRISE_MAIN)]);
        assert_eq!(c.conduite(), Conduite::Main);
    }

    #[test]
    fn ambigue_l_image_suivante_decide() {
        let t0 = Instant::now();
        let mut c = en_auto(t0);
        let t = t0 + ms(600);
        let ambigue = Vu::Image(Mesure::Ambigue, None);
        // Pas assez de texte dans le recouvrement : remonter de la moitié du pas, pas
        // réduit de moitié, réessayer (rien d'ajouté).
        assert_eq!(suite(&mut c, ambigue, t), vec![Geste::Descendre(-100)]);
        assert_eq!(c.pas(), 100);
        // Du texte retrouvé : le pas plein revient.
        assert_eq!(
            suite(&mut c, Vu::Image(Mesure::Avance(100), None), t),
            vec![Geste::Apercu, Geste::Descendre(200)]
        );
        // Une page unie sans fin : le pas descend jusqu'au plus petit, puis compte.
        assert_eq!(suite(&mut c, ambigue, t), vec![Geste::Descendre(-100)]);
        assert_eq!(suite(&mut c, ambigue, t), vec![Geste::Descendre(-50)]);
        assert_eq!(suite(&mut c, ambigue, t), vec![Geste::Descendre(-25)]);
        assert_eq!(c.pas(), PAS_MIN);
        for _ in 1..AMBIGUES_MAX {
            assert_eq!(suite(&mut c, ambigue, t), vec![Geste::Descendre(PAS_MIN)]);
        }
        // `AMBIGUES_MAX` au plus petit pas : une image immobile, puis une seconde, le bas.
        assert_eq!(suite(&mut c, ambigue, t), vec![Geste::Descendre(PAS_MIN)]);
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
        suite(&mut c, Vu::Image(Mesure::Avance(200), pos(0.0)), t);
        for i in 1..(3 * AMBIGUES_MAX) {
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
    fn etalonnage_du_lu() {
        // Un lu 5 % trop grand : 158 pour une vraie avance de 150.
        let k = etalon(150, 158).unwrap();
        assert!((k - 150.0 / 158.0).abs() < 1e-9);
        assert_eq!(lu_etalonne(Attendu::Lu(158), Some(k)), Attendu::Lu(150));
        assert_eq!(lu_etalonne(Attendu::Lu(-316), Some(k)), Attendu::Lu(-300));
        // Borné.
        assert_eq!(etalon(400, 100), Some(ETALON_MAX));
        assert_eq!(etalon(10, 100), Some(ETALON_MIN));
        // Rien sans lu positif ni avance.
        assert_eq!(etalon(150, 0), None);
        assert_eq!(etalon(150, -150), None);
        assert_eq!(etalon(0, 150), None);
        // Sans étalonnage, ou sans lu : inchangé.
        assert_eq!(lu_etalonne(Attendu::Lu(158), None), Attendu::Lu(158));
        assert_eq!(lu_etalonne(Attendu::Aucun, Some(k)), Attendu::Aucun);
    }

    #[test]
    fn attendu_seulement_observe() {
        let p = |pour_cent| {
            Some(Position {
                pour_cent,
                vue: 20.0,
                hauteur_vue: 400,
            })
        };
        // Observé : vue de 400 px qui montre 20 % de la page, parcours de 1 600 px ; 10 %
        // de plus = 160 px (B, defiler::decalage_px), dans les deux sens.
        assert_eq!(attendu(p(10.0), p(20.0)), Attendu::Lu(160));
        assert_eq!(attendu(p(20.0), p(10.0)), Attendu::Lu(-160));
        // Position inchangée : 0, jamais le pas commandé.
        assert_eq!(attendu(p(10.0), p(10.0)), Attendu::Lu(0));
        // Sans position : `Aucun` (le pas commandé n'est jamais passé à la mesure).
        assert_eq!(attendu(None, p(10.0)), Attendu::Aucun);
        assert_eq!(attendu(p(10.0), None), Attendu::Aucun);
        assert_eq!(attendu(None, None), Attendu::Aucun);
        // Vue illisible (toute la page visible), ou redimensionnée entre les deux : rien.
        let toute = Some(Position {
            pour_cent: 0.0,
            vue: 100.0,
            hauteur_vue: 400,
        });
        assert_eq!(attendu(p(10.0), toute), Attendu::Aucun);
        let plus_haute = Some(Position {
            pour_cent: 20.0,
            vue: 20.0,
            hauteur_vue: 500,
        });
        assert_eq!(attendu(p(10.0), plus_haute), Attendu::Aucun);
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
    fn filet_de_la_selection_apres_la_borne_de_la_page() {
        // Le filet de la page de sélection ne rend pas la main avant l'abandon du cœur.
        let page = include_str!("../../../interface/selection/selection.js");
        let ligne = format!(
            "const FILET_DEFILANTE_MS = FILET_MS + {};",
            DELAI_PAGE.as_millis()
        );
        assert!(page.contains(&ligne), "{ligne}");
    }

    #[test]
    fn lancement_controle_retrouve() {
        // Retrouvée pendant la visée : la page écoute déjà, montrée au relâchement.
        let mut l = Lancement::default();
        assert_eq!(l.controle_retrouve(), Etape::Rien);
        assert!(l.prete);
        assert_eq!(l.relache(), Etape::Montrer);
        // Retrouvée après le relâchement : montrée tout de suite.
        let mut l = Lancement::default();
        assert_eq!(l.relache(), Etape::Rien);
        assert_eq!(l.controle_retrouve(), Etape::Montrer);
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
            ligne_repli(
                Some(Moyen::Message),
                Some(Moyen::Automation),
                Raison::SansEffet
            ),
            "defilante repli: automation (message sans effet)"
        );
        assert_eq!(
            ligne_repli(
                Some(Moyen::Automation),
                Some(Moyen::Molette),
                Raison::Impossible
            ),
            "defilante repli: molette (automation impossible)"
        );
        assert_eq!(
            ligne_repli(
                Some(Moyen::Molette),
                Some(Moyen::Message),
                Raison::CranTropGrand
            ),
            "defilante repli: message (molette cran trop grand)"
        );
        assert_eq!(
            ligne_repli(Some(Moyen::Molette), None, Raison::SansEffet),
            "defilante repli: molette"
        );
        assert_eq!(LIGNE_MAIN, "defilante main (molette)");
        assert_eq!(LIGNE_TROP_VITE, "defilante trop vite");
        assert_eq!(LIGNE_REPRIS, "defilante repris");
        assert_eq!(LIGNE_PAGE_CHANGEE, "defilante page changee");
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
