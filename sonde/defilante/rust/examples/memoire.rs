//! S7 : encoder et copier une très grande image, comme `sortie` de l'app (PNG RGBA
//! « Fast » par la crate png, presse-papiers par arboard). Windows seulement.
//!
//!   memoire --taille 1000x16384 --png sortie.png
//!
//! Une image par processus : le pic de mémoire privée (`PeakPagefileUsage`) est celui de
//! cette seule image. L'image : des bandes de couleurs de 7 lignes (jamais noires), pas
//! du bruit, comme une page. Écrit une ligne « cle valeur » par mesure.

#[cfg(not(windows))]
fn main() {
    eprintln!("memoire : Windows seulement");
}

#[cfg(windows)]
fn main() {
    let a = sonde_defilante::arguments();
    match principal(&a) {
        Ok(()) => println!("ok 1"),
        Err(e) => {
            println!("ok 0");
            println!("erreur {}", e.replace('\n', " "));
        }
    }
}

#[cfg(windows)]
fn principal(a: &std::collections::HashMap<String, String>) -> Result<(), String> {
    use sonde_defilante::ressources;
    use std::time::Instant;
    let taille = a.get("taille").ok_or("--taille manquant")?;
    let (l, h) = taille
        .split_once('x')
        .and_then(|(l, h)| Some((l.parse::<u32>().ok()?, h.parse::<u32>().ok()?)))
        .ok_or("taille illisible")?;
    let sortie = std::path::PathBuf::from(a.get("png").ok_or("--png manquant")?);
    let repos = ressources();
    println!("taille {l}x{h}");
    println!("pixels {}", l as u64 * h as u64);
    println!("prive_repos {}", repos.0);

    let t = Instant::now();
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(l as usize * h as usize * 4)
        .map_err(|e| format!("mémoire refusée pour l'image : {e}"))?;
    for y in 0..h {
        let bande = y / 7;
        let c = [
            (40 + (bande * 37) % 200) as u8,
            (60 + (bande * 53) % 180) as u8,
            (80 + (bande * 71) % 160) as u8,
            255u8,
        ];
        for x in 0..l {
            // Un trait plus sombre tous les 64 pixels : des colonnes, comme du texte.
            if x % 64 < 2 {
                rgba.extend_from_slice(&[c[0] / 2, c[1] / 2, c[2] / 2, 255]);
            } else {
                rgba.extend_from_slice(&c);
            }
        }
    }
    println!("fabrication_ms {:.0}", t.elapsed().as_secs_f64() * 1000.0);

    // Encodage en mémoire, comme `sortie::png::encoder_rgba`.
    let t = Instant::now();
    let mut png_octets: Vec<u8> = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png_octets, l, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        let mut e = enc.write_header().map_err(|e| format!("png: {e}"))?;
        e.write_image_data(&rgba).map_err(|e| format!("png: {e}"))?;
    }
    let encodage = t.elapsed();
    println!("encodage_ms {:.0}", encodage.as_secs_f64() * 1000.0);
    println!("png_octets {}", png_octets.len());

    let t = Instant::now();
    std::fs::write(&sortie, &png_octets).map_err(|e| format!("écriture: {e}"))?;
    let ecriture = t.elapsed();
    println!("ecriture_ms {:.0}", ecriture.as_secs_f64() * 1000.0);
    println!(
        "fichier_ms {:.0}",
        (encodage + ecriture).as_secs_f64() * 1000.0
    );

    let t = Instant::now();
    let copie = arboard::Clipboard::new().and_then(|mut c| {
        c.set_image(arboard::ImageData {
            width: l as usize,
            height: h as usize,
            bytes: std::borrow::Cow::Borrowed(&rgba),
        })
    });
    let duree_copie = t.elapsed();
    match copie {
        Ok(()) => println!("copie_ms {:.0}", duree_copie.as_secs_f64() * 1000.0),
        Err(e) => println!(
            "copie_erreur {e} (après {:.0} ms)",
            duree_copie.as_secs_f64() * 1000.0
        ),
    }
    let fin = ressources();
    println!("prive_fin {}", fin.0);
    println!("prive_pic {}", fin.1);
    println!(
        "pic_moins_repos_mo {:.0}",
        (fin.1.saturating_sub(repos.0)) as f64 / 1_048_576.0
    );
    Ok(())
}
