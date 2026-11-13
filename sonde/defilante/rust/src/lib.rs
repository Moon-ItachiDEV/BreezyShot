//! Aides communes des exemples de la sonde défilante. Les exemples ne font quelque chose
//! que sous Windows ; ailleurs ils disent qu'ils ne servent à rien et sortent.

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

/// Écrit `lignes` dans `chemin` d'un coup (fichier temporaire puis renommage) : le lecteur
/// ne voit jamais un fichier à moitié écrit.
pub fn ecrire_etat(chemin: &Path, lignes: &[String]) {
    let temporaire = chemin.with_extension("tmp");
    let mut texte = lignes.join("\n");
    texte.push('\n');
    if std::fs::write(&temporaire, texte).is_ok() {
        // Le renommage échoue tant qu'un lecteur tient le fichier sans partage de
        // suppression : quelques essais (la dernière écriture, « fin », ne doit pas se perdre).
        for _ in 0..20 {
            if std::fs::rename(&temporaire, chemin).is_ok() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

/// Médiane, 95e centile et maximum, en millisecondes.
pub fn statistiques(durees: &[Duration]) -> (f64, f64, f64) {
    if durees.is_empty() {
        return (f64::NAN, f64::NAN, f64::NAN);
    }
    let mut ms: Vec<f64> = durees.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rang = |p: f64| -> f64 {
        let i = ((ms.len() as f64 - 1.0) * p).round() as usize;
        ms[i.min(ms.len() - 1)]
    };
    (rang(0.5), rang(0.95), ms[ms.len() - 1])
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

#[cfg(test)]
mod tests {
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
    }

    #[test]
    fn statistiques_simples() {
        let d: Vec<Duration> = (1..=100).map(Duration::from_millis).collect();
        let (m, p, x) = statistiques(&d);
        assert!((m - 51.0).abs() < 1.01 && (p - 95.0).abs() < 1.01 && (x - 100.0).abs() < 0.01);
    }
}
