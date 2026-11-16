//! Les clics visibles du film (0.7.7, réglage « Clics visibles », Pro) : un anneau qui
//! s'ouvre et s'efface autour du pointeur à chaque clic, posé dans les pixels de chaque
//! image du film (vidéo comme GIF), comme le pointeur.
//!
//! Tout est pur et testé partout : la détection d'un clic d'après deux lectures de la
//! souris (`systeme::souris`, lue par l'Application), la vie d'un anneau, et son dessin.
//! Rien n'est écrit au journal ici.

use std::time::Duration;

use crate::systeme::gel::Pixels;
use crate::systeme::souris::Lecture;
use crate::systeme::Rect;

/// Combien de temps un anneau vit : il s'ouvre et s'efface en 450 ms.
pub const DUREE_ANNEAU: Duration = Duration::from_millis(450);
/// Le nombre d'anneaux vivants au plus (une rafale de clics ne coûte pas davantage).
const ANNEAUX_MAX: usize = 8;
/// Rayon de départ et d'arrivée de l'anneau, en pixels à 100 % d'échelle.
const RAYON_DEPART: f32 = 10.0;
const RAYON_FIN: f32 = 34.0;
/// Épaisseur du trait, en pixels à 100 % d'échelle.
const EPAISSEUR: f32 = 4.0;
/// La couleur du trait, et son opacité de départ ; le contour sombre aide sur un fond clair.
const COULEUR: [u8; 3] = [255, 196, 0];
const OPACITE: f32 = 0.92;
const OPACITE_CONTOUR: f32 = 0.35;

/// Un clic en cours d'affichage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Anneau {
    centre: (i32, i32),
    /// Le temps du film du clic.
    depart: Duration,
}

/// L'état des clics d'un film.
#[derive(Debug, Clone)]
pub struct Clics {
    echelle: f32,
    /// L'état des boutons à la lecture précédente ; `None` avant la première (elle amorce :
    /// un bouton déjà tenu au départ n'est pas un clic).
    enfonce: Option<bool>,
    anneaux: Vec<Anneau>,
}

/// Vrai si cette lecture est un clic : un bouton enfoncé alors qu'il ne l'était pas, ou un
/// appui rattrapé entre deux lectures. Jamais à la première lecture (`precedent` vide).
pub fn clic_detecte(precedent: Option<bool>, lecture: &Lecture) -> bool {
    match precedent {
        None => false,
        Some(avant) => (lecture.enfonce && !avant) || lecture.appuye,
    }
}

/// Où en est l'anneau né à `depart` à l'instant `maintenant` : de 0 (né) à 1 (éteint) ;
/// `None` s'il est né dans le futur ou éteint.
fn progression(depart: Duration, maintenant: Duration) -> Option<f32> {
    let age = maintenant.checked_sub(depart)?;
    (age < DUREE_ANNEAU).then(|| age.as_secs_f32() / DUREE_ANNEAU.as_secs_f32())
}

impl Clics {
    /// `echelle` : l'échelle d'affichage de l'écran du film (1.0 = 100 %).
    pub fn nouveau(echelle: f64) -> Clics {
        let echelle = if echelle.is_finite() && echelle > 0.0 {
            echelle.clamp(0.5, 4.0) as f32
        } else {
            1.0
        };
        Clics {
            echelle,
            enfonce: None,
            anneaux: Vec::new(),
        }
    }

    /// Une lecture de la souris à l'instant `instant` du film : un clic dans `zone` ouvre un
    /// anneau ; un clic hors de la zone n'est pas montré. Sans lecture (`None`), rien ne
    /// change.
    pub fn lire(&mut self, lecture: Option<Lecture>, zone: Rect, instant: Duration) {
        let Some(l) = lecture else {
            return;
        };
        let clic = clic_detecte(self.enfonce, &l);
        self.enfonce = Some(l.enfonce);
        if !clic || !dans(zone, l.position) {
            return;
        }
        if self.anneaux.len() >= ANNEAUX_MAX {
            self.anneaux.remove(0);
        }
        self.anneaux.push(Anneau {
            centre: l.position,
            depart: instant,
        });
    }

    /// Retire les anneaux éteints à `instant`. Vrai si l'image de cet instant doit être
    /// refaite : un anneau est encore là, ou vient de s'éteindre (l'image doit le perdre).
    pub fn change(&mut self, instant: Duration) -> bool {
        let avant = self.anneaux.len();
        self.anneaux
            .retain(|a| progression(a.depart, instant).is_some());
        avant > 0
    }

    /// Pose les anneaux vivants à `instant` dans `pixels`, qui sont ceux de `zone` (pixels
    /// réels du bureau, même largeur et même hauteur). Des tailles incohérentes ne touchent
    /// à rien. Rend le nombre d'anneaux posés.
    pub fn poser(&self, pixels: &mut Pixels, zone: Rect, instant: Duration) -> usize {
        if zone.est_vide()
            || pixels.largeur != zone.largeur as u32
            || pixels.hauteur != zone.hauteur as u32
            || pixels.rgba.len() < pixels.largeur as usize * pixels.hauteur as usize * 4
        {
            return 0;
        }
        let mut poses = 0;
        for a in &self.anneaux {
            if let Some(p) = progression(a.depart, instant) {
                let centre = (
                    (a.centre.0 as i64 - zone.x as i64) as f32,
                    (a.centre.1 as i64 - zone.y as i64) as f32,
                );
                dessiner(pixels, centre, p, self.echelle);
                poses += 1;
            }
        }
        poses
    }

    /// Le nombre d'anneaux gardés (essais).
    #[cfg(test)]
    fn vivants(&self) -> usize {
        self.anneaux.len()
    }
}

/// Vrai si `p` est dans `zone` (bord haut et gauche compris, bas et droit exclus).
fn dans(zone: Rect, p: (i32, i32)) -> bool {
    let (x, y) = (i64::from(p.0), i64::from(p.1));
    x >= i64::from(zone.x) && y >= i64::from(zone.y) && x < zone.droite() && y < zone.bas()
}

/// Le rayon de l'anneau à la progression `p` : il s'ouvre vite, puis ralentit.
fn rayon(p: f32, echelle: f32) -> f32 {
    let ouverture = 1.0 - (1.0 - p) * (1.0 - p);
    (RAYON_DEPART + (RAYON_FIN - RAYON_DEPART) * ouverture) * echelle
}

/// L'opacité de l'anneau à la progression `p` : pleine à la naissance, nulle à la fin.
fn opacite(p: f32) -> f32 {
    (1.0 - p).clamp(0.0, 1.0)
}

/// Mêle `rgb` à `pixel` avec l'opacité `a` (0 à 1) ; l'alpha du pixel n'est pas touché.
fn meler(pixel: &mut [u8], rgb: [u8; 3], a: f32) {
    if a <= 0.0 {
        return;
    }
    for k in 0..3 {
        let v = rgb[k] as f32 * a + pixel[k] as f32 * (1.0 - a);
        pixel[k] = v.round().clamp(0.0, 255.0) as u8;
    }
}

/// Dessine un anneau de centre `centre` (pixels de l'image) à la progression `p`.
fn dessiner(pixels: &mut Pixels, centre: (f32, f32), p: f32, echelle: f32) {
    let r = rayon(p, echelle);
    let demi = EPAISSEUR * echelle / 2.0;
    let contour = demi + 1.5 * echelle;
    let reach = r + contour + 1.0;
    let (l, h) = (pixels.largeur as i64, pixels.hauteur as i64);
    let x0 = ((centre.0 - reach).floor() as i64).max(0);
    let x1 = ((centre.0 + reach).ceil() as i64).min(l - 1);
    let y0 = ((centre.1 - reach).floor() as i64).max(0);
    let y1 = ((centre.1 + reach).ceil() as i64).min(h - 1);
    let alpha = opacite(p);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f32 + 0.5 - centre.0;
            let dy = y as f32 + 0.5 - centre.1;
            let ecart = ((dx * dx + dy * dy).sqrt() - r).abs();
            let couverture = |demi: f32| (demi + 0.5 - ecart).clamp(0.0, 1.0);
            let sombre = couverture(contour);
            let clair = couverture(demi);
            if sombre <= 0.0 {
                continue;
            }
            let i = (y as usize * l as usize + x as usize) * 4;
            let pixel = &mut pixels.rgba[i..i + 4];
            meler(pixel, [0, 0, 0], OPACITE_CONTOUR * alpha * sombre);
            meler(pixel, COULEUR, OPACITE * alpha * clair);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn lecture(x: i32, y: i32, enfonce: bool, appuye: bool) -> Option<Lecture> {
        Some(Lecture {
            position: (x, y),
            enfonce,
            appuye,
        })
    }

    fn zone() -> Rect {
        Rect {
            x: 100,
            y: 50,
            largeur: 200,
            hauteur: 100,
        }
    }

    fn image(l: u32, h: u32, fond: u8) -> Pixels {
        Pixels {
            largeur: l,
            hauteur: h,
            rgba: vec![fond; l as usize * h as usize * 4],
        }
    }

    #[test]
    fn detection_du_clic() {
        let l = |enfonce, appuye| Lecture {
            position: (0, 0),
            enfonce,
            appuye,
        };
        // La première lecture amorce : jamais un clic.
        assert!(!clic_detecte(None, &l(true, true)));
        // Enfoncé alors qu'il ne l'était pas : un clic ; tenu : plus.
        assert!(clic_detecte(Some(false), &l(true, false)));
        assert!(!clic_detecte(Some(true), &l(true, false)));
        // Relâché : pas un clic ; un appui rattrapé entre deux lectures : un clic.
        assert!(!clic_detecte(Some(true), &l(false, false)));
        assert!(clic_detecte(Some(false), &l(false, true)));
    }

    #[test]
    fn un_clic_dans_la_zone_ouvre_un_anneau_hors_zone_rien() {
        let mut c = Clics::nouveau(1.0);
        c.lire(lecture(150, 80, false, false), zone(), ms(0));
        c.lire(lecture(150, 80, true, false), zone(), ms(33));
        assert_eq!(c.vivants(), 1);
        // Hors de la zone : pas montré.
        c.lire(lecture(10, 10, false, false), zone(), ms(66));
        c.lire(lecture(10, 10, true, false), zone(), ms(99));
        assert_eq!(c.vivants(), 1);
        // Pas de lecture : rien ne change.
        c.lire(None, zone(), ms(120));
        assert_eq!(c.vivants(), 1);
        // Bord bas et droit exclus.
        let mut c = Clics::nouveau(1.0);
        c.lire(lecture(300, 150, false, false), zone(), ms(0));
        c.lire(lecture(300, 150, true, false), zone(), ms(33));
        assert_eq!(c.vivants(), 0);
    }

    #[test]
    fn bouton_tenu_au_depart_n_est_pas_un_clic() {
        let mut c = Clics::nouveau(1.0);
        c.lire(lecture(150, 80, true, true), zone(), ms(0));
        c.lire(lecture(150, 80, true, false), zone(), ms(33));
        assert_eq!(c.vivants(), 0);
    }

    #[test]
    fn la_vie_d_un_anneau() {
        let mut c = Clics::nouveau(1.0);
        c.lire(lecture(150, 80, false, false), zone(), ms(0));
        c.lire(lecture(150, 80, true, false), zone(), ms(100));
        // Pendant sa vie, l'image change ; à son extinction, une dernière image le perd.
        assert!(c.change(ms(100)));
        assert!(c.change(ms(300)));
        assert!(c.change(ms(549)));
        assert!(c.change(ms(550)));
        assert_eq!(c.vivants(), 0);
        // Ensuite, plus rien : les images inchangées restent inchangées.
        assert!(!c.change(ms(583)));
    }

    #[test]
    fn une_rafale_ne_garde_que_les_plus_recents() {
        let mut c = Clics::nouveau(1.0);
        c.lire(lecture(150, 80, false, false), zone(), ms(0));
        for i in 0..20u64 {
            c.lire(lecture(150, 80, true, false), zone(), ms(i));
            c.lire(lecture(150, 80, false, false), zone(), ms(i));
        }
        assert_eq!(c.vivants(), ANNEAUX_MAX);
    }

    #[test]
    fn rayon_et_opacite_de_l_anneau() {
        assert_eq!(rayon(0.0, 1.0), RAYON_DEPART);
        assert!((rayon(1.0, 1.0) - RAYON_FIN).abs() < 1e-4);
        assert!(rayon(0.5, 1.0) > (RAYON_DEPART + RAYON_FIN) / 2.0);
        assert_eq!(rayon(0.0, 2.0), RAYON_DEPART * 2.0);
        assert_eq!(opacite(0.0), 1.0);
        assert_eq!(opacite(1.0), 0.0);
    }

    #[test]
    fn la_pose_dessine_un_anneau_sans_toucher_l_alpha() {
        let mut c = Clics::nouveau(1.0);
        let z = Rect {
            x: 0,
            y: 0,
            largeur: 100,
            hauteur: 100,
        };
        c.lire(lecture(50, 50, false, false), z, ms(0));
        c.lire(lecture(50, 50, true, false), z, ms(0));
        let mut im = image(100, 100, 40);
        for p in im.rgba.chunks_exact_mut(4) {
            p[3] = 200;
        }
        let avant = im.rgba.clone();
        assert_eq!(c.poser(&mut im, z, ms(0)), 1);
        // Le trait est à 10 px du centre (rayon de départ) : jaune ; le centre ne bouge pas.
        let i = (50 * 100 + 60) * 4;
        assert!(
            im.rgba[i] > 150 && im.rgba[i + 2] < 100,
            "{:?}",
            &im.rgba[i..i + 4]
        );
        let centre = (50 * 100 + 50) * 4;
        assert_eq!(im.rgba[centre..centre + 4], avant[centre..centre + 4]);
        assert!(im.rgba.chunks_exact(4).all(|p| p[3] == 200));
        // Éteint : plus rien n'est posé.
        let mut im2 = image(100, 100, 40);
        assert_eq!(c.poser(&mut im2, z, ms(450)), 0);
        assert_eq!(im2.rgba, image(100, 100, 40).rgba);
    }

    #[test]
    fn la_pose_reste_dans_l_image_meme_au_bord() {
        let mut c = Clics::nouveau(2.0);
        let z = Rect {
            x: 0,
            y: 0,
            largeur: 30,
            hauteur: 20,
        };
        c.lire(lecture(0, 0, false, false), z, ms(0));
        c.lire(lecture(0, 0, true, false), z, ms(0));
        let mut im = image(30, 20, 0);
        // Un anneau au coin, plus grand que l'image : aucune panique ni débordement.
        assert_eq!(c.poser(&mut im, z, ms(200)), 1);
        assert_eq!(im.rgba.len(), 30 * 20 * 4);
    }

    #[test]
    fn pose_refusee_si_les_tailles_different() {
        let mut c = Clics::nouveau(1.0);
        c.lire(lecture(150, 80, false, false), zone(), ms(0));
        c.lire(lecture(150, 80, true, false), zone(), ms(0));
        let mut im = image(10, 10, 0);
        assert_eq!(c.poser(&mut im, zone(), ms(0)), 0);
        assert_eq!(im.rgba, image(10, 10, 0).rgba);
    }

    #[test]
    fn echelle_invalide_ramenee_a_un() {
        assert_eq!(Clics::nouveau(f64::NAN).echelle, 1.0);
        assert_eq!(Clics::nouveau(-2.0).echelle, 1.0);
        assert_eq!(Clics::nouveau(1.5).echelle, 1.5);
        assert_eq!(Clics::nouveau(100.0).echelle, 4.0);
    }
}
