//! Géométrie du mode « fenêtre », en fonctions pures (pixels réels du bureau virtuel).
//!
//! La fenêtre est découpée dans la photo, coupée à l'écran de la souris ; ses coins sont
//! arrondis de façon approchée (Windows ne donne pas le rayon des coins).

use super::geometrie::{ecran_sous, zone_valide};
use crate::systeme::gel::{Coins, EcranInfo, FenetreInfo};
use crate::systeme::Rect;

/// Rayon des coins arrondis de Windows 11 à 100 %, en pixels.
pub const RAYON_ARRONDIS: i32 = 8;
/// Rayon des petits arrondis à 100 %, en pixels.
pub const RAYON_PETITS_ARRONDIS: i32 = 4;

/// La première fenêtre de la liste (du premier plan vers le fond) qui contient le point.
pub fn fenetre_sous(point: (i32, i32), fenetres: &[FenetreInfo]) -> Option<usize> {
    fenetres.iter().position(|f| f.rect.contient(point))
}

/// La fenêtre coupée à l'écran du point. `None` si le point n'est sur aucun écran, ou si
/// ce qui reste fait moins de `COTE_MIN` de côté.
pub fn zone_fenetre(fenetre: Rect, point: (i32, i32), ecrans: &[EcranInfo]) -> Option<Rect> {
    let ecran = ecran_sous(point, ecrans)?;
    fenetre.intersection(ecran.rect).filter(|z| zone_valide(*z))
}

/// Rayon des coins, en pixels réels : 0 pour des coins droits ou une fenêtre qui couvre
/// un écran entier ; sinon 8 (4 pour les petits arrondis) × l'échelle de l'écran qui
/// contient le centre de la fenêtre (à défaut celui qu'elle recouvre le plus, à défaut
/// 1), arrondi au pixel, au plus la moitié du petit côté.
pub fn rayon_coins(coins: Coins, fenetre: Rect, ecrans: &[EcranInfo]) -> i32 {
    let base = match coins {
        Coins::Droits => return 0,
        Coins::Arrondis => RAYON_ARRONDIS,
        Coins::PetitsArrondis => RAYON_PETITS_ARRONDIS,
    };
    if fenetre.est_vide() || ecrans.iter().any(|e| fenetre.englobe(e.rect)) {
        return 0;
    }
    let echelle = echelle_de(fenetre, ecrans);
    let rayon = (base as f64 * echelle).round() as i32;
    rayon.min(fenetre.largeur.min(fenetre.hauteur) / 2).max(0)
}

/// Échelle de l'écran de la fenêtre (voir `rayon_coins`). Une échelle absurde vaut 1.
fn echelle_de(fenetre: Rect, ecrans: &[EcranInfo]) -> f64 {
    let centre = |debut: i32, cote: i32| (debut as i64 + cote as i64 / 2) as i32;
    let point = (
        centre(fenetre.x, fenetre.largeur),
        centre(fenetre.y, fenetre.hauteur),
    );
    let aire = |r: Rect| r.largeur as i64 * r.hauteur as i64;
    let ecran = ecran_sous(point, ecrans).or_else(|| {
        ecrans
            .iter()
            .filter_map(|e| fenetre.intersection(e.rect).map(|i| (aire(i), e)))
            .max_by_key(|(a, _)| *a)
            .map(|(_, e)| e)
    });
    match ecran.map(|e| e.echelle) {
        Some(echelle) if echelle.is_finite() && echelle > 0.0 => echelle,
        _ => 1.0,
    }
}

/// Coins de la zone à arrondir : haut gauche, haut droit, bas gauche, bas droit. Vrai
/// seulement si ce coin de la zone est un vrai coin de la fenêtre (une fenêtre coupée
/// par le bord de l'écran garde des coins droits de ce côté).
pub fn coins_a_arrondir(fenetre: Rect, zone: Rect) -> [bool; 4] {
    let gauche = zone.x == fenetre.x;
    let haut = zone.y == fenetre.y;
    let droite = zone.droite() == fenetre.droite();
    let bas = zone.bas() == fenetre.bas();
    [gauche && haut, droite && haut, gauche && bas, droite && bas]
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

    fn ecran(rect: Rect, echelle: f64) -> EcranInfo {
        EcranInfo {
            rect,
            zone_utile: rect,
            echelle,
            principal: rect.x == 0 && rect.y == 0,
        }
    }

    /// Quatre écrans, quatre échelles : à gauche un 1440p à 150 % (plus haut que le
    /// principal), le principal 1080p à 100 %, au-dessus un 1080p à 125 %, à droite un
    /// 4K à 200 %.
    fn ecrans() -> Vec<EcranInfo> {
        vec![
            ecran(r(-2560, -300, 2560, 1440), 1.5),
            ecran(r(0, -1080, 1920, 1080), 1.25),
            ecran(r(0, 0, 1920, 1080), 1.0),
            ecran(r(1920, 0, 3840, 2160), 2.0),
        ]
    }

    fn fenetre(rect: Rect) -> FenetreInfo {
        FenetreInfo {
            rect,
            titre: String::new(),
            coins: Coins::Arrondis,
        }
    }

    #[test]
    fn premiere_fenetre_sous_le_point() {
        // Du premier plan vers le fond : une petite devant une grande.
        let liste = vec![
            fenetre(r(100, 100, 200, 200)),
            fenetre(r(0, 0, 1000, 800)),
            fenetre(r(-800, -200, 900, 400)),
        ];
        assert_eq!(fenetre_sous((150, 150), &liste), Some(0));
        assert_eq!(fenetre_sous((50, 50), &liste), Some(1));
        assert_eq!(fenetre_sous((-1, -1), &liste), Some(2));
        assert_eq!(fenetre_sous((-800, -200), &liste), Some(2));
        // Bords droit et bas exclus.
        assert_eq!(fenetre_sous((300, 150), &liste), Some(1));
        assert_eq!(fenetre_sous((1000, 0), &liste), None);
        assert_eq!(fenetre_sous((5000, 5000), &liste), None);
        assert_eq!(fenetre_sous((0, 0), &[]), None);
        // Une fenêtre vide ne contient rien.
        assert_eq!(fenetre_sous((0, 0), &[fenetre(r(0, 0, 0, 10))]), None);
    }

    #[test]
    fn fenetre_coupee_a_l_ecran_du_point() {
        let e = ecrans();
        // À cheval entre l'écran de gauche et le principal.
        let cheval = r(-300, 100, 800, 500);
        assert_eq!(
            zone_fenetre(cheval, (10, 200), &e),
            Some(r(0, 100, 500, 500))
        );
        assert_eq!(
            zone_fenetre(cheval, (-10, 200), &e),
            Some(r(-300, 100, 300, 500))
        );
        // Dépasse en haut du principal, souris dans la partie du haut (écran à 125 %).
        let haute = r(100, -50, 400, 300);
        assert_eq!(
            zone_fenetre(haute, (200, -10), &e),
            Some(r(100, -50, 400, 50))
        );
        assert_eq!(
            zone_fenetre(haute, (200, 10), &e),
            Some(r(100, 0, 400, 250))
        );
        // Entièrement sur un écran : intacte.
        let dedans = r(2000, 100, 640, 480);
        assert_eq!(zone_fenetre(dedans, (2100, 200), &e), Some(dedans));
        // Point sur aucun écran (trou du bureau virtuel).
        assert_eq!(zone_fenetre(cheval, (100, 1500), &e), None);
        assert_eq!(zone_fenetre(cheval, (10, 200), &[]), None);
    }

    #[test]
    fn fenetre_trop_petite_apres_la_coupe() {
        let e = ecrans();
        // Il reste 2 pixels sur le principal.
        let bord = r(-500, 100, 502, 400);
        assert_eq!(zone_fenetre(bord, (1, 200), &e), None);
        assert_eq!(
            zone_fenetre(bord, (-1, 200), &e),
            Some(r(-500, 100, 500, 400))
        );
        // Exactement COTE_MIN : gardée.
        let juste = r(-500, 100, 503, 400);
        assert_eq!(zone_fenetre(juste, (2, 200), &e), Some(r(0, 100, 3, 400)));
        // Le point n'est pas sur la fenêtre : aucune intersection avec son écran.
        assert_eq!(zone_fenetre(r(-500, 0, 100, 100), (10, 10), &e), None);
    }

    #[test]
    fn rayon_selon_l_echelle_de_l_ecran() {
        let e = ecrans();
        // Centre sur chaque écran : 100 %, 125 %, 150 %, 200 %.
        let sur = |x, y| r(x, y, 400, 300);
        assert_eq!(rayon_coins(Coins::Arrondis, sur(100, 100), &e), 8);
        assert_eq!(rayon_coins(Coins::Arrondis, sur(100, -800), &e), 10);
        assert_eq!(rayon_coins(Coins::Arrondis, sur(-1000, 0), &e), 12);
        assert_eq!(rayon_coins(Coins::Arrondis, sur(2500, 500), &e), 16);
        assert_eq!(rayon_coins(Coins::PetitsArrondis, sur(100, 100), &e), 4);
        // 4 × 1,25 = 5 ; 4 × 1,5 = 6.
        assert_eq!(rayon_coins(Coins::PetitsArrondis, sur(100, -800), &e), 5);
        assert_eq!(rayon_coins(Coins::PetitsArrondis, sur(-1000, 0), &e), 6);
        assert_eq!(rayon_coins(Coins::PetitsArrondis, sur(2500, 500), &e), 8);
        assert_eq!(rayon_coins(Coins::Droits, sur(100, 100), &e), 0);
    }

    #[test]
    fn rayon_arrondi_au_pixel() {
        // 8 × 1,75 = 14 ; 8 × 1,1 = 8,8 → 9 ; 4 × 1,125 = 4,5 → 5 (au plus loin de zéro).
        let un = |echelle| vec![ecran(r(0, 0, 1920, 1080), echelle)];
        let f = r(10, 10, 400, 300);
        assert_eq!(rayon_coins(Coins::Arrondis, f, &un(1.75)), 14);
        assert_eq!(rayon_coins(Coins::Arrondis, f, &un(1.1)), 9);
        assert_eq!(rayon_coins(Coins::PetitsArrondis, f, &un(1.125)), 5);
        // Échelle absurde : 1.
        assert_eq!(rayon_coins(Coins::Arrondis, f, &un(f64::NAN)), 8);
        assert_eq!(rayon_coins(Coins::Arrondis, f, &un(0.0)), 8);
    }

    #[test]
    fn rayon_si_le_centre_est_hors_des_ecrans() {
        let e = ecrans();
        // Centre dans le trou sous le principal ; recouvre surtout le principal (100 %),
        // un peu l'écran de droite (200 %).
        let f = r(1000, 1000, 1000, 400);
        assert_eq!(rayon_coins(Coins::Arrondis, f, &e), 8);
        // Centre encore dans le trou ; recouvre surtout l'écran de droite.
        let f = r(1000, 1050, 1200, 1000);
        assert_eq!(rayon_coins(Coins::Arrondis, f, &e), 16);
        // Sur aucun écran : échelle 1.
        assert_eq!(rayon_coins(Coins::Arrondis, r(0, 5000, 100, 100), &e), 8);
        assert_eq!(rayon_coins(Coins::Arrondis, r(0, 0, 100, 100), &[]), 8);
    }

    #[test]
    fn rayon_nul_pour_un_ecran_entier_et_plafonne() {
        let e = ecrans();
        // Couvre tout le principal (agrandie sans le dire, plein écran) : coins droits.
        assert_eq!(rayon_coins(Coins::Arrondis, r(0, 0, 1920, 1080), &e), 0);
        assert_eq!(rayon_coins(Coins::Arrondis, r(-8, -8, 1936, 1096), &e), 0);
        assert_eq!(
            rayon_coins(Coins::Arrondis, r(-2560, -300, 2560, 1440), &e),
            0
        );
        // Plafond : la moitié du petit côté.
        assert_eq!(rayon_coins(Coins::Arrondis, r(2000, 100, 300, 20), &e), 10);
        assert_eq!(rayon_coins(Coins::Arrondis, r(100, 100, 5, 300), &e), 2);
        assert_eq!(rayon_coins(Coins::Arrondis, r(100, 100, 1, 1), &e), 0);
        assert_eq!(rayon_coins(Coins::Arrondis, r(100, 100, 0, 0), &e), 0);
    }

    #[test]
    fn rayon_sans_debordement() {
        let e = vec![ecran(r(i32::MAX - 100, i32::MAX - 100, 100, 100), 2.0)];
        let f = r(i32::MAX - 50, i32::MAX - 50, 50, 50);
        assert_eq!(rayon_coins(Coins::Arrondis, f, &e), 16);
        // Bords à -1 : ne recouvre que l'écran de gauche (150 %), sans le couvrir entier.
        let immense = r(i32::MIN, i32::MIN, i32::MAX, i32::MAX);
        assert_eq!(rayon_coins(Coins::Arrondis, immense, &ecrans()), 12);
    }

    #[test]
    fn seuls_les_vrais_coins_sont_arrondis() {
        let f = r(-300, 100, 800, 500);
        // Entière : les quatre.
        assert_eq!(coins_a_arrondir(f, f), [true; 4]);
        // Coupée au bord gauche du principal : seuls les coins de droite.
        assert_eq!(
            coins_a_arrondir(f, r(0, 100, 500, 500)),
            [false, true, false, true]
        );
        // Partie gauche : seuls les coins de gauche.
        assert_eq!(
            coins_a_arrondir(f, r(-300, 100, 300, 500)),
            [true, false, true, false]
        );
        // Coupée en haut (écran du dessus) : seuls les coins du bas.
        let haute = r(100, -50, 400, 300);
        assert_eq!(
            coins_a_arrondir(haute, r(100, 0, 400, 250)),
            [false, false, true, true]
        );
        // Coupée en haut et à droite : seul le coin bas gauche.
        let coin = r(1800, -50, 400, 300);
        assert_eq!(
            coins_a_arrondir(coin, r(1800, 0, 120, 250)),
            [false, false, true, false]
        );
        // Coupée des quatre côtés : aucun.
        assert_eq!(
            coins_a_arrondir(r(-10, -10, 2000, 2000), r(0, 0, 1920, 1080)),
            [false; 4]
        );
    }
}
