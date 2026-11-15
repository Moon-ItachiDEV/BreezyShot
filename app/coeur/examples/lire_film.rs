//! Le lecteur MP4 de l'essai 15 (contrat 0.7, chantier B, F31, risque 22) : relit un film
//! par `systeme::mp4::lire` (un `IMFSourceReader`, derrière la garde F31 : sans Media
//! Foundation, rien n'est appelé et la réponse dit « media foundation absente »), et écrit
//! en JSON la durée, la taille, le nombre d'images, la forme du fichier, la piste son, la
//! géométrie du décodage, puis pour chaque image son temps et les couleurs des points
//! demandés ; en PNG, les images choisies.
//!
//! ```text
//! lire_film <film.mp4> [--points "x,y;x,y;…"] [--png <dossier> --images 0,1,150]
//!           [--sortie <fichier.json>]
//! ```
//!
//! Le lecteur suit les changements de type que `ReadSample` annonce, la largeur de ligne
//! réelle du tampon et la zone d'affichage (600 × 450 est décodé dans un tampon de
//! 608 × 464, ligne de 2 432 octets, zone 0, 0, 600 × 450 : mesuré par la sonde, D28).
//! Une couleur se lit au milieu d'un aplat, à 4 px au moins d'un bord (le 4:2:0 donne des
//! écarts de plusieurs dizaines sur un bord).
//!
//! Sortie : code 0 et `"ok": true` ; sinon code 1 et `"ok": false, "erreur": "…"`. Aucun
//! chemin dans les erreurs du lecteur ; jamais de panique sur un fichier faux.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use breezyshot::systeme::gel::Pixels;
use breezyshot::systeme::mp4;
use serde_json::{json, Value};

struct Options {
    film: PathBuf,
    points: Vec<(u32, u32)>,
    png: Option<PathBuf>,
    images: Vec<u64>,
    sortie: Option<PathBuf>,
}

fn options(arguments: &[String]) -> Result<Options, String> {
    let mut film = None;
    let mut points = Vec::new();
    let mut png = None;
    let mut images = Vec::new();
    let mut sortie = None;
    let mut i = 0;
    while i < arguments.len() {
        let a = arguments[i].as_str();
        let valeur = || {
            arguments
                .get(i + 1)
                .cloned()
                .ok_or_else(|| format!("{a} sans valeur"))
        };
        match a {
            "--points" => {
                points = lire_points(&valeur()?)?;
                i += 1;
            }
            "--png" => {
                png = Some(PathBuf::from(valeur()?));
                i += 1;
            }
            "--images" => {
                images = lire_nombres(&valeur()?)?;
                i += 1;
            }
            "--sortie" => {
                sortie = Some(PathBuf::from(valeur()?));
                i += 1;
            }
            _ if a.starts_with("--") => return Err(format!("option inconnue {a}")),
            _ if film.is_none() => film = Some(PathBuf::from(a)),
            _ => return Err("un seul film".into()),
        }
        i += 1;
    }
    if !images.is_empty() && png.is_none() {
        return Err("--images sans --png".into());
    }
    Ok(Options {
        film: film.ok_or("film manquant")?,
        points,
        png,
        images,
        sortie,
    })
}

/// « 10,20;300,40 » → [(10, 20), (300, 40)].
fn lire_points(texte: &str) -> Result<Vec<(u32, u32)>, String> {
    texte
        .split(';')
        .filter(|p| !p.trim().is_empty())
        .map(|p| {
            let (x, y) = p.split_once(',').ok_or_else(|| format!("point « {p} »"))?;
            let n = |s: &str| {
                s.trim()
                    .parse::<u32>()
                    .map_err(|_| format!("point « {p} »"))
            };
            Ok((n(x)?, n(y)?))
        })
        .collect()
}

/// « 0,1,150 » → [0, 1, 150].
fn lire_nombres(texte: &str) -> Result<Vec<u64>, String> {
    texte
        .split(',')
        .filter(|n| !n.trim().is_empty())
        .map(|n| n.trim().parse().map_err(|_| format!("image « {n} »")))
        .collect()
}

/// La forme du fichier : `mp4` si sa première boîte est `ftyp`.
fn conteneur(film: &Path) -> &'static str {
    use std::io::Read;
    let mut tete = [0u8; 8];
    let lu = std::fs::File::open(film).and_then(|mut f| f.read_exact(&mut tete));
    if lu.is_ok() && &tete[4..8] == b"ftyp" {
        "mp4"
    } else {
        "inconnu"
    }
}

fn millis(d: Duration) -> f64 {
    (d.as_secs_f64() * 1e6).round() / 1e3
}

fn couleur(img: &Pixels, (x, y): (u32, u32)) -> Value {
    if x >= img.largeur || y >= img.hauteur {
        return Value::Null;
    }
    let i = ((y as usize) * (img.largeur as usize) + x as usize) * 4;
    match img.rgba.get(i..i + 3) {
        Some(c) => json!([c[0], c[1], c[2]]),
        None => Value::Null,
    }
}

fn ecrire_png(chemin: &Path, img: &Pixels) -> Result<(), String> {
    let f = std::fs::File::create(chemin).map_err(|e| format!("png: {e}"))?;
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), img.largeur, img.hauteur);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    let mut w = e.write_header().map_err(|e| format!("png: {e}"))?;
    w.write_image_data(&img.rgba)
        .map_err(|e| format!("png: {e}"))
}

fn lire(o: &Options) -> Result<Value, String> {
    if let Some(d) = &o.png {
        std::fs::create_dir_all(d).map_err(|e| format!("dossier des png: {e}"))?;
    }
    let mut detail = Vec::new();
    let mut pngs = Vec::new();
    let mut faute_png = None;
    let lecture = mp4::lire(&o.film, |i, t, img| {
        detail.push(json!({
            "i": i,
            "t_ms": millis(t),
            "couleurs": o.points.iter().map(|&p| couleur(img, p)).collect::<Vec<_>>(),
        }));
        if let Some(d) = o.png.as_ref().filter(|_| o.images.contains(&i)) {
            let f = d.join(format!("image-{i}.png"));
            match ecrire_png(&f, img) {
                Ok(()) => pngs.push(json!({ "i": i, "fichier": f.display().to_string() })),
                Err(e) => faute_png = faute_png.take().or(Some(e)),
            }
        }
    })?;
    if let Some(e) = faute_png {
        return Err(e);
    }
    Ok(json!({
        "ok": true,
        "conteneur": conteneur(&o.film),
        "video": if lecture.h264 { "h264" } else { "autre" },
        "son": lecture.son,
        "duree_ms": millis(lecture.duree),
        "largeur": lecture.largeur,
        "hauteur": lecture.hauteur,
        "images": lecture.images,
        "dernier_ms": lecture.dernier_instant.map(millis),
        "geometrie": {
            "tampon": [lecture.tampon_largeur, lecture.tampon_hauteur],
            "ligne_octets": lecture.pas_ligne,
            "zone": [lecture.zone_x, lecture.zone_y, lecture.largeur, lecture.hauteur],
            "changements_de_type": lecture.changements_de_type,
        },
        "points": o.points.iter().map(|&(x, y)| json!([x, y])).collect::<Vec<_>>(),
        "detail": detail,
        "png": pngs,
    }))
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let (resultat, sortie) = match options(&arguments) {
        Ok(o) => (lire(&o), o.sortie),
        Err(e) => (Err(e), None),
    };
    let (code, valeur) = match resultat {
        Ok(v) => (ExitCode::SUCCESS, v),
        Err(e) => (ExitCode::FAILURE, json!({ "ok": false, "erreur": e })),
    };
    let texte = valeur.to_string();
    match sortie {
        Some(f) => {
            if let Err(e) = std::fs::write(&f, texte) {
                eprintln!("lire_film: sortie non ecrite: {e}");
                return ExitCode::FAILURE;
            }
        }
        None => println!("{texte}"),
    }
    code
}
