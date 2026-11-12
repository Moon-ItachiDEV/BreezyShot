//! Géométrie de la sélection, en fonctions pures (pixels réels du bureau virtuel).

use crate::systeme::gel::EcranInfo;
use crate::systeme::Rect;

/// En dessous de ce côté (en pixels réels), ce n'est pas une zone mais un clic.
pub const COTE_MIN: i32 = 3;

/// Le rectangle entre deux coins, donnés dans n'importe quel ordre. Les deux coins sont
/// inclus : de (0, 0) à (1919, 1079), la zone fait 1920 × 1080 ; un clic sans glisser
/// donne 1 × 1. Calcul en `i64`, borné aux `i32`.
pub fn rect_entre(a: (i32, i32), b: (i32, i32)) -> Rect {
    let cote = |u: i32, v: i32| ((u as i64 - v as i64).abs() + 1).min(i32::MAX as i64) as i32;
    Rect {
        x: a.0.min(b.0),
        y: a.1.min(b.1),
        largeur: cote(a.0, b.0),
        hauteur: cote(a.1, b.1),
    }
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

/// D'où vient l'écran choisi pour une capture plein écran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origine {
    /// L'écran qui contient la souris.
    Souris,
    /// Souris inconnue ou sur aucun écran : l'écran principal.
    Principal,
    /// Ni souris ni écran principal : le premier de la liste.
    Premier,
}

/// L'écran à capturer en plein écran : celui qui contient la souris ; sinon (souris
/// inconnue ou sur aucun écran) l'écran principal ; sinon le premier. `None` si la liste
/// est vide.
pub fn ecran_de_capture(
    souris: Option<(i32, i32)>,
    ecrans: &[EcranInfo],
) -> Option<(EcranInfo, Origine)> {
    if let Some(e) = souris.and_then(|p| ecran_sous(p, ecrans)) {
        return Some((*e, Origine::Souris));
    }
    if let Some(e) = ecrans.iter().find(|e| e.principal) {
        return Some((*e, Origine::Principal));
    }
    ecrans.first().map(|e| (*e, Origine::Premier))
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
        let attendu = r(10, 20, 31, 41);
        assert_eq!(rect_entre((10, 20), (40, 60)), attendu);
        assert_eq!(rect_entre((40, 60), (10, 20)), attendu);
        assert_eq!(rect_entre((10, 60), (40, 20)), attendu);
        assert_eq!(rect_entre((40, 20), (10, 60)), attendu);
        assert_eq!(rect_entre((-100, 5), (100, -5)), r(-100, -5, 201, 11));
    }

    #[test]
    fn clic_sans_glisser_n_est_pas_une_zone() {
        let clic = rect_entre((-5, -5), (-5, -5));
        assert_eq!(clic, r(-5, -5, 1, 1));
        assert!(!zone_valide(clic));
        assert!(!zone_valide(rect_entre((0, 0), (1, 1))));
        assert!(zone_valide(rect_entre((0, 0), (2, 2))));
    }

    #[test]
    fn coin_a_coin_d_un_ecran() {
        // Les quatre sens du glisser sur tout l'écran principal.
        let ecran = r(0, 0, 1920, 1080);
        assert_eq!(rect_entre((0, 0), (1919, 1079)), ecran);
        assert_eq!(rect_entre((1919, 1079), (0, 0)), ecran);
        assert_eq!(rect_entre((0, 1079), (1919, 0)), ecran);
        assert_eq!(rect_entre((1919, 0), (0, 1079)), ecran);
        // Écran à gauche, en coordonnées négatives.
        let gauche = r(-2560, -300, 2560, 1440);
        assert_eq!(rect_entre((-2560, -300), (-1, 1139)), gauche);
        assert_eq!(rect_entre((-1, 1139), (-2560, -300)), gauche);
        let e = trois_ecrans();
        assert_eq!(
            borner_a_un_ecran(rect_entre((-1, 1139), (-2560, -300)), (-1, 1139), &e),
            Some(gauche)
        );
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
            Some(r(0, 100, 101, 301))
        );
        // Même zone, glisser commencé à gauche : coupée à l'écran de gauche.
        assert_eq!(
            borner_a_un_ecran(zone, (-500, 400), &e),
            Some(r(-500, 100, 500, 301))
        );
        // Vers l'écran du dessus.
        let haut = rect_entre((50, 50), (300, -200));
        assert_eq!(
            borner_a_un_ecran(haut, (50, 50), &e),
            Some(r(50, 0, 251, 51))
        );
        assert_eq!(
            borner_a_un_ecran(haut, (300, -200), &e),
            Some(r(50, -200, 251, 200))
        );
        // Écran de gauche plus haut que le principal : la zone peut monter au-dessus de 0.
        let gauche = rect_entre((-10, -250), (-200, 1100));
        assert_eq!(
            borner_a_un_ecran(gauche, (-10, -250), &e),
            Some(r(-200, -250, 191, 1351))
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

#[cfg(test)]
mod tests_ecran_de_capture {
    use super::*;

    fn ecran(x: i32, y: i32, largeur: i32, hauteur: i32, principal: bool) -> EcranInfo {
        let rect = Rect {
            x,
            y,
            largeur,
            hauteur,
        };
        EcranInfo {
            rect,
            zone_utile: rect,
            echelle: 1.0,
            principal,
        }
    }

    /// Trois écrans côte à côte : à gauche (coordonnées négatives), le principal, à
    /// droite plus petit et décalé vers le bas (un trou sous le principal à droite).
    fn trois() -> Vec<EcranInfo> {
        vec![
            ecran(-2560, -300, 2560, 1440, false),
            ecran(0, 0, 1920, 1080, true),
            ecran(1920, 200, 1280, 1024, false),
        ]
    }

    #[test]
    fn souris_sur_le_deuxieme_de_trois() {
        let e = trois();
        assert_eq!(
            ecran_de_capture(Some((500, 500)), &e),
            Some((e[1], Origine::Souris))
        );
        assert_eq!(
            ecran_de_capture(Some((1920, 200)), &e),
            Some((e[2], Origine::Souris))
        );
    }

    #[test]
    fn souris_sur_un_ecran_a_gauche_en_coordonnees_negatives() {
        let e = trois();
        assert_eq!(
            ecran_de_capture(Some((-1, -300)), &e),
            Some((e[0], Origine::Souris))
        );
        assert_eq!(
            ecran_de_capture(Some((-2560, 1139)), &e),
            Some((e[0], Origine::Souris))
        );
    }

    #[test]
    fn souris_entre_deux_ecrans_donne_le_principal() {
        let e = trois();
        // Dans le trou au-dessus de l'écran de droite, et sous l'écran de gauche.
        assert_eq!(
            ecran_de_capture(Some((2000, 100)), &e),
            Some((e[1], Origine::Principal))
        );
        assert_eq!(
            ecran_de_capture(Some((-100, 1200)), &e),
            Some((e[1], Origine::Principal))
        );
    }

    #[test]
    fn souris_inconnue_donne_le_principal() {
        let e = trois();
        assert_eq!(ecran_de_capture(None, &e), Some((e[1], Origine::Principal)));
    }

    #[test]
    fn sans_principal_le_premier() {
        let e = vec![
            ecran(0, 0, 1920, 1080, false),
            ecran(1920, 0, 1920, 1080, false),
        ];
        assert_eq!(ecran_de_capture(None, &e), Some((e[0], Origine::Premier)));
        assert_eq!(
            ecran_de_capture(Some((5000, 5000)), &e),
            Some((e[0], Origine::Premier))
        );
        assert_eq!(
            ecran_de_capture(Some((1920, 0)), &e),
            Some((e[1], Origine::Souris))
        );
    }

    #[test]
    fn liste_vide() {
        assert_eq!(ecran_de_capture(Some((0, 0)), &[]), None);
        assert_eq!(ecran_de_capture(None, &[]), None);
    }
}
