//! S4 : le GIF par la crate `gif` (0.14.2) et `color_quant` (NeuQuant). Windows seulement
//! (le code est portable, mais la mémoire se lit par l'API de Windows).
//!
//!   gif --taille 600x450 --genre simple|fixe|plein --fps 15 --vitesse 10
//!       --mode difference|entier --quant crate|table --duree 10 --sortie f.gif
//!       [--fond f.png] [--file 8] [--ecarts]
//!
//! Comme le ferait l'app, au fil de l'eau : un fil « prise » fabrique une image du témoin
//! par créneau de 1/fps (horloge réelle, le compteur `k` avance toutes les 100 ms) et la
//! pousse dans une file bornée (`--file` images) ; le fil « codeur » la code et l'écrit.
//! - `difference` (la règle du contrat) : seul le rectangle qui a changé depuis l'image
//!   précédente est écrit, les pixels inchangés y sont transparents, palette locale
//!   (`Frame::from_rgba_speed`), image gardée (`DisposalMethod::Keep`) ; une image sans
//!   changement allonge le délai de la précédente.
//! - `entier` : l'image entière à chaque créneau, sans transparence.
//!
//! - `--quant crate` : ce que fait `Frame::from_rgba_speed` (gif 0.14.2, `src/common.rs`),
//!   refait ici à l'identique pour séparer ses deux postes (vérifié sur la première image :
//!   `identique_a_la_crate`) : l'apprentissage (ensemble des couleurs, ou NeuQuant au-delà
//!   de 256) et la recherche de l'indice de chaque pixel. `--quant table` (S4 point 5) :
//!   au-delà de 256 couleurs, NeuQuant apprend 255 couleurs sur un échantillon (64 Ki
//!   pixels au plus), une table de 65 536 cases (5, 6 et 5 bits) donne l'indice de chaque
//!   pixel, l'indice 255 est réservé à la transparence ; image donnée par
//!   `Frame::from_palette_pixels`.
//! - `--ecarts` : toutes les 15 images, l'écart de couleur des pixels opaques (rendus par
//!   la palette) à la source, et, en `table`, à ce qu'aurait rendu `from_rgba_speed`
//!   (calculé à part, hors des temps).
//!
//! Délais en centièmes, arrondis sur l'horloge (somme exacte de la durée). Boucle infinie.
//! Écrit : temps par image en quatre postes (différence, apprentissage de la palette,
//! recherche de l'indice de chaque pixel, écriture LZW), temps total, temps qu'il reste à
//! écrire après la dernière image, attente de la prise sur la file pleine, pixels opaques
//! devenus transparents (indice de la couleur transparente), taille du fichier, pic de
//! mémoire privée.

#[cfg(not(windows))]
fn main() {
    eprintln!("gif : Windows seulement");
}

#[cfg(windows)]
fn main() {
    let a = sonde_film::arguments();
    let _minuterie = sonde_film::Minuterie1ms::poser();
    match principal(&a) {
        Ok(()) => println!("ok 1"),
        Err(e) => {
            println!("ok 0");
            println!("erreur {}", e.replace('\n', " "));
        }
    }
}

/// Une image quantifiée et ses temps (ms).
#[cfg(windows)]
struct Quant {
    image: gif::Frame<'static>,
    apprentissage: f64,
    recherche: f64,
    /// Pixels opaques dont l'indice est celui de la couleur transparente.
    opaques_transparents: u64,
}

#[cfg(windows)]
fn ms(t: std::time::Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

#[cfg(windows)]
fn opaques_transparents(rgba: &[u8], indices: &[u8], transparent: Option<u8>) -> u64 {
    let Some(t) = transparent else { return 0 };
    rgba.chunks_exact(4)
        .zip(indices)
        .filter(|(p, &i)| p[3] != 0 && i == t)
        .count() as u64
}

/// `Frame::from_rgba_speed` de gif 0.14.2, à l'identique, en deux temps.
#[cfg(windows)]
fn quant_crate(l: u16, h: u16, pixels: &mut [u8], vitesse: i32) -> Quant {
    use std::collections::{BTreeMap, BTreeSet};
    use std::time::Instant;
    let t = Instant::now();
    let mut transparent: Option<[u8; 4]> = None;
    for pix in pixels.chunks_exact_mut(4) {
        if pix[3] != 0 {
            pix[3] = 0xFF;
            continue;
        }
        if let Some([r, g, b, a]) = transparent {
            pix[0] = r;
            pix[1] = g;
            pix[2] = b;
            pix[3] = a;
        } else {
            transparent = Some([pix[0], pix[1], pix[2], pix[3]]);
        }
    }
    let mut colors: BTreeSet<(u8, u8, u8, u8)> = BTreeSet::new();
    let mut nq = None;
    for pixel in pixels.chunks_exact(4) {
        if colors.insert((pixel[0], pixel[1], pixel[2], pixel[3])) && colors.len() > 256 {
            nq = Some(color_quant::NeuQuant::new(vitesse, 256, pixels));
            break;
        }
    }
    if let Some(nq) = nq {
        let apprentissage = ms(t);
        let t = Instant::now();
        let buffer: Vec<u8> = pixels
            .chunks_exact(4)
            .map(|p| nq.index_of(p) as u8)
            .collect();
        let tr = transparent.map(|x| nq.index_of(&x) as u8);
        let recherche = ms(t);
        let o = opaques_transparents(pixels, &buffer, tr);
        let mut f = gif::Frame::from_palette_pixels(l, h, buffer, nq.color_map_rgb(), tr);
        f.transparent = tr;
        return Quant {
            image: f,
            apprentissage,
            recherche,
            opaques_transparents: o,
        };
    }
    let mut colors_vec: Vec<(u8, u8, u8, u8)> = colors.into_iter().collect();
    colors_vec.sort_unstable();
    let palette: Vec<u8> = colors_vec
        .iter()
        .flat_map(|&(r, g, b, _a)| [r, g, b])
        .collect();
    let lookup: BTreeMap<(u8, u8, u8, u8), u8> = colors_vec.into_iter().zip(0..=255).collect();
    let apprentissage = ms(t);
    let t = Instant::now();
    let index_of = |p: &[u8]| lookup.get(&(p[0], p[1], p[2], p[3])).copied().unwrap_or(0);
    let buffer: Vec<u8> = pixels.chunks_exact(4).map(index_of).collect();
    let tr = transparent.map(|x| index_of(&x));
    let recherche = ms(t);
    let o = opaques_transparents(pixels, &buffer, tr);
    Quant {
        image: gif::Frame::from_palette_pixels(l, h, buffer, palette, tr),
        apprentissage,
        recherche,
        opaques_transparents: o,
    }
}

/// S4 point 5 : NeuQuant sur un échantillon, table 5-6-5, indice 255 transparent.
#[cfg(windows)]
fn quant_table(l: u16, h: u16, pixels: &mut [u8], vitesse: i32) -> Quant {
    use std::collections::BTreeSet;
    use std::time::Instant;
    let t = Instant::now();
    let mut colors: BTreeSet<(u8, u8, u8)> = BTreeSet::new();
    let mut plus_de_256 = false;
    for p in pixels.chunks_exact(4) {
        if p[3] != 0 && colors.insert((p[0], p[1], p[2])) && colors.len() > 255 {
            plus_de_256 = true;
            break;
        }
    }
    let transparent = pixels.chunks_exact(4).any(|p| p[3] == 0);
    if !plus_de_256 {
        // Palette exacte (255 couleurs au plus) et l'indice 255 pour la transparence.
        let v: Vec<(u8, u8, u8)> = colors.into_iter().collect();
        let mut palette: Vec<u8> = v.iter().flat_map(|&(r, g, b)| [r, g, b]).collect();
        palette.resize(256 * 3, 0);
        let apprentissage = ms(t);
        let t = Instant::now();
        let buffer: Vec<u8> = pixels
            .chunks_exact(4)
            .map(|p| {
                if p[3] == 0 {
                    255
                } else {
                    v.binary_search(&(p[0], p[1], p[2]))
                        .map(|i| i as u8)
                        .unwrap_or(0)
                }
            })
            .collect();
        let recherche = ms(t);
        let tr = transparent.then_some(255u8);
        return Quant {
            image: gif::Frame::from_palette_pixels(l, h, buffer, palette, tr),
            apprentissage,
            recherche,
            opaques_transparents: 0,
        };
    }
    let opaques: Vec<&[u8]> = pixels.chunks_exact(4).filter(|p| p[3] != 0).collect();
    let pas = (opaques.len() / 65536).max(1);
    let mut echantillon = Vec::with_capacity(opaques.len() / pas * 4 + 4);
    for p in opaques.iter().step_by(pas) {
        echantillon.extend_from_slice(&[p[0], p[1], p[2], 255]);
    }
    let nq = color_quant::NeuQuant::new(vitesse, 255, &echantillon);
    let mut palette = nq.color_map_rgb();
    palette.resize(256 * 3, 0);
    let mut table = vec![0u8; 65536];
    for (code, case) in table.iter_mut().enumerate() {
        let r = ((code >> 11) & 31) as u8;
        let g = ((code >> 5) & 63) as u8;
        let b = (code & 31) as u8;
        let c = [
            (r << 3) | (r >> 2),
            (g << 2) | (g >> 4),
            (b << 3) | (b >> 2),
            255,
        ];
        *case = nq.index_of(&c) as u8;
    }
    let apprentissage = ms(t);
    let t = Instant::now();
    let buffer: Vec<u8> = pixels
        .chunks_exact(4)
        .map(|p| {
            if p[3] == 0 {
                255
            } else {
                table[((p[0] as usize >> 3) << 11)
                    | ((p[1] as usize >> 2) << 5)
                    | (p[2] as usize >> 3)]
            }
        })
        .collect();
    let recherche = ms(t);
    let tr = transparent.then_some(255u8);
    Quant {
        image: gif::Frame::from_palette_pixels(l, h, buffer, palette, tr),
        apprentissage,
        recherche,
        opaques_transparents: 0,
    }
}

/// Écart moyen et maximal (par canal) des pixels opaques de `rgba` rendus par l'image `f`,
/// et entre deux images rendues.
#[cfg(windows)]
fn rendu(f: &gif::Frame, i: usize) -> [u8; 3] {
    let p = f.palette.as_deref().unwrap_or(&[]);
    let k = f.buffer[i] as usize * 3;
    if k + 2 < p.len() {
        [p[k], p[k + 1], p[k + 2]]
    } else {
        [0, 0, 0]
    }
}

#[cfg(windows)]
fn ecart(rgba: &[u8], a: &gif::Frame, b: Option<&gif::Frame>) -> (f64, f64) {
    let (mut s, mut m, mut n) = (0f64, 0f64, 0f64);
    for (i, p) in rgba.chunks_exact(4).enumerate() {
        if p[3] == 0 {
            continue;
        }
        let x = rendu(a, i);
        let y = match b {
            Some(b) => rendu(b, i),
            None => [p[0], p[1], p[2]],
        };
        for c in 0..3 {
            let d = (x[c] as f64 - y[c] as f64).abs();
            s += d;
            m = m.max(d);
            n += 1.0;
        }
    }
    (if n > 0.0 { s / n } else { 0.0 }, m)
}

#[cfg(windows)]
fn principal(a: &std::collections::HashMap<String, String>) -> Result<(), String> {
    use sonde_film::temoin::Genre;
    use sonde_film::{
        attendre_jusqua, ligne_stats, lire_png_bgra, nombre, ressources, taille, Generateur,
    };
    use std::path::{Path, PathBuf};
    use std::sync::mpsc::sync_channel;
    use std::time::{Duration, Instant};

    let (l, h) = taille(a.get("taille").map(String::as_str).unwrap_or("600x450"))?;
    let genre = Genre::lire(a.get("genre").map(String::as_str).unwrap_or("simple"))?;
    let fps: f64 = nombre(a, "fps", 15.0);
    let vitesse: i32 = nombre(a, "vitesse", 10);
    let difference = a.get("mode").map(String::as_str) != Some("entier");
    let duree: f64 = nombre(a, "duree", 10.0);
    let file: usize = nombre(a, "file", 8);
    let table = a.get("quant").map(String::as_str) == Some("table");
    let ecarts = a.contains_key("ecarts");
    let sortie = PathBuf::from(a.get("sortie").ok_or("--sortie manquant")?);
    if !(1..=30).contains(&vitesse) {
        return Err("--vitesse de 1 à 30".into());
    }
    let fond = match a.get("fond") {
        Some(f) => lire_png_bgra(Path::new(f)).ok(),
        None => None,
    };
    let r0 = ressources();
    println!("taille {l}x{h}");
    println!("fps {fps}");
    println!("vitesse {vitesse}");
    println!("mode {}", if difference { "difference" } else { "entier" });
    println!("file {file}");
    println!("quant {}", if table { "table" } else { "crate" });

    let n = (duree * fps).round() as u64;
    let (tx, rx) = sync_channel::<(u64, f64, Vec<u8>)>(file);
    let fichier =
        std::fs::File::create(&sortie).map_err(|e| format!("{}: {e}", sortie.display()))?;

    // Fil codeur.
    #[derive(Default)]
    struct Mesures {
        diff: Vec<f64>,
        appr: Vec<f64>,
        rech: Vec<f64>,
        ecr: Vec<f64>,
        aires: Vec<f64>,
        tout: Vec<f64>,
        ecrites: u64,
        opaques_transparents: u64,
        identique: Option<bool>,
        ecart_src: Vec<(f64, f64)>,
        ecart_ref: Vec<(f64, f64)>,
        ecart_entre: Vec<(f64, f64)>,
        fin: Option<Instant>,
    }
    let codeur = std::thread::spawn(move || -> Result<Mesures, String> {
        let mut enc = gif::Encoder::new(std::io::BufWriter::new(fichier), l as u16, h as u16, &[])
            .map_err(|e| format!("gif Encoder::new: {e}"))?;
        enc.set_repeat(gif::Repeat::Infinite)
            .map_err(|e| format!("gif set_repeat: {e}"))?;
        let cs = |t: f64| (t * 100.0).round() as i64;
        // Travail du codeur pour chaque image reçue (différence, quantification, et
        // écriture de l'image précédente, faite à ce moment-là) : `m.tout`.
        let mut m = Mesures::default();
        let mut rangs = 0u64;
        let mut prec: Option<Vec<u8>> = None;
        // L'image quantifiée en attente de son délai, et le temps où elle paraît.
        let mut attente: Option<(gif::Frame<'static>, f64)> = None;
        let ecrire = |enc: &mut gif::Encoder<std::io::BufWriter<std::fs::File>>,
                      f: &mut gif::Frame<'static>,
                      depuis: f64,
                      jusqua: f64|
         -> Result<f64, String> {
            f.delay = (cs(jusqua) - cs(depuis)).clamp(0, u16::MAX as i64) as u16;
            let c = Instant::now();
            enc.write_frame(f)
                .map_err(|e| format!("gif write_frame: {e}"))?;
            Ok(c.elapsed().as_secs_f64() * 1000.0)
        };
        while let Ok((_i, t, bgra)) = rx.recv() {
            let c0 = Instant::now();
            let c = Instant::now();
            // Rectangle changé (ou tout).
            let rect = match (&prec, difference) {
                (Some(p), true) => {
                    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
                    for y in 0..h as usize {
                        let a = &p[y * l as usize * 4..(y + 1) * l as usize * 4];
                        let b = &bgra[y * l as usize * 4..(y + 1) * l as usize * 4];
                        if a == b {
                            continue;
                        }
                        y0 = y0.min(y);
                        y1 = y1.max(y + 1);
                        let premier =
                            (0..l as usize).find(|&x| a[x * 4..x * 4 + 3] != b[x * 4..x * 4 + 3]);
                        let dernier = (0..l as usize)
                            .rev()
                            .find(|&x| a[x * 4..x * 4 + 3] != b[x * 4..x * 4 + 3]);
                        if let (Some(f), Some(d)) = (premier, dernier) {
                            x0 = x0.min(f);
                            x1 = x1.max(d + 1);
                        }
                    }
                    if y0 == usize::MAX || x0 == usize::MAX {
                        None
                    } else {
                        Some((x0, y0, x1 - x0, y1 - y0))
                    }
                }
                _ => Some((0, 0, l as usize, h as usize)),
            };
            let Some((rx0, ry0, rl, rh)) = rect else {
                // Rien n'a changé : l'image en attente reste plus longtemps.
                m.diff.push(c.elapsed().as_secs_f64() * 1000.0);
                m.tout.push(c0.elapsed().as_secs_f64() * 1000.0);
                continue;
            };
            let mut rgba = vec![0u8; rl * rh * 4];
            for y in 0..rh {
                for x in 0..rl {
                    let s = ((ry0 + y) * l as usize + rx0 + x) * 4;
                    let d = (y * rl + x) * 4;
                    let inchange =
                        difference && prec.as_ref().is_some_and(|p| p[s..s + 3] == bgra[s..s + 3]);
                    if inchange {
                        rgba[d + 3] = 0;
                    } else {
                        rgba[d] = bgra[s + 2];
                        rgba[d + 1] = bgra[s + 1];
                        rgba[d + 2] = bgra[s];
                        rgba[d + 3] = 255;
                    }
                }
            }
            m.diff.push(c.elapsed().as_secs_f64() * 1000.0);
            m.aires.push((rl * rh) as f64);
            let source = (ecarts || m.identique.is_none()).then(|| rgba.clone());
            let q = if table {
                quant_table(rl as u16, rh as u16, &mut rgba, vitesse)
            } else {
                quant_crate(rl as u16, rh as u16, &mut rgba, vitesse)
            };
            m.appr.push(q.apprentissage);
            m.rech.push(q.recherche);
            m.opaques_transparents += q.opaques_transparents;
            let pause = Instant::now();
            if let Some(mut src) = source {
                if m.identique.is_none() && !table {
                    let r = gif::Frame::from_rgba_speed(
                        rl as u16,
                        rh as u16,
                        &mut src.clone(),
                        vitesse,
                    );
                    m.identique = Some(
                        r.buffer == q.image.buffer
                            && r.palette == q.image.palette
                            && r.transparent == q.image.transparent,
                    );
                }
                if ecarts && rangs.is_multiple_of(15) {
                    let mut norm = src.clone();
                    for p in norm.chunks_exact_mut(4) {
                        if p[3] != 0 {
                            p[3] = 255;
                        }
                    }
                    m.ecart_src.push(ecart(&norm, &q.image, None));
                    if table {
                        let r =
                            gif::Frame::from_rgba_speed(rl as u16, rh as u16, &mut src, vitesse);
                        m.ecart_ref.push(ecart(&norm, &r, None));
                        m.ecart_entre.push(ecart(&norm, &q.image, Some(&r)));
                    }
                }
            }
            // Les vérifications sont hors des temps : le début de l'image est décalé d'autant.
            let hors = pause.elapsed();
            rangs += 1;
            let mut f = q.image;
            f.left = rx0 as u16;
            f.top = ry0 as u16;
            f.dispose = gif::DisposalMethod::Keep;
            if let Some((mut p, depuis)) = attente.take() {
                m.ecr.push(ecrire(&mut enc, &mut p, depuis, t)?);
                m.ecrites += 1;
            }
            attente = Some((f, t));
            prec = Some(bgra);
            m.tout
                .push((c0.elapsed().saturating_sub(hors)).as_secs_f64() * 1000.0);
        }
        if let Some((mut p, depuis)) = attente.take() {
            m.ecr.push(ecrire(&mut enc, &mut p, depuis, duree)?);
            m.ecrites += 1;
        }
        let w = enc
            .into_inner()
            .map_err(|e| format!("gif into_inner: {e}"))?;
        w.into_inner()
            .map_err(|e| format!("écriture du fichier : {e}"))?
            .sync_all()
            .map_err(|e| format!("écriture du fichier : {e}"))?;
        m.fin = Some(Instant::now());
        Ok(m)
    });

    // Fil « prise » (celui-ci).
    let mut g = Generateur::nouveau(l, h, genre, fond.as_ref());
    let periode = Duration::from_secs_f64(1.0 / fps);
    let mut bloque = Vec::new();
    let mut generation = Vec::new();
    let mut en_retard = 0u64;
    let debut = Instant::now();
    for i in 0..n {
        let creneau = debut + periode.mul_f64(i as f64);
        attendre_jusqua(creneau);
        let t = i as f64 / fps;
        let c = Instant::now();
        let img = g.image((t * 10.0).floor() as u32).to_vec();
        generation.push(c.elapsed().as_secs_f64() * 1000.0);
        let c = Instant::now();
        if tx.send((i, t, img)).is_err() {
            break;
        }
        bloque.push(c.elapsed().as_secs_f64() * 1000.0);
        if Instant::now() > creneau + periode {
            en_retard += 1;
        }
    }
    let derniere = Instant::now();
    drop(tx);
    let m = codeur
        .join()
        .map_err(|_| "le fil codeur a paniqué".to_string())??;
    let fin = m.fin.unwrap_or_else(Instant::now);
    let ecrites = m.ecrites;
    let quant: Vec<f64> = m.appr.iter().zip(&m.rech).map(|(a, b)| a + b).collect();
    let r = ressources();
    println!("images_prises {n}");
    println!("images_ecrites {ecrites}");
    println!("{}", ligne_stats("generation_ms", &generation));
    println!("{}", ligne_stats("difference_ms", &m.diff));
    println!("{}", ligne_stats("apprentissage_ms", &m.appr));
    println!("{}", ligne_stats("recherche_ms", &m.rech));
    println!("{}", ligne_stats("quantification_ms", &quant));
    println!("{}", ligne_stats("ecriture_ms", &m.ecr));
    println!("{}", ligne_stats("par_image_ms", &m.tout));
    println!("{}", ligne_stats("aire_px", &m.aires));
    println!("opaques_devenus_transparents {}", m.opaques_transparents);
    if let Some(i) = m.identique {
        println!("identique_a_la_crate {}", i as u8);
    }
    let moy = |v: &[(f64, f64)]| {
        if v.is_empty() {
            "—".to_string()
        } else {
            format!(
                "moyen {:.2} ; max {:.0} ({} images)",
                v.iter().map(|x| x.0).sum::<f64>() / v.len() as f64,
                v.iter().map(|x| x.1).fold(0.0, f64::max),
                v.len()
            )
        }
    };
    if ecarts {
        println!("ecart_source {}", moy(&m.ecart_src));
        if table {
            println!("ecart_source_from_rgba_speed {}", moy(&m.ecart_ref));
            println!("ecart_table_contre_from_rgba_speed {}", moy(&m.ecart_entre));
        }
    }
    println!("prise_bloquee_ms_total {:.1}", bloque.iter().sum::<f64>());
    println!("prise_images_en_retard {en_retard}");
    println!(
        "reste_apres_derniere_ms {:.1}",
        fin.saturating_duration_since(derniere).as_secs_f64() * 1000.0
    );
    println!("total_s {:.2}", fin.duration_since(debut).as_secs_f64());
    println!(
        "taille_octets {}",
        std::fs::metadata(&sortie).map(|m| m.len()).unwrap_or(0)
    );
    println!("prive_repos_mo {:.1}", r0.0 as f64 / 1048576.0);
    println!("prive_pic_mo {:.1}", r.1 as f64 / 1048576.0);
    Ok(())
}
