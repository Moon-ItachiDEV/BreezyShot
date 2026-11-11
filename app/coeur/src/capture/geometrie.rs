//! Géométrie de la sélection, en fonctions pures (pixels réels du bureau virtuel).

use crate::systeme::gel::EcranInfo;
use crate::systeme::Rect;

/// En dessous de ce côté (en pixels réels), ce n'est pas une zone mais un clic.
pub const COTE_MIN: i32 = 3;

/// Le rectangle entre deux coins, donnés dans n'importe quel ordre. Le coin d'arrivée est
/// exclu : de (10, 10) à (20, 15), la zone fait 10 × 5.
pub fn rect_entre(a: (i32, i32), b: (i32, i32)) -> Rect {
    Rect::depuis_bords(a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1))
}

/// L'écran qui contient le point ; `None` si le point n'est sur aucun écran.
pub fn ecran_sous(point: (i32, i32), ecrans: &[EcranInfo]) -> Option<&EcranInfo> {
    ecrans.iter().find(|e| e.rect.contient(point))
}

/// Une zone reste sur un seul écran : celui où le glisser a commencé (`depart`).
/// `None` si le départ n'est sur aucun écran, ou si la zone ne touche pas cet écran.
pub fn borner_a_un_ecran(zone: Rect, depart: (i32, i32), ecrans: &[EcranInfo]) -> Option<Rect> {
    let ecran = ecran_sous(depart, ecrans)?;
    zone.intersection(ecran.rect)
}

/// Vrai si la zone est assez grande pour être capturée.
pub fn zone_valide(zone: Rect) -> bool {
    zone.largeur >= COTE_MIN && zone.hauteur >= COTE_MIN
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

    fn ecran(rect: Rect, echelle: f64, principal: bool) -> EcranInfo {
        EcranInfo {
            rect,
            zone_utile: Rect {
                hauteur: rect.hauteur - 48,
                ..rect
            },
            echelle,
            principal,
        }
    }

    /// Trois écrans : un 1440p à 150 % à gauche (plus haut que le principal), le principal
    /// 1080p à 100 %, un 1080p à 125 % au-dessus du principal.
    fn trois_ecrans() -> Vec<EcranInfo> {
        vec![
            ecran(r(-2560, -300, 2560, 1440), 1.5, false),
            ecran(r(0, -1080, 1920, 1080), 1.25, false),
            ecran(r(0, 0, 1920, 1080), 1.0, true),
        ]
    }

    #[test]
    fn deux_coins_dans_tous_les_ordres() {
        let attendu = r(10, 20, 30, 40);
        assert_eq!(rect_entre((10, 20), (40, 60)), attendu);
        assert_eq!(rect_entre((40, 60), (10, 20)), attendu);
        assert_eq!(rect_entre((10, 60), (40, 20)), attendu);
        assert_eq!(rect_entre((40, 20), (10, 60)), attendu);
        assert_eq!(rect_entre((-5, -5), (-5, -5)), r(-5, -5, 0, 0));
        assert_eq!(rect_entre((-100, 5), (100, -5)), r(-100, -5, 200, 10));
    }

    #[test]
    fn coins_extremes_sans_panique() {
        let z = rect_entre((i32::MIN, i32::MIN), (i32::MAX, i32::MAX));
        assert_eq!(z.x, i32::MIN);
        assert_eq!(z.largeur, i32::MAX);
    }

    #[test]
    fn ecran_sous_un_point() {
        let e = trois_ecrans();
        assert_eq!(ecran_sous((-1, 0), &e).map(|e| e.echelle), Some(1.5));
        assert_eq!(ecran_sous((-2560, -300), &e).map(|e| e.echelle), Some(1.5));
        assert_eq!(ecran_sous((0, 0), &e).map(|e| e.principal), Some(true));
        assert_eq!(ecran_sous((1919, -1), &e).map(|e| e.echelle), Some(1.25));
        // Bords droit et bas exclus ; trous du bureau virtuel.
        assert!(ecran_sous((1920, 0), &e).is_none());
        assert!(ecran_sous((-10, 1140), &e).is_none());
        assert!(ecran_sous((100, 2000), &e).is_none());
        assert!(ecran_sous((0, 0), &[]).is_none());
    }

    #[test]
    fn zone_bornee_a_l_ecran_de_depart() {
        let e = trois_ecrans();
        // Glisser du principal vers l'écran de gauche : coupé au principal.
        let zone = rect_entre((100, 100), (-500, 400));
        assert_eq!(
            borner_a_un_ecran(zone, (100, 100), &e),
            Some(r(0, 100, 100, 300))
        );
        // Même zone, glisser commencé à gauche : coupée à l'écran de gauche.
        assert_eq!(
            borner_a_un_ecran(zone, (-500, 400), &e),
            Some(r(-500, 100, 500, 300))
        );
        // Vers l'écran du dessus.
        let haut = rect_entre((50, 50), (300, -200));
        assert_eq!(
            borner_a_un_ecran(haut, (50, 50), &e),
            Some(r(50, 0, 250, 50))
        );
        assert_eq!(
            borner_a_un_ecran(haut, (300, -200), &e),
            Some(r(50, -200, 250, 200))
        );
        // Écran de gauche plus haut que le principal : la zone peut monter au-dessus de 0.
        let gauche = rect_entre((-10, -250), (-200, 1100));
        assert_eq!(
            borner_a_un_ecran(gauche, (-10, -250), &e),
            Some(r(-200, -250, 190, 1350))
        );
    }

    #[test]
    fn depart_hors_de_tout_ecran() {
        let e = trois_ecrans();
        let zone = r(-100, 1100, 300, 300);
        assert_eq!(borner_a_un_ecran(zone, (-100, 1150), &e), None);
        assert_eq!(borner_a_un_ecran(zone, (5, 5), &[]), None);
    }

    #[test]
    fn un_seul_ecran() {
        let e = vec![ecran(r(0, 0, 1366, 768), 1.0, true)];
        assert_eq!(
            borner_a_un_ecran(r(1000, 700, 1000, 1000), (1000, 700), &e),
            Some(r(1000, 700, 366, 68))
        );
    }

    #[test]
    fn taille_minimale() {
        assert!(zone_valide(r(0, 0, COTE_MIN, COTE_MIN)));
        assert!(!zone_valide(r(0, 0, COTE_MIN - 1, 100)));
        assert!(!zone_valide(r(0, 0, 100, 0)));
        assert!(!zone_valide(r(0, 0, -5, 100)));
    }
}
