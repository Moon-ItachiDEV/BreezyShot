//! Calculs purs du bandeau : où poser la fenêtre, et quand elle doit recevoir les clics.
//! Tout est en pixels réels d'écran, sauf ce que la page donne (pixels CSS).

/// Un rectangle en pixels réels d'écran (l'origine peut être négative : écran à gauche).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub largeur: u32,
    pub hauteur: u32,
}

impl Rect {
    /// Vrai si le point est dans le rectangle (bord droit et bas exclus).
    pub fn contient(&self, x: f64, y: f64) -> bool {
        let (x0, y0) = (f64::from(self.x), f64::from(self.y));
        x >= x0 && y >= y0 && x < x0 + f64::from(self.largeur) && y < y0 + f64::from(self.hauteur)
    }
}

/// Un écran tel que Tauri le décrit : son rectangle entier, sa zone utile (sans la barre des
/// tâches), son échelle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ecran {
    pub rect: Rect,
    pub zone_utile: Rect,
    pub echelle: f64,
}

/// Le rectangle de la pilule envoyé par la page (`bandeau_zone`), en pixels CSS, relatif à
/// la page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoneCss {
    pub x: f64,
    pub y: f64,
    pub largeur: f64,
    pub hauteur: f64,
}

/// Taille de la fenêtre du bandeau en pixels logiques (CSS) : pilule de 460 × 108 au plus,
/// posée à 64 px du haut, et la place de son ombre (environ 42 px).
pub const TAILLE_LOGIQUE: (f64, f64) = (560.0, 220.0);

/// Échelle utilisable : une valeur absurde vaut 1.
fn echelle_sure(echelle: f64) -> f64 {
    if echelle.is_finite() && echelle > 0.0 {
        echelle
    } else {
        1.0
    }
}

/// L'écran sous la souris ; à défaut le premier de la liste (l'écran principal en tête).
pub fn ecran_sous(souris: Option<(f64, f64)>, ecrans: &[Ecran]) -> Option<Ecran> {
    souris
        .and_then(|(x, y)| ecrans.iter().find(|e| e.rect.contient(x, y)))
        .or_else(|| ecrans.first())
        .copied()
}

/// Place du bandeau : collé en haut de la zone utile, centré horizontalement, taille
/// logique convertie avec l'échelle de cet écran.
pub fn placer(ecran: &Ecran, taille_logique: (f64, f64)) -> Rect {
    let e = echelle_sure(ecran.echelle);
    let largeur = (taille_logique.0 * e).round().max(1.0) as u32;
    let hauteur = (taille_logique.1 * e).round().max(1.0) as u32;
    let zone = ecran.zone_utile;
    let marge = (i64::from(zone.largeur) - i64::from(largeur)) / 2;
    Rect {
        x: (i64::from(zone.x) + marge) as i32,
        y: zone.y,
        largeur,
        hauteur,
    }
}

/// Le rectangle de la pilule en pixels réels d'écran.
pub fn zone_ecran(zone: &ZoneCss, echelle: f64, fenetre: (i32, i32)) -> Rect {
    let e = echelle_sure(echelle);
    Rect {
        x: fenetre.0 + (zone.x * e).round() as i32,
        y: fenetre.1 + (zone.y * e).round() as i32,
        largeur: (zone.largeur * e).round().max(0.0) as u32,
        hauteur: (zone.hauteur * e).round().max(0.0) as u32,
    }
}

/// La fenêtre doit-elle recevoir les clics ? Seulement pour un message à croix (info,
/// erreur), et seulement quand la souris est sur la pilule.
pub fn cliquable(
    a_croix: bool,
    zone: Option<&ZoneCss>,
    echelle: f64,
    fenetre: (i32, i32),
    souris: Option<(f64, f64)>,
) -> bool {
    let (Some(zone), Some((x, y)), true) = (zone, souris, a_croix) else {
        return false;
    };
    zone_ecran(zone, echelle, fenetre).contient(x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, largeur: u32, hauteur: u32) -> Rect {
        Rect {
            x,
            y,
            largeur,
            hauteur,
        }
    }

    fn ecran(r: Rect, utile: Rect, echelle: f64) -> Ecran {
        Ecran {
            rect: r,
            zone_utile: utile,
            echelle,
        }
    }

    #[test]
    fn place_a_100_pour_cent() {
        let e = ecran(rect(0, 0, 1920, 1080), rect(0, 0, 1920, 1032), 1.0);
        assert_eq!(placer(&e, TAILLE_LOGIQUE), rect(680, 0, 560, 220));
    }

    #[test]
    fn place_a_125_pour_cent() {
        let e = ecran(rect(0, 0, 2560, 1440), rect(0, 0, 2560, 1380), 1.25);
        assert_eq!(placer(&e, TAILLE_LOGIQUE), rect(930, 0, 700, 275));
    }

    #[test]
    fn place_a_150_pour_cent_barre_en_haut() {
        // Barre des tâches en haut : la zone utile commence plus bas.
        let e = ecran(rect(0, 0, 2880, 1800), rect(0, 72, 2880, 1728), 1.5);
        assert_eq!(placer(&e, TAILLE_LOGIQUE), rect(1020, 72, 840, 330));
    }

    #[test]
    fn place_sur_un_ecran_a_gauche() {
        // Écran secondaire à gauche du principal : coordonnées négatives.
        let e = ecran(
            rect(-1920, -200, 1920, 1080),
            rect(-1920, -200, 1920, 1040),
            1.0,
        );
        assert_eq!(placer(&e, TAILLE_LOGIQUE), rect(-1240, -200, 560, 220));
    }

    #[test]
    fn place_avec_une_echelle_absurde() {
        let e = ecran(rect(0, 0, 800, 600), rect(0, 0, 800, 600), f64::NAN);
        assert_eq!(placer(&e, TAILLE_LOGIQUE), rect(120, 0, 560, 220));
        let petit = ecran(rect(0, 0, 400, 300), rect(0, 0, 400, 300), 1.0);
        assert_eq!(placer(&petit, TAILLE_LOGIQUE).x, -80);
    }

    #[test]
    fn ecran_sous_la_souris() {
        let principal = ecran(rect(0, 0, 1920, 1080), rect(0, 0, 1920, 1040), 1.0);
        let gauche = ecran(rect(-2560, 0, 2560, 1440), rect(-2560, 0, 2560, 1380), 1.25);
        let ecrans = [principal, gauche];
        assert_eq!(ecran_sous(Some((-10.0, 50.0)), &ecrans), Some(gauche));
        assert_eq!(ecran_sous(Some((10.0, 50.0)), &ecrans), Some(principal));
        assert_eq!(ecran_sous(Some((0.0, 0.0)), &ecrans), Some(principal));
        // Hors de tout écran ou souris inconnue : le premier.
        assert_eq!(ecran_sous(Some((5000.0, 0.0)), &ecrans), Some(principal));
        assert_eq!(ecran_sous(None, &ecrans), Some(principal));
        assert_eq!(ecran_sous(None, &[]), None);
    }

    const PILULE: ZoneCss = ZoneCss {
        x: 50.0,
        y: 64.0,
        largeur: 460.0,
        hauteur: 60.0,
    };

    #[test]
    fn cliquable_sur_la_pilule_seulement() {
        let f = (680, 0);
        let z = Some(&PILULE);
        assert!(cliquable(true, z, 1.0, f, Some((730.0, 64.0))));
        assert!(cliquable(true, z, 1.0, f, Some((1189.0, 123.0))));
        assert!(!cliquable(true, z, 1.0, f, Some((1190.0, 100.0))));
        assert!(!cliquable(true, z, 1.0, f, Some((729.0, 100.0))));
        assert!(!cliquable(true, z, 1.0, f, Some((800.0, 63.0))));
        assert!(!cliquable(true, z, 1.0, f, Some((800.0, 124.0))));
    }

    #[test]
    fn cliquable_avec_echelle_et_ecran_a_gauche() {
        // 150 % : la pilule va de 75 à 765 en x, de 96 à 186 en y, depuis la fenêtre.
        let f = (-1240, -200);
        let z = Some(&PILULE);
        assert!(cliquable(true, z, 1.5, f, Some((-1165.0, -104.0))));
        assert!(cliquable(true, z, 1.5, f, Some((-476.0, -15.0))));
        assert!(!cliquable(true, z, 1.5, f, Some((-475.0, -50.0))));
        assert!(!cliquable(true, z, 1.5, f, Some((-1166.0, -50.0))));
        assert!(!cliquable(true, z, 1.5, f, Some((-800.0, -105.0))));
    }

    #[test]
    fn jamais_cliquable_sans_croix_sans_zone_ou_sans_souris() {
        let f = (0, 0);
        assert!(!cliquable(
            false,
            Some(&PILULE),
            1.0,
            f,
            Some((100.0, 80.0))
        ));
        assert!(!cliquable(true, None, 1.0, f, Some((100.0, 80.0))));
        assert!(!cliquable(true, Some(&PILULE), 1.0, f, None));
    }

    #[test]
    fn zone_en_pixels_reels() {
        assert_eq!(zone_ecran(&PILULE, 1.25, (930, 0)), rect(993, 80, 575, 75));
    }
}
