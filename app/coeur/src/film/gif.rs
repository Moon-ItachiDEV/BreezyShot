//! L'écriture du GIF au fil de l'eau (F19), par la crate `gif` : GIF89a, boucle infinie ;
//! la première image entière et opaque ; ensuite le seul rectangle changé, ses pixels
//! inchangés transparents, une palette locale (`Frame::from_rgba_speed`, vitesse 10), et
//! l'image précédente gardée dessous (`DisposalMethod::Keep`). Une image identique à la
//! précédente n'est pas écrite : elle allonge le délai de la précédente. Délais en
//! centièmes de seconde, sur les temps réels, avec report de l'arrondi. Jamais tout le GIF
//! en mémoire : ce qui est gardé est borné par la taille de l'image (deux images RGBA, la
//! sous-image et l'image en attente de son délai), quelle que soit la durée.

use std::borrow::Cow;
use std::fmt;
use std::io::Write;
use std::time::Duration;

use super::difference::{rectangle_change, sous_image_dans};
use crate::systeme::gel::{Pixels, Rect};

/// Le plus long délai d'une image GIF (16 bits), en centièmes de seconde.
const DELAI_MAX: u64 = u16::MAX as u64;

/// La sortie vue par le codeur : la première erreur d'écriture est gardée, et tout ce qui
/// suit est avalé. Le codeur de la crate écrit la fin du fichier quand il est détruit et
/// panique si elle échoue (`Drop`, sans la fonctionnalité `raii_no_panic`) : un disque
/// plein ne doit jamais faire paniquer le fil `film-gif`.
struct Garde<W: Write> {
    sortie: W,
    erreur: Option<std::io::Error>,
}

impl<W: Write> Write for Garde<W> {
    fn write(&mut self, octets: &[u8]) -> std::io::Result<usize> {
        if self.erreur.is_none() {
            match self.sortie.write(octets) {
                Ok(n) => return Ok(n),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => return Err(e),
                Err(e) => self.erreur = Some(e),
            }
        }
        Ok(octets.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.erreur.is_none() {
            if let Err(e) = self.sortie.flush() {
                self.erreur = Some(e);
            }
        }
        Ok(())
    }
}

/// Un GIF en cours d'écriture dans `W` (le fichier de travail, dans le fil `film-gif`).
pub struct Gif<W: Write> {
    codeur: ::gif::Encoder<Garde<W>>,
    largeur: u16,
    hauteur: u16,
    vitesse: i32,
    /// L'image écrite en dernier (ce que montre le GIF), alpha à 255 ; vide au début.
    precedente: Pixels,
    /// L'image reçue, alpha à 255 (tampon réutilisé).
    courante: Pixels,
    /// La sous-image du rectangle changé (tampon réutilisé).
    sous: Vec<u8>,
    /// L'alpha de chaque pixel de la sous-image (tampon réutilisé).
    alpha: Vec<u8>,
    /// L'image quantifiée qui attend son délai (il dépend de l'image suivante), et son temps.
    attente: Option<(::gif::Frame<'static>, Duration)>,
    /// Images écrites dans le fichier.
    ecrites: u64,
}

impl<W: Write> fmt::Debug for Gif<W> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Gif")
            .field("largeur", &self.largeur)
            .field("hauteur", &self.hauteur)
            .field("vitesse", &self.vitesse)
            .field("ecrites", &self.ecrites)
            .field("en_attente", &self.attente.is_some())
            .finish()
    }
}

fn faute(quoi: &str, e: impl fmt::Display) -> String {
    format!("gif {quoi}: {e}")
}

/// Un temps en centièmes de seconde, arrondi au plus proche.
fn centiemes(t: Duration) -> u64 {
    let cs = (t.as_nanos() + 5_000_000) / 10_000_000;
    u64::try_from(cs).unwrap_or(u64::MAX)
}

impl<W: Write> Gif<W> {
    /// Commence un GIF de `largeur` × `hauteur` pixels dans `sortie` ; `vitesse` est celle
    /// de la quantification de la crate (10, D13), de 1 à 30.
    pub fn nouveau(sortie: W, largeur: u16, hauteur: u16, vitesse: i32) -> Result<Gif<W>, String> {
        if largeur == 0 || hauteur == 0 {
            return Err(format!("gif: taille refusee {largeur}x{hauteur}"));
        }
        if !(1..=30).contains(&vitesse) {
            return Err(format!("gif: vitesse refusee {vitesse}"));
        }
        let garde = Garde {
            sortie,
            erreur: None,
        };
        let mut codeur =
            ::gif::Encoder::new(garde, largeur, hauteur, &[]).map_err(|e| faute("debut", e))?;
        codeur
            .set_repeat(::gif::Repeat::Infinite)
            .map_err(|e| faute("boucle", e))?;
        if let Some(e) = codeur.get_mut().erreur.take() {
            return Err(faute("debut", e));
        }
        let vide = || Pixels {
            largeur: 0,
            hauteur: 0,
            rgba: Vec::new(),
        };
        Ok(Gif {
            codeur,
            largeur,
            hauteur,
            vitesse,
            precedente: vide(),
            courante: vide(),
            sous: Vec::new(),
            alpha: Vec::new(),
            attente: None,
            ecrites: 0,
        })
    }

    /// L'image du temps `instant` du film. Faux : image identique à la précédente, rien
    /// d'écrit (F19). Une image d'une autre taille que le GIF est refusée.
    pub fn image(&mut self, pixels: &Pixels, instant: Duration) -> Result<bool, String> {
        let (l, h) = (usize::from(self.largeur), usize::from(self.hauteur));
        if pixels.largeur as usize != l
            || pixels.hauteur as usize != h
            || pixels.rgba.len() != l * h * 4
        {
            return Err(format!(
                "gif: image {}x{} pour un gif {l}x{h}",
                pixels.largeur, pixels.hauteur
            ));
        }
        // L'image reçue, toujours opaque : seul le GIF décide de ce qui est transparent.
        self.courante.largeur = pixels.largeur;
        self.courante.hauteur = pixels.hauteur;
        self.courante.rgba.clear();
        self.courante.rgba.extend_from_slice(&pixels.rgba);
        for p in self.courante.rgba.chunks_exact_mut(4) {
            p[3] = 255;
        }
        let r = if self.precedente.rgba.is_empty() {
            // La première image : entière et opaque.
            self.sous.clear();
            self.sous.extend_from_slice(&self.courante.rgba);
            Rect {
                x: 0,
                y: 0,
                largeur: l as i32,
                hauteur: h as i32,
            }
        } else {
            let Some(r) = rectangle_change(&self.precedente, &self.courante) else {
                return Ok(false);
            };
            sous_image_dans(&self.precedente, &self.courante, r, &mut self.sous);
            r
        };
        let mut image = self.quantifier(r.largeur as u16, r.hauteur as u16);
        image.left = r.x as u16;
        image.top = r.y as u16;
        image.dispose = ::gif::DisposalMethod::Keep;
        if let Some((avant, depuis)) = self.attente.take() {
            self.ecrire(avant, depuis, instant)?;
        }
        self.attente = Some((image, instant));
        std::mem::swap(&mut self.precedente, &mut self.courante);
        Ok(true)
    }

    /// Écrit l'image en attente avec le délai qui mène à `duree`, puis la fin du GIF ; rend
    /// la sortie. Sans aucune image : refusé.
    pub fn finir(mut self, duree: Duration) -> Result<W, String> {
        match self.attente.take() {
            Some((image, depuis)) => self.ecrire(image, depuis, duree)?,
            None if self.ecrites == 0 => return Err("gif: aucune image".into()),
            None => {}
        }
        let garde = self.codeur.into_inner().map_err(|e| faute("fin", e))?;
        match garde.erreur {
            Some(e) => Err(faute("ecriture", e)),
            None => Ok(garde.sortie),
        }
    }

    /// La première erreur d'écriture, rendue une fois (les écritures suivantes sont
    /// avalées par la garde).
    fn verifier(&mut self) -> Result<(), String> {
        match self.codeur.get_mut().erreur.take() {
            Some(e) => {
                // Gardée pour `finir`, qui la rend aussi.
                let texte = faute("ecriture", &e);
                self.codeur.get_mut().erreur = Some(e);
                Err(texte)
            }
            None => Ok(()),
        }
    }

    /// Images écrites dans le fichier jusqu'ici (sans celle qui attend son délai).
    pub fn ecrites(&self) -> u64 {
        self.ecrites
    }

    /// La sous-image (`self.sous`) en image GIF à palette locale, aucun pixel opaque sur
    /// l'indice transparent (F19).
    fn quantifier(&mut self, largeur: u16, hauteur: u16) -> ::gif::Frame<'static> {
        self.alpha.clear();
        self.alpha.extend(
            self.sous
                .chunks_exact(4)
                .map(|p| if p[3] == 0 { 0 } else { 255 }),
        );
        let mut image =
            ::gif::Frame::from_rgba_speed(largeur, hauteur, &mut self.sous, self.vitesse);
        if let (Some(t), Some(palette)) = (image.transparent, image.palette.as_deref()) {
            let palette = palette.to_vec();
            opaques_hors_du_transparent(image.buffer.to_mut(), &self.alpha, &palette, t);
        }
        image
    }

    /// Écrit `image` avec le délai de `depuis` à `jusqu_a`. Un délai qui ne tient pas dans
    /// 16 bits (plus de 655,35 s, ce que la limite de 60 s exclut) est continué par des
    /// images d'un pixel transparent.
    fn ecrire(
        &mut self,
        mut image: ::gif::Frame<'static>,
        depuis: Duration,
        jusqu_a: Duration,
    ) -> Result<(), String> {
        let mut reste = centiemes(jusqu_a).saturating_sub(centiemes(depuis));
        let premier = reste.min(DELAI_MAX);
        image.delay = premier as u16;
        reste -= premier;
        self.codeur
            .write_frame(&image)
            .map_err(|e| faute("image", e))?;
        self.verifier()?;
        self.ecrites += 1;
        while reste > 0 {
            let d = reste.min(DELAI_MAX);
            reste -= d;
            let suite = ::gif::Frame {
                width: 1,
                height: 1,
                buffer: Cow::Owned(vec![0]),
                palette: Some(vec![0, 0, 0]),
                transparent: Some(0),
                dispose: ::gif::DisposalMethod::Keep,
                delay: d as u16,
                ..::gif::Frame::default()
            };
            self.codeur
                .write_frame(&suite)
                .map_err(|e| faute("image", e))?;
            self.verifier()?;
            self.ecrites += 1;
        }
        Ok(())
    }

    /// Pour les tests : les octets gardés par le GIF (capacités de ses tampons).
    #[cfg(test)]
    fn octets_gardes(&self) -> usize {
        let attente = self.attente.as_ref().map_or(0, |(f, _)| {
            f.buffer.len() + f.palette.as_ref().map_or(0, Vec::capacity)
        });
        self.precedente.rgba.capacity()
            + self.courante.rgba.capacity()
            + self.sous.capacity()
            + self.alpha.capacity()
            + attente
    }
}

/// Le délai d'une image, en centièmes de seconde, de `depuis` à `jusqu_a` (temps du film),
/// avec report de l'arrondi : les deux temps sont arrondis au centième, puis soustraits,
/// si bien que la somme des délais égale la durée à un centième près. Borné à 16 bits ;
/// 0 si `jusqu_a` précède `depuis`.
pub fn delai_cs(depuis: Duration, jusqu_a: Duration) -> u16 {
    centiemes(jusqu_a)
        .saturating_sub(centiemes(depuis))
        .min(DELAI_MAX) as u16
}

/// F19 : aucun pixel opaque à l'indice `transparent` ; un pixel opaque (alpha non nul dans
/// `alpha`, un octet par pixel) qui l'a reçu est ramené à l'indice opaque le plus proche de
/// `palette` (RGB) : celui dont la couleur est la plus proche de celle de l'indice
/// transparent, que la quantification avait jugée la plus proche du pixel. Une palette qui
/// n'a pas d'autre indice ne change rien.
pub fn opaques_hors_du_transparent(
    indices: &mut [u8],
    alpha: &[u8],
    palette: &[u8],
    transparent: u8,
) {
    let fautifs = indices
        .iter()
        .zip(alpha)
        .any(|(&i, &a)| i == transparent && a != 0);
    if !fautifs {
        return;
    }
    let couleur = |k: usize| palette.get(k * 3..k * 3 + 3);
    let Some(t) = couleur(usize::from(transparent)) else {
        return;
    };
    let mut meilleur: Option<(u32, u8)> = None;
    for k in 0..(palette.len() / 3).min(256) {
        if k == usize::from(transparent) {
            continue;
        }
        let Some(c) = couleur(k) else { continue };
        let d: u32 = c
            .iter()
            .zip(t)
            .map(|(&x, &y)| {
                let e = i32::from(x) - i32::from(y);
                (e * e) as u32
            })
            .sum();
        if meilleur.is_none_or(|(m, _)| d < m) {
            meilleur = Some((d, k as u8));
        }
    }
    let Some((_, remplacant)) = meilleur else {
        return;
    };
    for (i, &a) in indices.iter_mut().zip(alpha) {
        if *i == transparent && a != 0 {
            *i = remplacant;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::time::Instant;

    const L: u32 = 600;
    const H: u32 = 450;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// Le temps de l'image `n` à 15 images par seconde.
    fn t15(n: u64) -> Duration {
        Duration::from_nanos(n * 1_000_000_000 / 15)
    }

    fn uni(l: u32, h: u32, c: [u8; 4]) -> Pixels {
        Pixels {
            largeur: l,
            hauteur: h,
            rgba: c.repeat((l * h) as usize),
        }
    }

    /// Un fond fixe en dégradé (peu de couleurs) et un compteur : un carré dont la couleur
    /// change avec `k` (le témoin « simple » de la sonde).
    fn simple(l: u32, h: u32, k: u32) -> Pixels {
        let mut p = Pixels {
            largeur: l,
            hauteur: h,
            rgba: Vec::with_capacity((l * h * 4) as usize),
        };
        for y in 0..h {
            for x in 0..l {
                let bande = ((x / 40) * 16) as u8;
                p.rgba
                    .extend_from_slice(&[bande, (y / 50 * 20) as u8, 200, 255]);
            }
        }
        for y in 20..60.min(h) {
            for x in 20..100.min(l) {
                let i = ((y * l + x) * 4) as usize;
                p.rgba[i..i + 4].copy_from_slice(&[(k * 37) as u8, (k * 91) as u8, 0, 255]);
            }
        }
        p
    }

    /// Tout change, bien plus de 256 couleurs (le pire cas : NeuQuant).
    fn plein(l: u32, h: u32, k: u32) -> Pixels {
        let mut graine = 0x9E37_79B9u32 ^ k.wrapping_mul(0x85EB_CA6B);
        let mut rgba = Vec::with_capacity((l * h * 4) as usize);
        for _ in 0..l * h {
            graine ^= graine << 13;
            graine ^= graine >> 17;
            graine ^= graine << 5;
            let o = graine.to_le_bytes();
            rgba.extend_from_slice(&[o[0], o[1], o[2], 255]);
        }
        Pixels {
            largeur: l,
            hauteur: h,
            rgba,
        }
    }

    /// Une page qui défile de 7 pixels par pas : des dégradés, bien plus de 256 couleurs,
    /// et tout change à chaque pas.
    fn defile(l: u32, h: u32, n: u32) -> Pixels {
        let mut rgba = Vec::with_capacity((l * h * 4) as usize);
        for y in 0..h {
            let v = y + 7 * n;
            for x in 0..l {
                let ligne = if (v / 18).is_multiple_of(3) && x % 90 < 70 {
                    40
                } else {
                    0
                };
                rgba.extend_from_slice(&[
                    (x * 255 / l) as u8,
                    (v % 256) as u8,
                    ((x + v) % 200) as u8 + ligne,
                    255,
                ]);
            }
        }
        Pixels {
            largeur: l,
            hauteur: h,
            rgba,
        }
    }

    /// Ce que la relecture d'un GIF rend : la boucle, les images (indices, palette,
    /// rectangle, délai, transparence, mode).
    struct Relu {
        largeur: u16,
        hauteur: u16,
        boucle: ::gif::Repeat,
        images: Vec<::gif::Frame<'static>>,
    }

    fn relire(octets: &[u8]) -> Relu {
        let mut options = ::gif::DecodeOptions::new();
        options.set_color_output(::gif::ColorOutput::Indexed);
        let mut d = options.read_info(io::Cursor::new(octets)).expect("en-tête");
        let mut images = Vec::new();
        while let Some(f) = d.read_next_frame().expect("image") {
            images.push(f.clone());
        }
        Relu {
            largeur: d.width(),
            hauteur: d.height(),
            boucle: d.repeat(),
            images,
        }
    }

    /// Pose l'image `f` sur `toile` (RGBA, `l` de large), ses pixels transparents laissés.
    fn poser(toile: &mut [u8], l: usize, f: &::gif::Frame) {
        let palette = f.palette.as_deref().expect("palette locale");
        for y in 0..usize::from(f.height) {
            for x in 0..usize::from(f.width) {
                let i = f.buffer[y * usize::from(f.width) + x];
                if Some(i) == f.transparent {
                    continue;
                }
                let k = usize::from(i) * 3;
                let d = ((usize::from(f.top) + y) * l + usize::from(f.left) + x) * 4;
                toile[d..d + 3].copy_from_slice(&palette[k..k + 3]);
                toile[d + 3] = 255;
            }
        }
    }

    fn ecrire_suite(images: &[(Pixels, Duration)], duree: Duration) -> Vec<u8> {
        let (l, h) = (images[0].0.largeur as u16, images[0].0.hauteur as u16);
        let mut g = Gif::nouveau(Vec::new(), l, h, 10).expect("gif");
        for (p, t) in images {
            g.image(p, *t).expect("image");
        }
        g.finir(duree).expect("fin")
    }

    #[test]
    fn delais_avec_report_de_l_arrondi() {
        // 15 images par seconde : 6,67 cs par pas ; les délais font 7, 7, 6…, et leur
        // somme égale la durée.
        let d: Vec<u16> = (0..15).map(|n| delai_cs(t15(n), t15(n + 1))).collect();
        assert_eq!(d.iter().map(|&x| u32::from(x)).sum::<u32>(), 100);
        assert!(d.iter().all(|&x| x == 6 || x == 7));
        assert_eq!(delai_cs(ms(0), ms(4)), 0);
        assert_eq!(delai_cs(ms(0), ms(5)), 1);
        assert_eq!(delai_cs(ms(1000), ms(10)), 0);
        // 16 bits : 655,35 s au plus.
        assert_eq!(delai_cs(ms(0), Duration::from_secs(655)), 65_500);
        assert_eq!(delai_cs(ms(0), Duration::from_secs(700)), u16::MAX);
    }

    #[test]
    fn gif_relu_images_delais_premiere_entiere_boucle() {
        // 10 s à 15 images par seconde ; le compteur change dix fois par seconde (k = t ×
        // 10) : des pas de suite sont identiques et ne sont pas écrits.
        let (l, h) = (200u32, 150u32);
        let mut suite = Vec::new();
        let mut attendues = 0;
        let mut dernier_k = None;
        for n in 0..150u64 {
            let t = t15(n);
            let k = (t.as_millis() / 100) as u32;
            if dernier_k != Some(k) {
                attendues += 1;
                dernier_k = Some(k);
            }
            suite.push((simple(l, h, k), t));
        }
        let duree = Duration::from_secs(10);
        let octets = ecrire_suite(&suite, duree);
        assert_eq!(&octets[..6], b"GIF89a");
        let r = relire(&octets);
        assert_eq!((r.largeur, r.hauteur), (l as u16, h as u16));
        assert_eq!(r.boucle, ::gif::Repeat::Infinite);
        assert_eq!(r.images.len(), attendues);
        assert_eq!(attendues, 100);
        // Somme des délais = durée, au centième.
        let somme: u32 = r.images.iter().map(|f| u32::from(f.delay)).sum();
        assert_eq!(somme, 1000);
        // Le compteur change toutes les 100 ms, la grille tous les 66,7 ms : chaque image
        // reste un ou deux pas.
        assert!(r.images.iter().all(|f| (6..=14).contains(&f.delay)));
        // Première image : entière, opaque, palette locale.
        let p = &r.images[0];
        assert_eq!(
            (p.left, p.top, p.width, p.height),
            (0, 0, l as u16, h as u16)
        );
        assert_eq!(p.transparent, None);
        assert!(p.palette.is_some());
        // Les suivantes : le seul carré du compteur, l'image gardée dessous.
        for f in &r.images[1..] {
            assert_eq!((f.left, f.top, f.width, f.height), (20, 20, 80, 40));
            assert_eq!(f.dispose, ::gif::DisposalMethod::Keep);
            assert!(f.palette.is_some());
        }
        // Peu de couleurs : palette exacte, l'image composée est la source au pixel près.
        let mut toile = vec![0u8; (l * h * 4) as usize];
        let mut i = 0;
        let mut dernier_k = None;
        for (p, t) in &suite {
            let k = (t.as_millis() / 100) as u32;
            if dernier_k == Some(k) {
                continue;
            }
            dernier_k = Some(k);
            poser(&mut toile, l as usize, &r.images[i]);
            assert_eq!(toile, p.rgba, "image {i}");
            i += 1;
        }
    }

    #[test]
    fn image_identique_allonge_la_precedente() {
        let a = uni(10, 10, [1, 2, 3, 255]);
        let mut b = a.clone();
        b.rgba[0] = 200;
        let mut g = Gif::nouveau(Vec::new(), 10, 10, 10).expect("gif");
        assert!(g.image(&a, ms(0)).expect("a"));
        assert!(!g.image(&a, ms(500)).expect("même"));
        // L'alpha reçu ne compte pas : l'image est toujours opaque.
        let mut a_transparente = a.clone();
        a_transparente.rgba[3] = 0;
        assert!(!g.image(&a_transparente, ms(700)).expect("même"));
        assert!(g.image(&b, ms(1230)).expect("b"));
        assert_eq!(g.ecrites(), 1);
        let r = relire(&g.finir(ms(2000)).expect("fin"));
        assert_eq!(r.images.len(), 2);
        assert_eq!(r.images[0].delay, 123);
        assert_eq!(r.images[1].delay, 77);
        assert_eq!(
            (r.images[1].left, r.images[1].top, r.images[1].width),
            (0, 0, 1)
        );
    }

    #[test]
    fn plus_de_256_couleurs_aucun_opaque_transparent() {
        // Le pire cas : plus de 256 couleurs dans le rectangle changé, avec des pixels
        // inchangés dedans (donc un indice transparent) : NeuQuant. Aucun pixel qui a
        // changé ne doit tomber sur l'indice transparent.
        let (l, h) = (160u32, 120u32);
        let mut suite = vec![(plein(l, h, 0), t15(0))];
        for n in 1..20u64 {
            let mut p = suite[n as usize - 1].0.clone();
            let neuf = plein(l, h, n as u32);
            // Une case sur trois change : les autres restent (transparentes).
            for (i, px) in p.rgba.chunks_exact_mut(4).enumerate() {
                if (i + n as usize).is_multiple_of(3) {
                    px.copy_from_slice(&neuf.rgba[i * 4..i * 4 + 4]);
                }
            }
            suite.push((p, t15(n)));
        }
        let octets = ecrire_suite(&suite, t15(20));
        let r = relire(&octets);
        assert_eq!(r.images.len(), 20);
        let mut perdus = 0usize;
        let mut transparents = 0usize;
        for (n, f) in r.images.iter().enumerate().skip(1) {
            assert!(f.transparent.is_some(), "image {n} : indice transparent");
            let (avant, apres) = (&suite[n - 1].0, &suite[n].0);
            for y in 0..usize::from(f.height) {
                for x in 0..usize::from(f.width) {
                    let s = ((usize::from(f.top) + y) * l as usize + usize::from(f.left) + x) * 4;
                    let change = avant.rgba[s..s + 4] != apres.rgba[s..s + 4];
                    let i = f.buffer[y * usize::from(f.width) + x];
                    if Some(i) == f.transparent {
                        transparents += 1;
                        if change {
                            perdus += 1;
                        }
                    }
                }
            }
        }
        assert!(transparents > 0);
        assert_eq!(perdus, 0, "pixels opaques devenus transparents");
    }

    #[test]
    fn opaques_ramenes_a_l_indice_le_plus_proche() {
        // Palette : 0 noir, 1 rouge (transparent), 2 rouge presque, 3 blanc.
        let palette = [0, 0, 0, 255, 0, 0, 250, 5, 5, 255, 255, 255];
        let mut indices = [1, 1, 0, 3, 1];
        let alpha = [0, 255, 255, 255, 9];
        opaques_hors_du_transparent(&mut indices, &alpha, &palette, 1);
        assert_eq!(indices, [1, 2, 0, 3, 2]);
        // Rien de fautif : rien ne change.
        let mut indices = [1, 0];
        opaques_hors_du_transparent(&mut indices, &[0, 255], &palette, 1);
        assert_eq!(indices, [1, 0]);
        // Aucun autre indice : rien ne change, sans panique.
        let mut indices = [0, 0];
        opaques_hors_du_transparent(&mut indices, &[255, 255], &[1, 2, 3], 0);
        assert_eq!(indices, [0, 0]);
        // Indice transparent hors de la palette : sans panique.
        opaques_hors_du_transparent(&mut indices, &[255, 255], &[1, 2, 3], 7);
    }

    #[test]
    fn delai_au_dela_de_16_bits_continue() {
        let a = uni(4, 4, [5, 5, 5, 255]);
        let mut g = Gif::nouveau(Vec::new(), 4, 4, 10).expect("gif");
        g.image(&a, Duration::ZERO).expect("a");
        let r = relire(&g.finir(Duration::from_secs(700)).expect("fin"));
        let somme: u32 = r.images.iter().map(|f| u32::from(f.delay)).sum();
        assert_eq!(somme, 70_000);
        assert_eq!(r.images.len(), 2);
        assert_eq!(r.images[1].transparent, Some(0));
    }

    #[test]
    fn refus_sans_panique() {
        assert!(Gif::nouveau(Vec::new(), 0, 10, 10).is_err());
        assert!(Gif::nouveau(Vec::new(), 10, 10, 0).is_err());
        assert!(Gif::nouveau(Vec::new(), 10, 10, 31).is_err());
        let mut g = Gif::nouveau(Vec::new(), 10, 10, 10).expect("gif");
        assert!(g.image(&uni(11, 10, [0; 4]), ms(0)).is_err());
        let abime = Pixels {
            largeur: 10,
            hauteur: 10,
            rgba: vec![0; 12],
        };
        assert!(g.image(&abime, ms(0)).is_err());
        assert!(g.finir(ms(10)).is_err(), "aucune image");
        // Une sortie qui refuse d'écrire : une erreur, pas une panique.
        struct Refus;
        impl Write for Refus {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("disque plein"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let refus = Gif::nouveau(Refus, 10, 10, 10);
        assert!(refus.is_err());
        // Le disque se remplit en cours de route : l'image suivante le dit, la fin aussi,
        // et détruire le GIF ne panique pas.
        struct Plein(usize);
        impl Write for Plein {
            fn write(&mut self, b: &[u8]) -> io::Result<usize> {
                if self.0 < b.len() {
                    return Err(io::Error::other("disque plein"));
                }
                self.0 -= b.len();
                Ok(b.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut g = Gif::nouveau(Plein(200), 10, 10, 10).expect("en-tête");
        let mut a = uni(10, 10, [0, 0, 0, 255]);
        let mut fautes = 0;
        for k in 0..20u8 {
            a.rgba[0] = k;
            if g.image(&a, ms(u64::from(k) * 100)).is_err() {
                fautes += 1;
            }
        }
        assert!(fautes >= 1);
        assert!(g.finir(ms(3000)).is_err());
        let mut g = Gif::nouveau(Plein(200), 10, 10, 10).expect("en-tête");
        for k in 0..20u8 {
            a.rgba[0] = k;
            let _ = g.image(&a, ms(u64::from(k) * 100));
        }
        drop(g);
    }

    /// Une sortie qui ne garde rien : elle compte.
    struct Compteur(u64);
    impl Write for Compteur {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.0 += b.len() as u64;
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn memoire_bornee_quelle_que_soit_la_duree() {
        // 10 s puis 60 s du pire cas en 120 × 90 : ce que le GIF garde ne dépend que de
        // la taille de l'image, jamais de la durée ; la sortie reçoit tout au fil de l'eau.
        let (l, h) = (120u32, 90u32);
        let images: Vec<Pixels> = (0..4).map(|k| plein(l, h, k)).collect();
        let mut g = Gif::nouveau(Compteur(0), l as u16, h as u16, 30).expect("gif");
        let borne = (l * h * 4 * 3 + l * h * 2 + 3 * 256) as usize;
        let mut apres_10 = 0;
        let mut ecrit_10 = 0;
        for n in 0..900u64 {
            g.image(&images[(n % 4) as usize], t15(n)).expect("image");
            assert!(g.octets_gardes() <= borne, "image {n}");
            if n == 149 {
                apres_10 = g.octets_gardes();
                ecrit_10 = g.codeur.get_ref().sortie.0;
            }
        }
        assert_eq!(g.octets_gardes(), apres_10);
        assert!(g.codeur.get_ref().sortie.0 > ecrit_10 * 5);
        assert_eq!(g.ecrites(), 899);
        let w = g.finir(Duration::from_secs(60)).expect("fin");
        println!(
            "gif 120x90 60 s pire cas : {} octets, garde {apres_10} octets",
            w.0
        );
    }

    /// La mesure demandée : une zone 600 × 450, 10 s à 15 images par seconde, le cas
    /// ordinaire (un compteur) et le pire cas (tout change). En release seulement :
    /// `cargo test --release --lib film::gif::tests::mesure -- --ignored --nocapture`.
    #[test]
    #[ignore = "mesure : à lancer en release"]
    fn mesure_600x450_10_s() {
        // `defile` : tout change à chaque pas, mais une image d'écran (dégradés qui
        // défilent) ; `bruit` : tout change, du bruit (le pire cas pour la palette et LZW).
        for (nom, genre) in [("simple", 0u8), ("defile", 3), ("bruit", 1), ("fixe", 2)] {
            let suite: Vec<(Pixels, Duration)> = (0..150u64)
                .map(|n| {
                    let t = t15(n);
                    let k = (t.as_millis() / 100) as u32;
                    let p = match genre {
                        0 => simple(L, H, k),
                        1 => plein(L, H, n as u32),
                        3 => defile(L, H, n as u32),
                        _ => simple(L, H, 0),
                    };
                    (p, t)
                })
                .collect();
            let mut g = Gif::nouveau(Compteur(0), L as u16, H as u16, 10).expect("gif");
            let mut temps = Vec::new();
            let debut = Instant::now();
            for (p, t) in &suite {
                let c = Instant::now();
                g.image(p, *t).expect("image");
                temps.push(c.elapsed());
            }
            let w = g.finir(Duration::from_secs(10)).expect("fin");
            let total = debut.elapsed();
            let premiere = temps[0];
            temps.sort();
            println!(
                "mesure gif 600x450 10 s {nom} : total {:.1} ms, mediane {:.2} ms, max {:.2} ms, \
                 premiere {:.2} ms, {} octets",
                total.as_secs_f64() * 1000.0,
                temps[temps.len() / 2].as_secs_f64() * 1000.0,
                temps[temps.len() - 1].as_secs_f64() * 1000.0,
                premiere.as_secs_f64() * 1000.0,
                w.0
            );
        }
    }
}
