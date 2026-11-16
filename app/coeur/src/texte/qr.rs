//! Lire un QR code (QR code) : « Copier le texte » lit aussi un QR code. Pur, sauf le temps de
//! calcul ; n'importe que `crate::systeme::gel` (les pixels), comme `texte`.
//!
//! Ce que le code contient ne va jamais au journal ni à une page : le `Debug` de `Code` ne
//! dit que son genre et sa taille. Le contenu n'en sort que par `a_copier` (vers le
//! presse-papiers), `lien` (vers « Ouvrir ») et `detail` (la ligne du bandeau).
//!
//! Un code n'est pris que s'il est seul et qu'il couvre une bonne part de la zone
//! (`PART_MINIMALE`) : une capture de page avec un petit QR dans un coin rend son texte, pas
//! le code. Un lien n'est « ouvrable » que s'il est sûr à lire d'un coup d'œil (http ou
//! https, un nom de domaine en ASCII, sans identifiant devant) ; tout autre lien reste un
//! texte, qu'on copie sans pouvoir l'ouvrir d'un clic.

use std::fmt;

use rqrr::PreparedImage;

use crate::systeme::gel::Pixels;

/// La part de la zone que le code doit couvrir (surface de son cadre) : un dixième, soit un
/// code dont le côté fait le tiers de celui de la zone. En dessous, ce n'est pas lui que l'on
/// a choisi de lire (un petit QR dans un coin de page).
pub const PART_MINIMALE: f32 = 0.1;

/// Au-delà, l'image est réduite avant la lecture (un côté, en pixels).
pub const COTE_MAX: u32 = 3200;

/// En dessous, l'image est aussi essayée agrandie ×2 (un côté, en pixels).
const PETIT_COTE: u32 = 500;

/// Un lien plus long n'est jamais ouvert d'un clic (octets).
pub const LIEN_MAX: usize = 2048;

/// Le début du texte montré au bandeau (caractères), et la fin du domaine (caractères).
const DETAIL_TEXTE: usize = 60;
const DETAIL_DOMAINE: usize = 48;

/// Ce que dit un code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Genre {
    /// Un lien http ou https que l'on peut ouvrir d'un clic.
    Lien,
    /// Un réseau Wi-Fi (`WIFI:T:WPA;S:nom;P:mot de passe;;`).
    Wifi,
    /// Tout le reste : texte, `mailto:`, `tel:`, carte de visite, lien douteux…
    Texte,
}

/// Un code lu. Le contenu reste privé et hors de son `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct Code {
    genre: Genre,
    /// Ce qui va au presse-papiers : le lien entier, le mot de passe (ou le nom du réseau
    /// d'un réseau ouvert), ou le texte.
    copie: String,
    /// La ligne du bandeau : le domaine, le nom du réseau, le début du texte.
    detail: String,
    /// Le mot de passe est copié (et non le nom du réseau).
    avec_mot_de_passe: bool,
}

impl fmt::Debug for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Code")
            .field("genre", &self.genre)
            .field("octets", &self.copie.len())
            .finish_non_exhaustive()
    }
}

impl Code {
    pub fn genre(&self) -> Genre {
        self.genre
    }

    /// Ce qui va au presse-papiers.
    pub fn a_copier(&self) -> &str {
        &self.copie
    }

    /// Ce que la copie contient est un secret (un mot de passe Wi-Fi) : il ne va ni dans
    /// l'historique du presse-papiers ni dans le nuage de Windows.
    pub fn est_secret(&self) -> bool {
        self.genre == Genre::Wifi && self.avec_mot_de_passe
    }

    /// Le lien à ouvrir au clic ; `None` pour tout ce qui n'est pas un lien sûr.
    pub fn lien(&self) -> Option<&str> {
        (self.genre == Genre::Lien).then_some(self.copie.as_str())
    }

    /// La ligne du bandeau sous le titre.
    pub fn detail(&self) -> &str {
        &self.detail
    }

    /// Le genre en un mot, pour le journal.
    pub fn genre_en_clair(&self) -> &'static str {
        match self.genre {
            Genre::Lien => "lien",
            Genre::Wifi if self.avec_mot_de_passe => "wifi",
            Genre::Wifi => "wifi ouvert",
            Genre::Texte => "texte",
        }
    }
}

/// Lit le code de `pixels` : `None` s'il n'y en a pas, s'il y en a plusieurs différents, ou
/// s'il est trop petit dans la zone. Bloquant : dans le fil `texte`, jamais sur le fil
/// principal. Au premier essai qui réussit, les suivants ne sont pas faits.
pub fn lire(pixels: &Pixels) -> Option<Code> {
    let gris = Gris::depuis(pixels)?;
    // L'image comme elle est, puis ses couleurs inversées (un code clair sur fond sombre),
    // puis réduite de moitié (un code aux modules gros, ou un écran en 125 ou 150 %), puis
    // agrandie ×2 (une petite zone).
    if let Some(code) = essayer(&gris).or_else(|| essayer(&gris.inverser())) {
        return Some(code);
    }
    let cote = gris.l.max(gris.h);
    if cote >= 200 {
        if let Some(code) = essayer(&gris.reduire_de_moitie()) {
            return Some(code);
        }
    }
    if cote <= PETIT_COTE {
        return essayer(&gris.doubler());
    }
    None
}

/// Un seul passage de détection sur `gris`.
fn essayer(gris: &Gris) -> Option<Code> {
    let (l, h) = (gris.l as usize, gris.h as usize);
    let mut image = PreparedImage::prepare_from_greyscale(l, h, |x, y| gris.pixels[y * l + x]);
    let mut lus: Vec<String> = Vec::new();
    let mut couvert = 0f32;
    for grille in image.detect_grids() {
        let Ok((_, contenu)) = grille.decode() else {
            continue;
        };
        if contenu.is_empty() {
            continue;
        }
        let part = part_couverte(&grille.bounds, l as f32, h as f32);
        if !lus.contains(&contenu) {
            lus.push(contenu);
        }
        couvert = couvert.max(part);
    }
    // Plusieurs codes différents : on ne choisit pas pour la personne.
    if lus.len() != 1 || couvert < PART_MINIMALE {
        return None;
    }
    Some(classer(lus.remove(0)))
}

/// La part de l'image prise par le cadre du code (l'aire du quadrilatère, par la formule du
/// lacet), entre 0 et 1.
fn part_couverte(coins: &[rqrr::Point; 4], l: f32, h: f32) -> f32 {
    let mut somme = 0f32;
    for i in 0..4 {
        let (a, b) = (&coins[i], &coins[(i + 1) % 4]);
        somme += a.x as f32 * b.y as f32 - b.x as f32 * a.y as f32;
    }
    let total = l * h;
    if total <= 0.0 {
        return 0.0;
    }
    (somme.abs() / 2.0 / total).clamp(0.0, 1.0)
}

/// Les niveaux de gris d'une image (0 noir, 255 blanc).
#[derive(Clone)]
struct Gris {
    pixels: Vec<u8>,
    l: u32,
    h: u32,
}

impl Gris {
    /// Gris de `pixels` (alpha composé sur blanc), réduit par moyenne si un côté dépasse
    /// `COTE_MAX`. `None` si l'image est vide ou ses octets ne font pas `l × h × 4`.
    fn depuis(pixels: &Pixels) -> Option<Gris> {
        let (l, h) = (pixels.largeur, pixels.hauteur);
        let attendu = (l as usize).checked_mul(h as usize)?.checked_mul(4)?;
        if l == 0 || h == 0 || pixels.rgba.len() != attendu {
            return None;
        }
        let gris: Vec<u8> = pixels
            .rgba
            .chunks_exact(4)
            .map(|p| {
                let a = u32::from(p[3]);
                // Luminance (Rec. 601, en entiers), puis alpha sur blanc.
                let y =
                    (299 * u32::from(p[0]) + 587 * u32::from(p[1]) + 114 * u32::from(p[2])) / 1000;
                ((y * a + 255 * (255 - a) + 127) / 255) as u8
            })
            .collect();
        let mut g = Gris { pixels: gris, l, h };
        while g.l.max(g.h) > COTE_MAX {
            g = g.reduire_de_moitie();
        }
        Some(g)
    }

    /// Chaque pixel est la moyenne de 2 × 2 pixels (au moins 1 × 1).
    fn reduire_de_moitie(&self) -> Gris {
        let (nl, nh) = ((self.l / 2).max(1), (self.h / 2).max(1));
        let mut pixels = Vec::with_capacity(nl as usize * nh as usize);
        for y in 0..nh {
            for x in 0..nl {
                let mut somme = 0u32;
                let mut n = 0u32;
                for dy in 0..2 {
                    for dx in 0..2 {
                        let (sx, sy) = ((x * 2 + dx).min(self.l - 1), (y * 2 + dy).min(self.h - 1));
                        somme += u32::from(self.pixels[(sy * self.l + sx) as usize]);
                        n += 1;
                    }
                }
                pixels.push(((somme + n / 2) / n) as u8);
            }
        }
        Gris {
            pixels,
            l: nl,
            h: nh,
        }
    }

    /// Agrandit ×2 au plus proche voisin.
    fn doubler(&self) -> Gris {
        let (nl, nh) = (self.l * 2, self.h * 2);
        let mut pixels = Vec::with_capacity(nl as usize * nh as usize);
        for y in 0..nh {
            for x in 0..nl {
                pixels.push(self.pixels[((y / 2) * self.l + x / 2) as usize]);
            }
        }
        Gris {
            pixels,
            l: nl,
            h: nh,
        }
    }

    fn inverser(&self) -> Gris {
        Gris {
            pixels: self.pixels.iter().map(|v| 255 - v).collect(),
            l: self.l,
            h: self.h,
        }
    }
}

/// Range ce que dit un code : un réseau Wi-Fi, un lien sûr, ou un texte.
pub fn classer(contenu: String) -> Code {
    if let Some(wifi) = analyser_wifi(&contenu) {
        let (copie, detail, avec_mot_de_passe) = match wifi.mot_de_passe {
            Some(mdp) => (mdp, wifi.nom, true),
            None => (
                wifi.nom.clone(),
                format!("{} (réseau ouvert)", wifi.nom),
                false,
            ),
        };
        return Code {
            genre: Genre::Wifi,
            copie,
            detail: couper_debut(&detail, DETAIL_TEXTE),
            avec_mot_de_passe,
        };
    }
    if let Some(domaine) = domaine_sur(&contenu) {
        return Code {
            genre: Genre::Lien,
            detail: couper_fin(&domaine, DETAIL_DOMAINE),
            copie: contenu,
            avec_mot_de_passe: false,
        };
    }
    let detail = couper_debut(
        &contenu.split_whitespace().collect::<Vec<_>>().join(" "),
        DETAIL_TEXTE,
    );
    Code {
        genre: Genre::Texte,
        copie: contenu,
        detail,
        avec_mot_de_passe: false,
    }
}

/// Le domaine (et son port s'il y en a un) d'un lien sûr ; `None` si ce n'est pas un lien
/// http ou https, ou s'il est douteux : plus de `LIEN_MAX` octets, un espace ou un
/// caractère de contrôle, une barre oblique inverse, un identifiant devant le domaine
/// (`http://banque.fr@autre.site`), un domaine vide ou qui n'est pas en ASCII (des lettres
/// qui en imitent d'autres), un port qui n'est pas un nombre.
pub fn domaine_sur(contenu: &str) -> Option<String> {
    if contenu.len() > LIEN_MAX
        || contenu
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || c == '\\')
    {
        return None;
    }
    let reste = {
        let bas = contenu.get(..8).map(str::to_ascii_lowercase);
        let bas7 = contenu.get(..7).map(str::to_ascii_lowercase);
        if bas.as_deref() == Some("https://") {
            &contenu[8..]
        } else if bas7.as_deref() == Some("http://") {
            &contenu[7..]
        } else {
            return None;
        }
    };
    let autorite = reste.split(['/', '?', '#']).next().unwrap_or("");
    if autorite.contains('@') {
        return None;
    }
    let (hote, port) = match autorite.rsplit_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (autorite, None),
    };
    if let Some(p) = port {
        if p.is_empty() || p.len() > 5 || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
    }
    let hote_ok = !hote.is_empty()
        && !hote.starts_with(['.', '-'])
        && !hote.ends_with(['-'])
        && !hote.contains("..")
        && hote
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
    if !hote_ok {
        return None;
    }
    Some(match port {
        Some(p) => format!("{}:{p}", hote.to_ascii_lowercase()),
        None => hote.to_ascii_lowercase(),
    })
}

struct Wifi {
    nom: String,
    mot_de_passe: Option<String>,
}

/// `WIFI:T:WPA;S:nom;P:mot de passe;H:false;;` : les champs dans n'importe quel ordre, les
/// `\;`, `\:`, `\,`, `\"` et `\\` rendus. Un nom vide n'est pas un réseau ; un mot de passe
/// vide, ou `T:nopass`, est un réseau ouvert.
fn analyser_wifi(contenu: &str) -> Option<Wifi> {
    let debut = contenu.get(..5)?;
    if !debut.eq_ignore_ascii_case("WIFI:") {
        return None;
    }
    let mut nom = None;
    let mut mdp = None;
    let mut sans_mot_de_passe = false;
    let mut champ = String::new();
    let mut echappe = false;
    let mut champs: Vec<String> = Vec::new();
    for c in contenu[5..].chars() {
        if echappe {
            champ.push(c);
            echappe = false;
        } else if c == '\\' {
            echappe = true;
            // Le champ garde la barre pour que `:` échappé ne coupe pas la clé : on la
            // retire plus bas, au découpage clé/valeur.
            champ.push('\u{1}');
        } else if c == ';' {
            champs.push(std::mem::take(&mut champ));
        } else {
            champ.push(c);
        }
    }
    if !champ.is_empty() {
        champs.push(champ);
    }
    for c in champs {
        // Le premier `:` non échappé sépare la clé de la valeur.
        let mut cle = String::new();
        let mut valeur = String::new();
        let mut dans_valeur = false;
        let mut precedent_echappe = false;
        for ch in c.chars() {
            if ch == '\u{1}' {
                precedent_echappe = true;
                continue;
            }
            if dans_valeur {
                valeur.push(ch);
            } else if ch == ':' && !precedent_echappe {
                dans_valeur = true;
            } else {
                cle.push(ch);
            }
            precedent_echappe = false;
        }
        match cle.as_str() {
            "S" => nom = Some(valeur),
            "P" => mdp = Some(valeur),
            "T" if valeur.eq_ignore_ascii_case("nopass") => sans_mot_de_passe = true,
            _ => {}
        }
    }
    let nom = nom.filter(|n| !n.is_empty())?;
    let mot_de_passe = mdp.filter(|m| !m.is_empty() && !sans_mot_de_passe);
    Some(Wifi { nom, mot_de_passe })
}

/// Les `max` premiers caractères, « … » si on a coupé.
fn couper_debut(texte: &str, max: usize) -> String {
    if texte.chars().count() <= max {
        return texte.to_string();
    }
    let mut s: String = texte.chars().take(max).collect();
    s.push('…');
    s
}

/// Les `max` derniers caractères, « … » devant si on a coupé : la fin d'un domaine est ce
/// qui dit où l'on va.
fn couper_fin(texte: &str, max: usize) -> String {
    let n = texte.chars().count();
    if n <= max {
        return texte.to_string();
    }
    let fin: String = texte.chars().skip(n - max).collect();
    format!("…{fin}")
}

#[cfg(test)]
mod tests {
    use qrcode::{Color, QrCode};

    use super::*;

    /// Un QR code de `contenu`, chaque module sur `module` pixels, une marge de 4 modules,
    /// posé au milieu d'une image `l × h` (blanc, ou noir si `sombre`) ; `inverse` : le
    /// code clair sur fond sombre.
    fn image(contenu: &str, module: u32, l: u32, h: u32, inverse: bool) -> Pixels {
        let code = QrCode::new(contenu.as_bytes()).unwrap();
        let n = code.width() as u32;
        let couleurs = code.to_colors();
        let (fond, noir) = if inverse { (0u8, 255u8) } else { (255u8, 0u8) };
        let cote = (n + 8) * module;
        assert!(cote <= l && cote <= h, "le code ne tient pas : {cote}");
        let (x0, y0) = ((l - cote) / 2, (h - cote) / 2);
        let mut rgba = vec![fond; (l * h * 4) as usize];
        for i in 0..(l * h) as usize {
            rgba[i * 4 + 3] = 255;
        }
        for my in 0..n {
            for mx in 0..n {
                if couleurs[(my * n + mx) as usize] != Color::Dark {
                    continue;
                }
                for dy in 0..module {
                    for dx in 0..module {
                        let (x, y) = (x0 + (mx + 4) * module + dx, y0 + (my + 4) * module + dy);
                        let i = ((y * l + x) * 4) as usize;
                        rgba[i..i + 3].fill(noir);
                    }
                }
            }
        }
        Pixels {
            largeur: l,
            hauteur: h,
            rgba,
        }
    }

    #[test]
    fn lit_un_lien() {
        let code = lire(&image(
            "https://exemple.fr/reserver?evt=12",
            6,
            400,
            400,
            false,
        ))
        .unwrap();
        assert_eq!(code.genre(), Genre::Lien);
        assert_eq!(code.a_copier(), "https://exemple.fr/reserver?evt=12");
        assert_eq!(code.lien(), Some("https://exemple.fr/reserver?evt=12"));
        assert_eq!(code.detail(), "exemple.fr");
        assert!(!code.est_secret());
    }

    #[test]
    fn lit_un_code_clair_sur_fond_sombre() {
        let code = lire(&image("https://exemple.fr", 6, 400, 400, true)).unwrap();
        assert_eq!(code.a_copier(), "https://exemple.fr");
    }

    #[test]
    fn lit_un_code_aux_gros_modules_et_un_petit() {
        // 14 px le module : une capture d'un écran en 150 % agrandie ; et 2 px : une zone
        // minuscule (la lecture l'agrandit).
        assert!(lire(&image("https://exemple.fr", 14, 700, 700, false)).is_some());
        assert!(lire(&image("https://exemple.fr", 2, 120, 120, false)).is_some());
    }

    #[test]
    fn lit_un_texte_et_garde_ses_retours_a_la_ligne() {
        let code = lire(&image("Billet 2412\nRang F", 6, 400, 400, false)).unwrap();
        assert_eq!(code.genre(), Genre::Texte);
        assert_eq!(code.lien(), None);
        assert_eq!(code.a_copier(), "Billet 2412\nRang F");
    }

    #[test]
    fn un_code_trop_petit_dans_la_zone_laisse_le_texte() {
        // Un petit code dans une grande zone : 3 px le module, zone 900 × 900.
        let petit = image("https://exemple.fr", 3, 900, 900, false);
        assert!(
            part_couverte(
                &[
                    rqrr::Point { x: 0, y: 0 },
                    rqrr::Point { x: 10, y: 0 },
                    rqrr::Point { x: 10, y: 10 },
                    rqrr::Point { x: 0, y: 10 }
                ],
                100.0,
                100.0
            ) < PART_MINIMALE
        );
        assert!(lire(&petit).is_none());
    }

    #[test]
    fn deux_codes_differents_ne_donnent_rien() {
        let a = image("https://un.fr", 4, 300, 300, false);
        let b = image("https://deux.fr", 4, 300, 300, false);
        let mut rgba = Vec::new();
        for y in 0..300usize {
            rgba.extend_from_slice(&a.rgba[y * 300 * 4..(y + 1) * 300 * 4]);
            rgba.extend_from_slice(&b.rgba[y * 300 * 4..(y + 1) * 300 * 4]);
        }
        let deux = Pixels {
            largeur: 600,
            hauteur: 300,
            rgba,
        };
        assert!(lire(&deux).is_none());
    }

    #[test]
    fn sans_code_ou_image_vide_rien() {
        assert!(lire(&Pixels {
            largeur: 200,
            hauteur: 200,
            rgba: vec![255; 200 * 200 * 4]
        })
        .is_none());
        assert!(lire(&Pixels {
            largeur: 0,
            hauteur: 0,
            rgba: Vec::new()
        })
        .is_none());
        assert!(lire(&Pixels {
            largeur: 10,
            hauteur: 10,
            rgba: vec![0; 5]
        })
        .is_none());
    }

    #[test]
    fn le_debug_ne_dit_pas_le_contenu() {
        let code = classer("https://secret.exemple.fr/jeton=abc".into());
        let d = format!("{code:?}");
        assert!(d.contains("Lien") && d.contains("octets"));
        assert!(!d.contains("secret") && !d.contains("abc"));
        let wifi = classer("WIFI:T:WPA;S:Maison;P:hunter2;;".into());
        let d = format!("{wifi:?}");
        assert!(!d.contains("hunter2") && !d.contains("Maison"));
    }

    #[test]
    fn un_lien_sur_est_ouvrable() {
        for (lien, domaine) in [
            ("https://github.com/Moon-ItachiDEV", "github.com"),
            ("HTTP://Exemple.FR/Chemin", "exemple.fr"),
            ("https://exemple.fr:8443/x", "exemple.fr:8443"),
            ("http://192.168.1.10/admin", "192.168.1.10"),
            ("https://a.b-c.fr?x=1#y", "a.b-c.fr"),
        ] {
            assert_eq!(domaine_sur(lien).as_deref(), Some(domaine), "{lien}");
            assert_eq!(classer(lien.into()).genre(), Genre::Lien, "{lien}");
        }
    }

    #[test]
    fn un_lien_douteux_reste_un_texte() {
        for lien in [
            "http://banque.fr@pirate.site/connexion",
            "https://exemple.fr\\@pirate.site",
            "https://раураl.com/x",
            "https://xn--80ak6aa92e.com",
            "https:///x",
            "https://.exemple.fr",
            "https://exemple..fr",
            "https://exemple.fr:port/x",
            "https://exemple.fr: 80",
            "https://exemple.fr/un deux",
            "https://exemple.fr/\u{7}",
            "ftp://exemple.fr",
            "javascript:alert(1)",
            "file:///C:/Windows/system32/calc.exe",
            "mailto:moi@exemple.fr",
            "exemple.fr",
        ] {
            let attendu_lien = lien == "https://xn--80ak6aa92e.com";
            let genre = classer(lien.into()).genre();
            // Le punycode brut est de l'ASCII : on le montre tel quel, il se lit.
            assert_eq!(genre == Genre::Lien, attendu_lien, "{lien}");
            assert!(
                classer(lien.into()).lien().is_some() == attendu_lien,
                "{lien}"
            );
        }
    }

    #[test]
    fn un_lien_trop_long_n_est_pas_ouvrable() {
        let long = format!("https://exemple.fr/{}", "a".repeat(LIEN_MAX));
        assert_eq!(classer(long).genre(), Genre::Texte);
    }

    #[test]
    fn le_domaine_trop_long_montre_sa_fin() {
        let hote = format!("{}.exemple.fr", "a".repeat(80));
        let code = classer(format!("https://{hote}/x"));
        assert!(code.detail().starts_with('…'));
        assert!(code.detail().ends_with("exemple.fr"));
        assert_eq!(code.detail().chars().count(), DETAIL_DOMAINE + 1);
    }

    #[test]
    fn le_wifi_copie_le_mot_de_passe_et_montre_le_reseau() {
        let code = classer("WIFI:T:WPA;S:Salle-Pleyel;P:aurore2412;;".into());
        assert_eq!(code.genre(), Genre::Wifi);
        assert_eq!(code.a_copier(), "aurore2412");
        assert_eq!(code.detail(), "Salle-Pleyel");
        assert!(code.est_secret());
        assert_eq!(code.lien(), None);
    }

    #[test]
    fn le_wifi_rend_les_echappements_et_l_ordre_libre() {
        let code = classer(r#"WIFI:P:a\;b\:c\\d\"e;S:Mon\;Wifi;T:WPA;H:true;;"#.into());
        assert_eq!(code.a_copier(), r#"a;b:c\d"e"#);
        assert_eq!(code.detail(), "Mon;Wifi");
        let minuscule = classer("wifi:S:Maison;P:xyz;;".into());
        assert_eq!(minuscule.genre(), Genre::Wifi);
    }

    #[test]
    fn un_wifi_ouvert_copie_le_nom_du_reseau() {
        for contenu in [
            "WIFI:T:nopass;S:Cafe;;",
            "WIFI:S:Cafe;P:;;",
            "WIFI:T:nopass;S:Cafe;P:abc;;",
        ] {
            let code = classer(contenu.into());
            assert_eq!(code.genre(), Genre::Wifi, "{contenu}");
            assert_eq!(code.a_copier(), "Cafe", "{contenu}");
            assert!(code.detail().contains("réseau ouvert"));
            assert!(!code.est_secret());
        }
    }

    #[test]
    fn un_wifi_sans_nom_est_un_texte() {
        assert_eq!(classer("WIFI:T:WPA;P:abc;;".into()).genre(), Genre::Texte);
        assert_eq!(classer("WIFI:".into()).genre(), Genre::Texte);
    }

    #[test]
    fn le_detail_du_texte_est_coupe_sur_une_ligne() {
        let code = classer(format!("Un\nDeux\t{}", "x".repeat(100)));
        assert!(!code.detail().contains('\n') && !code.detail().contains('\t'));
        assert_eq!(code.detail().chars().count(), DETAIL_TEXTE + 1);
        assert!(code.detail().ends_with('…'));
        assert_eq!(code.a_copier().chars().count(), 2 + 1 + 4 + 1 + 100);
    }
}
