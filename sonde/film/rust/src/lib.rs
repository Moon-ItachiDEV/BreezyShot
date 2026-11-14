//! Aides communes des exemples de la sonde du film (contrat 0.7, partie I). Les exemples ne
//! font quelque chose que sous Windows ; ailleurs ils disent qu'ils ne servent à rien.
//!
//! Le témoin (`temoin`) est dessiné ici exactement comme par `natif.cs` (`Temoin.Peindre`)
//! pour la fenêtre WPF de la sonde : mêmes constantes, mêmes couleurs. Les exemples qui
//! codent (MP4, GIF) fabriquent leurs images avec, et savent ce que chaque image doit
//! montrer (la valeur `k`, les pastilles).

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

/// Les arguments `--cle valeur` (ou `--drapeau` seul, qui vaut "1").
pub fn arguments() -> HashMap<String, String> {
    let mut sortie = HashMap::new();
    let liste: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < liste.len() {
        if let Some(cle) = liste[i].strip_prefix("--") {
            let valeur = match liste.get(i + 1) {
                Some(v) if !v.starts_with("--") => {
                    i += 1;
                    v.clone()
                }
                _ => "1".to_string(),
            };
            sortie.insert(cle.to_string(), valeur);
        }
        i += 1;
    }
    sortie
}

/// Lit `--cle` comme un nombre, ou `defaut`.
pub fn nombre<T: std::str::FromStr>(a: &HashMap<String, String>, cle: &str, defaut: T) -> T {
    a.get(cle).and_then(|s| s.parse().ok()).unwrap_or(defaut)
}

/// Un rectangle `x,y,l,h` en pixels réels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub l: i32,
    pub h: i32,
}

impl Rect {
    pub fn lire(texte: &str) -> Result<Rect, String> {
        let v: Vec<i32> = texte
            .split(',')
            .map(|s| s.trim().parse::<i32>())
            .collect::<Result<_, _>>()
            .map_err(|e| format!("rectangle illisible « {texte} » : {e}"))?;
        if v.len() != 4 || v[2] <= 0 || v[3] <= 0 {
            return Err(format!("rectangle illisible « {texte} »"));
        }
        Ok(Rect {
            x: v[0],
            y: v[1],
            l: v[2],
            h: v[3],
        })
    }
    pub fn contient(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.l && y < self.y + self.h
    }
}

/// `600x450` → (600, 450).
pub fn taille(texte: &str) -> Result<(u32, u32), String> {
    let (l, h) = texte
        .split_once('x')
        .ok_or_else(|| format!("taille illisible « {texte} »"))?;
    let l: u32 = l
        .trim()
        .parse()
        .map_err(|_| format!("taille illisible « {texte} »"))?;
    let h: u32 = h
        .trim()
        .parse()
        .map_err(|_| format!("taille illisible « {texte} »"))?;
    if l == 0 || h == 0 {
        return Err(format!("taille nulle « {texte} »"));
    }
    Ok((l, h))
}

/// Écrit `lignes` dans `chemin` d'un coup (fichier temporaire puis renommage) : le lecteur
/// ne voit jamais un fichier à moitié écrit.
pub fn ecrire_etat(chemin: &Path, lignes: &[String]) {
    let temporaire = chemin.with_extension("tmp");
    let mut texte = lignes.join("\n");
    texte.push('\n');
    if std::fs::write(&temporaire, texte).is_ok() {
        for _ in 0..20 {
            if std::fs::rename(&temporaire, chemin).is_ok() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// Médiane, 95e centile et maximum, en millisecondes.
pub fn statistiques(durees: &[Duration]) -> (f64, f64, f64) {
    let ms: Vec<f64> = durees.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    statistiques_ms(&ms)
}

/// Médiane, 95e centile et maximum d'une liste de valeurs (NaN si vide).
pub fn statistiques_ms(valeurs: &[f64]) -> (f64, f64, f64) {
    if valeurs.is_empty() {
        return (f64::NAN, f64::NAN, f64::NAN);
    }
    let mut ms = valeurs.to_vec();
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rang = |p: f64| -> f64 {
        let i = ((ms.len() as f64 - 1.0) * p).round() as usize;
        ms[i.min(ms.len() - 1)]
    };
    (rang(0.5), rang(0.95), ms[ms.len() - 1])
}

/// Une ligne « cle mediane;p95;max » (ms, deux décimales).
pub fn ligne_stats(cle: &str, valeurs: &[f64]) -> String {
    let (m, p, x) = statistiques_ms(valeurs);
    format!("{cle} {m:.2};{p:.2};{x:.2}")
}

/// Un HRESULT lisible : « 0x8007000E (message) ».
pub fn hresult(code: i32, message: &str) -> String {
    let m = message.trim().replace(['\r', '\n'], " ");
    if m.is_empty() {
        format!("0x{:08X}", code as u32)
    } else {
        format!("0x{:08X} ({m})", code as u32)
    }
}

// ---------------------------------------------------------------------------------------
// Le témoin (contrat 0.7, §1.0), en BGRA. Constantes partagées avec natif.cs.

#[cfg(windows)]
pub mod prise;

#[cfg(windows)]
pub mod charge;

pub mod temoin {
    /// Taille de la zone cliente du témoin.
    pub const L: i32 = 600;
    pub const H: i32 = 450;
    /// Seize carrés : le compteur `k` sur 16 bits, bit de poids fort à gauche ; noir = 1.
    pub const CARRES: i32 = 16;
    pub const CARRE: i32 = 32;
    pub const CARRE_PAS: i32 = 36;
    pub const CARRE_X: i32 = 12;
    pub const CARRE_Y: i32 = 12;
    /// La bande de la couleur `couleur(k)`, toute la largeur.
    pub const BANDE_Y: i32 = 60;
    pub const BANDE_H: i32 = 40;
    /// Quatre pastilles de couleurs fixes connues (RVB).
    pub const PASTILLE: i32 = 48;
    pub const PASTILLE_Y: i32 = 390;
    pub const PASTILLES_X: [i32; 4] = [20, 90, 160, 230];
    pub const PASTILLES: [[u8; 3]; 4] =
        [[200, 60, 60], [60, 170, 80], [60, 90, 200], [230, 200, 60]];
    /// Fond simple : blanc, filets gris toutes les 30 lignes, barres de « texte ».
    pub const BLANC: [u8; 3] = [255, 255, 255];
    pub const FILET: [u8; 3] = [225, 228, 234];
    pub const TEXTE: [u8; 3] = [60, 60, 60];

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Genre {
        /// Le compteur et la bande changent (peu de pixels).
        Simple,
        /// Rien ne bouge (k reste 0).
        Fixe,
        /// Tout le fond change à chaque pas, dégradé de plus de 256 couleurs.
        Plein,
    }

    impl Genre {
        pub fn lire(t: &str) -> Result<Genre, String> {
            match t {
                "simple" => Ok(Genre::Simple),
                "fixe" => Ok(Genre::Fixe),
                "plein" => Ok(Genre::Plein),
                autre => Err(format!("genre de témoin inconnu « {autre} »")),
            }
        }
    }

    /// La couleur de la bande pour `k` (RVB).
    pub fn couleur(k: u32) -> [u8; 3] {
        [
            ((k.wrapping_mul(67) + 40) & 255) as u8,
            ((k.wrapping_mul(29) + 120) & 255) as u8,
            ((k.wrapping_mul(151) + 200) & 255) as u8,
        ]
    }

    fn fond(x: i32, y: i32, k: u32, genre: Genre) -> [u8; 3] {
        if genre == Genre::Plein {
            let k = k as i32;
            return [
                ((x + 3 * k) & 255) as u8,
                ((y + 2 * k) & 255) as u8,
                (((x + y) / 2 + 5 * k) & 255) as u8,
            ];
        }
        if y % 30 < 2 {
            return FILET;
        }
        // Barres de « texte » : de 120 à 360, une par ligne de 30 px, longueur variable.
        if (120..360).contains(&y) && (y % 30) >= 10 && (y % 30) < 20 {
            let rangee = y / 30;
            let fin = 20 + 100 + (rangee * 73) % 400;
            if x >= 20 && x < fin {
                return TEXTE;
            }
        }
        BLANC
    }

    /// La couleur du pixel (x, y) du témoin (0 ≤ x < L, 0 ≤ y < H) pour `k`.
    pub fn pixel(x: i32, y: i32, k: u32, genre: Genre) -> [u8; 3] {
        let k = if genre == Genre::Fixe { 0 } else { k };
        if (CARRE_Y..CARRE_Y + CARRE).contains(&y) {
            for i in 0..CARRES {
                let x0 = CARRE_X + i * CARRE_PAS;
                if (x0..x0 + CARRE).contains(&x) {
                    let bit = (k >> (CARRES - 1 - i)) & 1;
                    return if bit == 1 { [0, 0, 0] } else { BLANC };
                }
            }
        }
        if (BANDE_Y..BANDE_Y + BANDE_H).contains(&y) {
            return couleur(k);
        }
        if (PASTILLE_Y..PASTILLE_Y + PASTILLE).contains(&y) {
            for (i, x0) in PASTILLES_X.iter().enumerate() {
                if (*x0..*x0 + PASTILLE).contains(&x) {
                    return PASTILLES[i];
                }
            }
        }
        fond(x, y, k, genre)
    }

    /// Peint le témoin dans `buf` (BGRA, `l` pixels par ligne, `h` lignes), son coin
    /// en (`ox`, `oy`) ; ce qui sort du tampon est coupé. Alpha 255.
    pub fn peindre(buf: &mut [u8], l: usize, h: usize, ox: i32, oy: i32, k: u32, genre: Genre) {
        peindre_lignes(buf, l, h, (ox, oy), k, genre, 0, H);
    }

    /// Le haut du témoin seulement (carrés et bande) : ce qui change dans le cas simple.
    pub const HAUT_CHANGEANT: i32 = BANDE_Y + BANDE_H;

    /// `peindre`, pour les seules lignes `y0..y1` du témoin.
    #[allow(clippy::too_many_arguments)]
    pub fn peindre_lignes(
        buf: &mut [u8],
        l: usize,
        h: usize,
        (ox, oy): (i32, i32),
        k: u32,
        genre: Genre,
        y0: i32,
        y1: i32,
    ) {
        for ty in y0.max(0)..y1.min(H) {
            let y = oy + ty;
            if y < 0 || y >= h as i32 {
                continue;
            }
            for tx in 0..L {
                let x = ox + tx;
                if x < 0 || x >= l as i32 {
                    continue;
                }
                let c = pixel(tx, ty, k, genre);
                let i = (y as usize * l + x as usize) * 4;
                buf[i] = c[2];
                buf[i + 1] = c[1];
                buf[i + 2] = c[0];
                buf[i + 3] = 255;
            }
        }
    }

    /// Moyenne RVB d'un carré `n × n` centré en (cx, cy) d'une image BGRA.
    pub fn moyenne(buf: &[u8], l: usize, cx: i32, cy: i32, n: i32) -> Option<[f64; 3]> {
        let h = buf.len() / 4 / l.max(1);
        let mut s = [0f64; 3];
        let mut c = 0f64;
        for y in cy - n / 2..cy + n / 2 {
            for x in cx - n / 2..cx + n / 2 {
                if x < 0 || y < 0 || x >= l as i32 || y >= h as i32 {
                    return None;
                }
                let i = (y as usize * l + x as usize) * 4;
                s[0] += buf[i + 2] as f64;
                s[1] += buf[i + 1] as f64;
                s[2] += buf[i] as f64;
                c += 1.0;
            }
        }
        if c == 0.0 {
            return None;
        }
        Some([s[0] / c, s[1] / c, s[2] / c])
    }

    /// Lit `k` sur les carrés d'une image BGRA (le témoin en (ox, oy)) ; `None` si un carré
    /// n'est ni clair ni sombre.
    pub fn lire_k(buf: &[u8], l: usize, ox: i32, oy: i32) -> Option<u32> {
        let mut k = 0u32;
        for i in 0..CARRES {
            let cx = ox + CARRE_X + i * CARRE_PAS + CARRE / 2;
            let cy = oy + CARRE_Y + CARRE / 2;
            let m = moyenne(buf, l, cx, cy, 12)?;
            let lum = 0.2126 * m[0] + 0.7152 * m[1] + 0.0722 * m[2];
            let bit = if lum < 80.0 {
                1
            } else if lum > 175.0 {
                0
            } else {
                return None;
            };
            k = (k << 1) | bit;
        }
        Some(k)
    }

    /// Écart maximal par canal entre la bande lue et `couleur(k)`.
    pub fn ecart_bande(buf: &[u8], l: usize, ox: i32, oy: i32, k: u32) -> Option<f64> {
        let m = moyenne(buf, l, ox + L / 2, oy + BANDE_Y + BANDE_H / 2, 16)?;
        let c = couleur(k);
        Some(
            (0..3)
                .map(|i| (m[i] - c[i] as f64).abs())
                .fold(0.0, f64::max),
        )
    }

    /// Écart maximal par canal de chacune des quatre pastilles (moyenne de 16 × 16 au centre).
    pub fn ecarts_pastilles(buf: &[u8], l: usize, ox: i32, oy: i32) -> Option<[f64; 4]> {
        let mut e = [0f64; 4];
        for (i, x0) in PASTILLES_X.iter().enumerate() {
            let m = moyenne(
                buf,
                l,
                ox + x0 + PASTILLE / 2,
                oy + PASTILLE_Y + PASTILLE / 2,
                16,
            )?;
            e[i] = (0..3)
                .map(|c| (m[c] - PASTILLES[i][c] as f64).abs())
                .fold(0.0, f64::max);
        }
        Some(e)
    }
}

/// Un fond de « bureau » fabriqué (quand la sonde n'a pas donné de copie de l'écran) :
/// dégradé bleu (plus de 256 couleurs) et barre des tâches sombre de 48 px.
pub fn fond_bureau(buf: &mut [u8], l: usize, h: usize) {
    for y in 0..h {
        for x in 0..l {
            let i = (y * l + x) * 4;
            let barre = y + 48 >= h;
            let (r, v, b) = if barre {
                (32u8, 32u8, 40u8)
            } else {
                (
                    (20 + (x * 60 / l.max(1))) as u8,
                    (60 + (y * 90 / h.max(1))) as u8,
                    (120 + ((x + y) * 100 / (l + h).max(1))) as u8,
                )
            };
            buf[i] = b;
            buf[i + 1] = v;
            buf[i + 2] = r;
            buf[i + 3] = 255;
        }
    }
}

/// Fabrique les images d'un film du témoin (BGRA, `l × h`) : le témoin au centre d'un fond
/// (copie de l'écran donnée par la sonde, sinon `fond_bureau`) si l'image est plus grande
/// que lui, sinon en (0, 0) et coupé. Ne repeint que ce qui change (le haut du témoin dans
/// le cas simple, rien dans le cas fixe, tout dans le cas plein).
pub struct Generateur {
    pub l: usize,
    pub h: usize,
    pub ox: i32,
    pub oy: i32,
    genre: temoin::Genre,
    fond: Vec<u8>,
    image: Vec<u8>,
    k_peint: Option<u32>,
}

impl Generateur {
    pub fn nouveau(
        l: u32,
        h: u32,
        genre: temoin::Genre,
        fond: Option<&(Vec<u8>, u32, u32)>,
    ) -> Self {
        let (l, h) = (l as usize, h as usize);
        let mut f = vec![0u8; l * h * 4];
        fond_bureau(&mut f, l, h);
        if let Some((png, pl, ph)) = fond {
            let (pl, ph) = (*pl as usize, *ph as usize);
            if pl > 0 && ph > 0 && png.len() >= pl * ph * 4 {
                for y in 0..h {
                    for x in 0..l {
                        let s = ((y % ph) * pl + (x % pl)) * 4;
                        let d = (y * l + x) * 4;
                        f[d..d + 4].copy_from_slice(&png[s..s + 4]);
                    }
                }
            }
        }
        let (ox, oy) = if l as i32 > temoin::L && h as i32 > temoin::H {
            ((l as i32 - temoin::L) / 2, (h as i32 - temoin::H) / 2)
        } else {
            (0, 0)
        };
        Generateur {
            l,
            h,
            ox,
            oy,
            genre,
            image: f.clone(),
            fond: f,
            k_peint: None,
        }
    }

    pub fn image(&mut self, k: u32) -> &[u8] {
        use temoin::Genre;
        match (self.genre, self.k_peint) {
            (Genre::Simple, Some(p)) if p != k => temoin::peindre_lignes(
                &mut self.image,
                self.l,
                self.h,
                (self.ox, self.oy),
                k,
                self.genre,
                0,
                temoin::HAUT_CHANGEANT,
            ),
            (Genre::Simple, Some(_)) | (Genre::Fixe, Some(_)) => {}
            _ => {
                self.image.copy_from_slice(&self.fond);
                temoin::peindre(
                    &mut self.image,
                    self.l,
                    self.h,
                    self.ox,
                    self.oy,
                    k,
                    self.genre,
                );
            }
        }
        self.k_peint = Some(k);
        &self.image
    }
}

/// Lit un PNG en BGRA (opaque) ; rend (octets, largeur, hauteur).
#[cfg(windows)]
pub fn lire_png_bgra(chemin: &Path) -> Result<(Vec<u8>, u32, u32), String> {
    let octets = std::fs::read(chemin).map_err(|e| format!("{}: {e}", chemin.display()))?;
    let mut dec = png::Decoder::new(std::io::Cursor::new(octets));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut lecteur = dec.read_info().map_err(|e| format!("png: {e}"))?;
    let mut tampon = vec![0u8; lecteur.output_buffer_size().ok_or("png: taille inconnue")?];
    let info = lecteur
        .next_frame(&mut tampon)
        .map_err(|e| format!("png: {e}"))?;
    let (l, h) = (info.width, info.height);
    let n = (l * h) as usize;
    let mut bgra = vec![255u8; n * 4];
    let canaux = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Indexed => return Err("png indexé non pris".into()),
    };
    for p in 0..n {
        let s = &tampon[p * canaux..p * canaux + canaux];
        let (r, v, b) = if canaux >= 3 {
            (s[0], s[1], s[2])
        } else {
            (s[0], s[0], s[0])
        };
        bgra[p * 4] = b;
        bgra[p * 4 + 1] = v;
        bgra[p * 4 + 2] = r;
    }
    Ok((bgra, l, h))
}

/// Encode une image BGRA (de haut en bas, `l × h`) en PNG RGB opaque.
#[cfg(windows)]
pub fn enregistrer_png_bgra(chemin: &Path, bgra: &[u8], l: u32, h: u32) -> Result<(), String> {
    let f = std::fs::File::create(chemin).map_err(|e| format!("{}: {e}", chemin.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), l, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Fast);
    let mut ecrivain = enc.write_header().map_err(|e| format!("png: {e}"))?;
    let mut rgb = Vec::with_capacity((l * h * 3) as usize);
    for p in bgra.chunks_exact(4) {
        rgb.extend_from_slice(&[p[2], p[1], p[0]]);
    }
    ecrivain
        .write_image_data(&rgb)
        .map_err(|e| format!("png: {e}"))
}

/// Mémoire privée du processus (octets) : actuelle et pic (`PagefileUsage`,
/// `PeakPagefileUsage`), et nombre d'objets GDI et USER.
#[cfg(windows)]
pub fn ressources() -> (u64, u64, u32, u32) {
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::{
        GetCurrentProcess, GetGuiResources, GR_GDIOBJECTS, GR_USEROBJECTS,
    };
    let mut c = PROCESS_MEMORY_COUNTERS {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ..Default::default()
    };
    // SAFETY : pseudo-handle du processus courant ; `c` est de la taille annoncée.
    unsafe {
        let p = GetCurrentProcess();
        let _ = GetProcessMemoryInfo(p, &mut c, c.cb);
        (
            c.PagefileUsage as u64,
            c.PeakPagefileUsage as u64,
            GetGuiResources(p, GR_GDIOBJECTS),
            GetGuiResources(p, GR_USEROBJECTS),
        )
    }
}

/// Temps de processeur du processus (noyau + utilisateur, tous fils), en secondes.
#[cfg(windows)]
pub fn temps_processeur() -> f64 {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    let (mut a, mut b, mut c, mut d) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    // SAFETY : pseudo-handle du processus courant ; quatre FILETIME valides.
    if unsafe { GetProcessTimes(GetCurrentProcess(), &mut a, &mut b, &mut c, &mut d) }.is_err() {
        return f64::NAN;
    }
    let v = |f: FILETIME| ((f.dwHighDateTime as u64) << 32 | f.dwLowDateTime as u64) as f64;
    (v(c) + v(d)) / 1e7
}

/// Pixels réels pour tout le processus (comme l'app), avant toute fenêtre.
#[cfg(windows)]
pub fn pixels_reels() {
    use windows::Win32::UI::HiDpi::{
        SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };
    // SAFETY : sans effet de bord hors du processus ; un échec (déjà posé) est sans gravité.
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

/// Minuterie du système à 1 ms le temps de l'objet (les attentes d'une cadence de 33 ms
/// ne doivent pas être arrondies au pas de 15,6 ms).
#[cfg(windows)]
pub struct Minuterie1ms;

#[cfg(windows)]
impl Minuterie1ms {
    pub fn poser() -> Minuterie1ms {
        // SAFETY : apparié à timeEndPeriod dans `drop`.
        unsafe {
            let _ = windows::Win32::Media::timeBeginPeriod(1);
        }
        Minuterie1ms
    }
}

#[cfg(windows)]
impl Drop for Minuterie1ms {
    fn drop(&mut self) {
        // SAFETY : apparié à timeBeginPeriod.
        unsafe {
            let _ = windows::Win32::Media::timeEndPeriod(1);
        }
    }
}

/// Attend jusqu'à `quand` (sommeil puis attente active de la dernière milliseconde).
pub fn attendre_jusqua(quand: std::time::Instant) {
    loop {
        let maintenant = std::time::Instant::now();
        if maintenant >= quand {
            return;
        }
        let reste = quand - maintenant;
        if reste > Duration::from_millis(2) {
            std::thread::sleep(reste - Duration::from_millis(1));
        } else {
            std::hint::spin_loop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::temoin::*;
    use super::*;

    #[test]
    fn rect_lu() {
        assert_eq!(
            Rect::lire("1,-2,3,4"),
            Ok(Rect {
                x: 1,
                y: -2,
                l: 3,
                h: 4
            })
        );
        assert!(Rect::lire("1,2,0,4").is_err());
        assert_eq!(taille("601x451"), Ok((601, 451)));
    }

    #[test]
    fn statistiques_simples() {
        let d: Vec<Duration> = (1..=100).map(Duration::from_millis).collect();
        let (m, p, x) = statistiques(&d);
        assert!((m - 51.0).abs() < 1.01 && (p - 95.0).abs() < 1.01 && (x - 100.0).abs() < 0.01);
    }

    #[test]
    fn temoin_relu() {
        for genre in [Genre::Simple, Genre::Plein] {
            for k in [0u32, 1, 2, 1234, 4095, 65535] {
                let (l, h) = (700usize, 500usize);
                let mut b = vec![0u8; l * h * 4];
                peindre(&mut b, l, h, 30, 20, k, genre);
                assert_eq!(lire_k(&b, l, 30, 20), Some(k));
                assert_eq!(ecart_bande(&b, l, 30, 20, k), Some(0.0));
                assert_eq!(ecarts_pastilles(&b, l, 30, 20), Some([0.0; 4]));
            }
        }
        let (l, h) = (600usize, 450usize);
        let mut b = vec![0u8; l * h * 4];
        peindre(&mut b, l, h, 0, 0, 77, Genre::Fixe);
        assert_eq!(lire_k(&b, l, 0, 0), Some(0));
    }

    #[test]
    fn generateur_repeint_ce_qui_change() {
        let mut g = Generateur::nouveau(1024, 768, Genre::Simple, None);
        let (ox, oy) = (g.ox, g.oy);
        assert_eq!((ox, oy), (212, 159));
        for k in [0u32, 1, 2, 3, 77] {
            let i = g.image(k).to_vec();
            assert_eq!(lire_k(&i, 1024, ox, oy), Some(k));
            assert_eq!(ecarts_pastilles(&i, 1024, ox, oy), Some([0.0; 4]));
        }
        let mut p = Generateur::nouveau(600, 450, Genre::Plein, None);
        let a = p.image(3).to_vec();
        let b = p.image(4).to_vec();
        assert_ne!(
            a[600 * 4 * 300..600 * 4 * 301],
            b[600 * 4 * 300..600 * 4 * 301]
        );
        assert_eq!(lire_k(&b, 600, 0, 0), Some(4));
    }

    /// Même empreinte que `natif.cs` (`Temoin.Peindre`, octets BGRA, s = s × 31 + b
    /// modulo 1 000 000 007) : les deux dessins du témoin sont identiques au bit près.
    #[test]
    fn meme_dessin_que_natif_cs() {
        let attendu = [
            (Genre::Simple, 0, 304625508u64),
            (Genre::Simple, 5, 109936270),
            (Genre::Simple, 1234, 628270942),
            (Genre::Simple, 65535, 121212849),
            (Genre::Plein, 0, 329216421),
            (Genre::Plein, 5, 256736263),
            (Genre::Plein, 1234, 732679127),
        ];
        for (genre, k, somme) in attendu {
            let mut b = vec![0u8; (L * H * 4) as usize];
            peindre(&mut b, L as usize, H as usize, 0, 0, k, genre);
            let s = b
                .iter()
                .fold(0u64, |s, &x| (s * 31 + x as u64) % 1_000_000_007);
            assert_eq!(s, somme, "{genre:?} k={k}");
        }
    }

    #[test]
    fn plein_a_plus_de_256_couleurs() {
        let mut vues = std::collections::HashSet::new();
        for y in 0..H {
            for x in 0..L {
                vues.insert(pixel(x, y, 5, Genre::Plein));
            }
        }
        assert!(vues.len() > 256);
        let mut simples = std::collections::HashSet::new();
        for y in 0..H {
            for x in 0..L {
                simples.insert(pixel(x, y, 5, Genre::Simple));
            }
        }
        assert!(simples.len() <= 256);
    }
}
