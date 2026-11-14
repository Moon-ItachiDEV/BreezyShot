//! Fenêtre de contrôle du film (S1 (a) et (d), S5) et lecture d'un fichier dans une vue
//! WebView2 (S2 point 9, S4). Windows seulement.
//!
//!   controle --genre film --etat E --commandes C [--animation 0|1] [--calcul 1] --zone x,y,l,h
//!            --pilule x,y,l,h
//!   controle --genre lecture --etat E --fichier F --type video|gif [--temoin x,y]
//!
//! `film` imite `fenetres::creer_fenetre_outil` comme la fenêtre de la capture défilante
//! (`fenetres/defilante.rs`) : WebView2 transparente sur tout l'écran, toujours au-dessus,
//! sans focus, hors de la barre des tâches, exclue des captures (WDA_EXCLUDEFROMCAPTURE),
//! traversée par les clics (WS_EX_LAYERED | WS_EX_TRANSPARENT) sauf sur la pilule quand la
//! souris y est (surveillance toutes les 30 ms). La page : un cadre autour de la zone, la
//! pilule avec le point rouge qui « respire » (animation sans fin de 1,6 s, coupée par
//! `prefers-reduced-motion: reduce` ou `--animation 0`) et une durée qui change chaque
//! seconde. Le fichier d'état E est réécrit d'un coup ; C : « affinite <n> », « quitter ».
//!
//! `lecture` : une vue WebView2 (le moteur d'Edge, celui de la vignette de l'app) ouvre F
//! par un protocole de la sonde (avec les requêtes partielles « Range ») : pour une vidéo,
//! `duration`, `videoWidth`, images présentées (requestVideoFrameCallback), et la couleur
//! d'une pastille du témoin sur la première image peinte (canvas) ; pour un GIF, la taille
//! décodée et la même couleur.

#[cfg(not(windows))]
fn main() {
    eprintln!("controle : Windows seulement");
}

#[cfg(windows)]
fn main() {
    windows_seul::principal();
}

#[cfg(windows)]
mod windows_seul {
    use sonde_film::{arguments, ecrire_etat, pixels_reels, Rect};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};
    use tao::dpi::{PhysicalPosition, PhysicalSize};
    use tao::event::{Event, StartCause};
    use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopWindowTarget};
    use tao::platform::windows::{WindowBuilderExtWindows, WindowExtWindows};
    use tao::window::{Window, WindowBuilder};
    use windows::Win32::Foundation::{HWND, POINT};
    use windows::Win32::UI::WindowsAndMessaging::*;
    use wry::{WebContext, WebViewBuilder, WebViewBuilderExtWindows};

    /// Arguments de l'app pour ses vues Web (`fenetres::ARGUMENTS_NAVIGATEUR`).
    const ARGUMENTS_NAVIGATEUR: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion --autoplay-policy=no-user-gesture-required";
    const BITS_APP: u32 = WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0;

    struct Config {
        etat: PathBuf,
        commandes: Option<PathBuf>,
        zone: Rect,
        pilule: Rect,
        debut: Instant,
        donnees: PathBuf,
    }

    static CONFIG: OnceLock<Config> = OnceLock::new();
    static MOLETTES: AtomicU32 = AtomicU32::new(0);
    static CLICS: AtomicU32 = AtomicU32::new(0);
    static TOUCHES: AtomicU32 = AtomicU32::new(0);
    static DOM_MS: AtomicU32 = AtomicU32::new(0);
    static SECONDES: AtomicU32 = AtomicU32::new(0);
    static TRAVERSE: AtomicBool = AtomicBool::new(true);
    static QUITTER: AtomicBool = AtomicBool::new(false);
    static REDUIT: Mutex<String> = Mutex::new(String::new());
    static ERREURS: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static LECTURE: Mutex<Vec<String>> = Mutex::new(Vec::new());

    fn noter_erreur(e: String) {
        if let Ok(mut v) = ERREURS.lock() {
            v.push(e.replace('\n', " "));
        }
    }

    fn bureau() -> Rect {
        // SAFETY : lectures de métriques sans effet de bord.
        unsafe {
            Rect {
                x: GetSystemMetrics(SM_XVIRTUALSCREEN),
                y: GetSystemMetrics(SM_YVIRTUALSCREEN),
                l: GetSystemMetrics(SM_CXVIRTUALSCREEN),
                h: GetSystemMetrics(SM_CYVIRTUALSCREEN),
            }
        }
    }

    pub fn principal() {
        pixels_reels();
        let a = arguments();
        let genre = a.get("genre").cloned().unwrap_or_default();
        let Some(etat) = a.get("etat").map(PathBuf::from) else {
            eprintln!("--etat manquant");
            std::process::exit(2);
        };
        let b = bureau();
        let lire = |cle: &str, defaut: Rect| -> Rect {
            a.get(cle)
                .and_then(|t| Rect::lire(t).ok())
                .unwrap_or(defaut)
        };
        let _ = CONFIG.set(Config {
            etat: etat.clone(),
            commandes: a.get("commandes").map(PathBuf::from),
            zone: lire(
                "zone",
                Rect {
                    x: b.x + 100,
                    y: b.y + 100,
                    l: 600,
                    h: 450,
                },
            ),
            pilule: lire(
                "pilule",
                Rect {
                    x: b.x + b.l / 2 - 130,
                    y: b.y + b.h - 120,
                    l: 260,
                    h: 48,
                },
            ),
            debut: Instant::now(),
            donnees: std::env::temp_dir().join(format!(
                "breezyshot-sonde-film-webview2-{}",
                std::process::id()
            )),
        });
        let r = match genre.as_str() {
            "film" => film(
                a.get("animation").map(String::as_str) != Some("0"),
                a.get("calcul").map(String::as_str) == Some("1"),
            ),
            "lecture" => lecture(
                a.get("fichier").cloned().unwrap_or_default(),
                a.get("type").cloned().unwrap_or_else(|| "video".into()),
                a.get("temoin").cloned().unwrap_or_else(|| "0,0".into()),
            ),
            autre => Err(format!("genre inconnu « {autre} »")),
        };
        if let Err(e) = r {
            ecrire_etat(&etat, &[format!("erreur {e}")]);
            std::process::exit(1);
        }
    }

    fn fenetre(
        cible: &EventLoopWindowTarget<String>,
        r: Rect,
        outil: bool,
    ) -> Result<Window, String> {
        WindowBuilder::new()
            .with_title("BreezyShot sonde film")
            .with_position(PhysicalPosition::new(r.x, r.y))
            .with_inner_size(PhysicalSize::new(r.l as u32, r.h as u32))
            .with_visible(false)
            .with_focused(false)
            .with_focusable(!outil)
            .with_decorations(!outil)
            .with_transparent(outil)
            .with_undecorated_shadow(false)
            .with_always_on_top(outil)
            .with_skip_taskbar(outil)
            .with_resizable(false)
            .with_content_protection(outil)
            .build(cible)
            .map_err(|e| format!("fenetre tao: {e}"))
    }

    fn hwnd(f: &Window) -> HWND {
        HWND(f.hwnd() as *mut core::ffi::c_void)
    }

    /// `systeme::traverser_clics`.
    fn traverser(h: HWND, oui: bool) {
        // SAFETY : fenêtre de ce processus ; lecture et écriture de son style étendu.
        unsafe {
            let actuel = GetWindowLongW(h, GWL_EXSTYLE);
            let voulu = if oui {
                actuel | BITS_APP as i32
            } else {
                actuel & !(BITS_APP as i32)
            };
            if voulu != actuel {
                SetWindowLongW(h, GWL_EXSTYLE, voulu);
                let _ = SetWindowPos(
                    h,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );
            }
        }
        TRAVERSE.store(oui, Ordering::SeqCst);
    }

    /// Style « outil » de `systeme::preparer_fenetre_outil`.
    fn style_outil(h: HWND) {
        // SAFETY : idem.
        unsafe {
            let actuel = GetWindowLongW(h, GWL_EXSTYLE);
            let voulu = (actuel & !(WS_EX_APPWINDOW.0 as i32))
                | WS_EX_TOOLWINDOW.0 as i32
                | WS_EX_NOACTIVATE.0 as i32;
            if voulu != actuel {
                SetWindowLongW(h, GWL_EXSTYLE, voulu);
                let _ = SetWindowPos(
                    h,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );
            }
        }
    }

    /// `systeme::exclure_des_captures` (ou l'inverse avec 0) ; rend l'affinité relue.
    fn affinite(h: HWND, valeur: u32) -> u32 {
        let mut lue = u32::MAX;
        // SAFETY : fenêtre de ce processus ; `lue` reçoit un DWORD.
        unsafe {
            if let Err(e) = SetWindowDisplayAffinity(h, WINDOW_DISPLAY_AFFINITY(valeur)) {
                noter_erreur(format!("SetWindowDisplayAffinity({valeur}): {e}"));
            }
            if GetWindowDisplayAffinity(h, &mut lue).is_err() {
                lue = u32::MAX;
            }
        }
        lue
    }

    /// Un tour de surveillance : pilule, commandes, état. Rend faux s'il faut quitter.
    fn tour(h: HWND) -> bool {
        let Some(c) = CONFIG.get() else { return false };
        let mut p = POINT::default();
        // SAFETY : `p` est un POINT valide.
        if unsafe { GetCursorPos(&mut p) }.is_ok() {
            let dedans = c.pilule.contient(p.x, p.y);
            if dedans == TRAVERSE.load(Ordering::SeqCst) {
                traverser(h, !dedans);
            }
        }
        if let Some(chemin) = &c.commandes {
            if let Ok(texte) = std::fs::read_to_string(chemin) {
                let _ = std::fs::remove_file(chemin);
                for ligne in texte.lines() {
                    let mut mots = ligne.split_whitespace();
                    match (mots.next(), mots.next()) {
                        (Some("affinite"), Some(v)) => {
                            if let Ok(v) = v.parse::<u32>() {
                                affinite(h, v);
                            }
                        }
                        (Some("quitter"), _) => QUITTER.store(true, Ordering::SeqCst),
                        _ => {}
                    }
                }
            }
        }
        let mut lue = 0u32;
        // SAFETY : lectures sur une fenêtre de ce processus.
        let (style, visible) = unsafe {
            let _ = GetWindowDisplayAffinity(h, &mut lue);
            (
                GetWindowLongW(h, GWL_EXSTYLE) as u32,
                IsWindowVisible(h).as_bool(),
            )
        };
        let mut lignes = vec![
            format!("hwnd {}", h.0 as isize),
            format!("pid {}", std::process::id()),
            format!("donnees {}", c.donnees.display()),
            format!("dom_ms {}", DOM_MS.load(Ordering::SeqCst)),
            format!("visible {}", visible as u8),
            format!("affinite {lue}"),
            format!("exstyle 0x{style:08X}"),
            format!("traverse {}", TRAVERSE.load(Ordering::SeqCst) as u8),
            format!("molettes {}", MOLETTES.load(Ordering::SeqCst)),
            format!("clics {}", CLICS.load(Ordering::SeqCst)),
            format!("touches {}", TOUCHES.load(Ordering::SeqCst)),
            format!("secondes_page {}", SECONDES.load(Ordering::SeqCst)),
            format!("depuis_ms {}", c.debut.elapsed().as_millis()),
        ];
        if let Ok(r) = REDUIT.lock() {
            if !r.is_empty() {
                lignes.push(format!("reduit {r}"));
            }
        }
        if let Ok(v) = ERREURS.lock() {
            for e in v.iter() {
                lignes.push(format!("erreur {e}"));
            }
        }
        ecrire_etat(&c.etat, &lignes);
        !QUITTER.load(Ordering::SeqCst)
    }

    /// La page du film : cadre de la zone, pilule (point qui respire, durée, deux boutons).
    /// `calcul` : la page calcule exprès (10 ms toutes les 20 ms), pour contrôler que la
    /// mesure du processeur des processus WebView2 voit bien le travail de la page.
    fn page_film(b: Rect, zone: Rect, pilule: Rect, animation: bool, calcul: bool) -> String {
        format!(
            r#"<!doctype html><html><head><meta charset="utf-8"><style>
html,body{{margin:0;height:100%;overflow:hidden;background:transparent;font:14px "Segoe UI",sans-serif}}
#zone{{position:fixed;left:{zx}px;top:{zy}px;width:{zl}px;height:{zh}px;outline:2px solid #f5c518;outline-offset:2px}}
#pilule{{position:fixed;left:{px}px;top:{py}px;width:{pl}px;height:{ph}px;border-radius:{rr}px;
background:rgba(28,28,30,.92);color:#fff;display:flex;align-items:center;gap:12px;padding:0 16px;box-sizing:border-box}}
.point{{width:10px;height:10px;border-radius:50%;background:#ff3b30;animation:respire 1.6s ease-in-out infinite}}
.sans .point{{animation:none}}
@keyframes respire{{0%,100%{{transform:scale(1);opacity:1}}50%{{transform:scale(.55);opacity:.35}}}}
@media (prefers-reduced-motion: reduce){{.point{{animation:none}}}}
#duree{{font-variant-numeric:tabular-nums;min-width:44px}}
.bouton{{padding:4px 10px;border-radius:12px;background:rgba(255,255,255,.14)}}
</style></head><body class="{classe}"><div id="zone"></div>
<div id="pilule"><div class="point"></div><div id="duree">0:00</div><div class="bouton">Pause</div><div class="bouton">Arrêter</div></div>
<script>
var d0=Date.now();
setInterval(function(){{var s=Math.floor((Date.now()-d0)/1000);
document.getElementById('duree').textContent=Math.floor(s/60)+':'+String(s%60).padStart(2,'0');
window.ipc.postMessage('seconde '+s);}},1000);
document.addEventListener('wheel',function(){{window.ipc.postMessage('molette');}},{{passive:true}});
document.addEventListener('mousedown',function(){{window.ipc.postMessage('clic');}});
document.addEventListener('keydown',function(){{window.ipc.postMessage('touche');}});
window.ipc.postMessage('reduit '+(matchMedia('(prefers-reduced-motion: reduce)').matches?1:0));
{calcul}
window.ipc.postMessage('dom');
</script></body></html>"#,
            zx = zone.x - b.x - 2,
            zy = zone.y - b.y - 2,
            zl = zone.l + 4,
            zh = zone.h + 4,
            px = pilule.x - b.x,
            py = pilule.y - b.y,
            pl = pilule.l,
            ph = pilule.h,
            rr = pilule.h / 2,
            classe = if animation { "" } else { "sans" },
            calcul = if calcul {
                "var x=0;setInterval(function(){var f=performance.now()+10;while(performance.now()<f){x=(x*1103515245+12345)%2147483648;}},20);"
            } else {
                ""
            },
        )
    }

    fn film(animation: bool, calcul: bool) -> Result<(), String> {
        let c = CONFIG.get().ok_or("config")?;
        let b = bureau();
        let boucle = EventLoopBuilder::<String>::with_user_event().build();
        let proxy = boucle.create_proxy();
        let mut contexte = WebContext::new(Some(c.donnees.clone()));
        let f = fenetre(&boucle, b, true)?;
        let h = hwnd(&f);
        style_outil(h);
        affinite(h, WDA_EXCLUDEFROMCAPTURE.0);
        traverser(h, true);
        let v = WebViewBuilder::new_with_web_context(&mut contexte)
            .with_transparent(true)
            .with_focused(false)
            .with_html(page_film(b, c.zone, c.pilule, animation, calcul))
            .with_ipc_handler(move |req| {
                let _ = proxy.send_event(req.body().clone());
            })
            .with_additional_browser_args(ARGUMENTS_NAVIGATEUR)
            .build(&f)
            .map_err(|e| format!("webview: {e}"))?;
        let debut = c.debut;
        tour(h);
        let mut garde = Some((f, v, contexte));
        boucle.run(move |evenement, _, flot| {
            *flot = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(30));
            match evenement {
                Event::UserEvent(m) => {
                    if m == "dom" {
                        DOM_MS.store(debut.elapsed().as_millis().max(1) as u32, Ordering::SeqCst);
                        // Montrée sans activer, comme `montrer_sans_activer`.
                        // SAFETY : notre fenêtre.
                        unsafe {
                            let _ = ShowWindow(h, SW_SHOWNOACTIVATE);
                            let _ = SetWindowPos(
                                h,
                                Some(HWND_TOPMOST),
                                0,
                                0,
                                0,
                                0,
                                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                            );
                        }
                    } else if let Some(r) = m.strip_prefix("reduit ") {
                        if let Ok(mut x) = REDUIT.lock() {
                            *x = r.to_string();
                        }
                    } else if let Some(s) = m.strip_prefix("seconde ") {
                        SECONDES.store(s.parse().unwrap_or(0), Ordering::SeqCst);
                    } else if m == "molette" {
                        MOLETTES.fetch_add(1, Ordering::SeqCst);
                    } else if m == "clic" {
                        CLICS.fetch_add(1, Ordering::SeqCst);
                    } else if m == "touche" {
                        TOUCHES.fetch_add(1, Ordering::SeqCst);
                    }
                }
                Event::NewEvents(StartCause::ResumeTimeReached { .. }) if !tour(h) => {
                    garde.take();
                    *flot = ControlFlow::Exit;
                }
                _ => {}
            }
        });
    }

    // -----------------------------------------------------------------------------------
    // Lecture d'un fichier dans WebView2

    /// Réponse à une requête, avec « Range: bytes=a-b » (206) si elle est demandée.
    fn repondre(
        corps: &[u8],
        genre: &str,
        range: Option<&str>,
    ) -> wry::http::Response<std::borrow::Cow<'static, [u8]>> {
        let total = corps.len();
        let plage = range
            .and_then(|r| r.strip_prefix("bytes="))
            .and_then(|r| r.split_once('-'))
            .and_then(|(a, b)| {
                let a: usize = a.trim().parse().ok()?;
                let b: usize = if b.trim().is_empty() {
                    total.saturating_sub(1)
                } else {
                    b.trim().parse().ok()?
                };
                (a < total).then_some((a, b.min(total.saturating_sub(1))))
            });
        let r = wry::http::Response::builder()
            .header("Content-Type", genre)
            .header("Accept-Ranges", "bytes")
            .header("Access-Control-Allow-Origin", "*");
        let r = match plage {
            Some((a, b)) => r
                .status(206)
                .header("Content-Range", format!("bytes {a}-{b}/{total}"))
                .header("Content-Length", (b + 1 - a).to_string())
                .body(std::borrow::Cow::Owned(corps[a..=b].to_vec())),
            None => r
                .header("Content-Length", total.to_string())
                .body(std::borrow::Cow::Owned(corps.to_vec())),
        };
        r.unwrap_or_else(|_| wry::http::Response::new(std::borrow::Cow::Borrowed(&[][..])))
    }

    fn lecture(fichier: String, genre: String, temoin: String) -> Result<(), String> {
        let c = CONFIG.get().ok_or("config")?;
        let etat = c.etat.clone();
        let octets = std::fs::read(&fichier).map_err(|e| format!("{fichier}: {e}"))?;
        let (tx, ty) = temoin
            .split_once(',')
            .and_then(|(a, b)| Some((a.trim().parse::<i32>().ok()?, b.trim().parse::<i32>().ok()?)))
            .unwrap_or((0, 0));
        // Centre de la première pastille du témoin (coordonnées de l'image).
        let (px, py) = (
            tx + sonde_film::temoin::PASTILLES_X[0] + sonde_film::temoin::PASTILLE / 2,
            ty + sonde_film::temoin::PASTILLE_Y + sonde_film::temoin::PASTILLE / 2,
        );
        let video = genre == "video";
        let page = format!(
            r#"<!doctype html><html><head><meta charset="utf-8"></head><body style="margin:0;background:#222">
<div id="v"></div><script>
function dire(m){{window.ipc.postMessage(m);}}
function pixel(src,w,h){{
  try{{var c=document.createElement('canvas');c.width=w;c.height=h;var x=c.getContext('2d');x.drawImage(src,0,0);
  var p=x.getImageData({px},{py},1,1).data;return p[0]+','+p[1]+','+p[2]+','+p[3];}}catch(e){{return 'erreur_'+String(e).replace(/\s+/g,'_');}}
}}
var fini=false;function fin(){{if(!fini){{fini=true;dire('fin');}}}}
setTimeout(function(){{dire('delai 15 s dépassé');fin();}},15000);
if({video}){{
  var v=document.createElement('video');v.muted=true;v.autoplay=true;v.playsInline=true;v.src='http://sonde.localhost/fichier';
  document.getElementById('v').appendChild(v);
  v.addEventListener('loadedmetadata',function(){{dire('duree_s '+v.duration.toFixed(3));dire('taille '+v.videoWidth+'x'+v.videoHeight);}});
  v.addEventListener('error',function(){{dire('erreur_media '+(v.error?v.error.code+' '+v.error.message:'?'));fin();}});
  var presentees=0,premiere=true;
  function image(t,m){{presentees=m.presentedFrames;if(premiere){{premiere=false;dire('premiere_image_ms '+performance.now().toFixed(0));dire('pastille '+pixel(v,v.videoWidth,v.videoHeight));
    setTimeout(function(){{dire('images_presentees_2s '+presentees);fin();}},2000);}}v.requestVideoFrameCallback(image);}}
  if(v.requestVideoFrameCallback){{v.requestVideoFrameCallback(image);}}else{{dire('requestVideoFrameCallback absent');}}
  v.play().catch(function(e){{dire('play_refuse '+String(e).replace(/\s+/g,'_'));}});
}}else{{
  var i=new Image();i.src='http://sonde.localhost/fichier';
  i.decode().then(function(){{dire('taille '+i.naturalWidth+'x'+i.naturalHeight);dire('pastille '+pixel(i,i.naturalWidth,i.naturalHeight));document.getElementById('v').appendChild(i);fin();}})
   .catch(function(e){{dire('erreur_decode '+String(e).replace(/\s+/g,'_'));fin();}});
}}
</script></body></html>"#,
            video = video
        );
        let type_fichier = if video { "video/mp4" } else { "image/gif" };
        let page_octets = page.into_bytes();
        let boucle = EventLoopBuilder::<String>::with_user_event().build();
        let proxy = boucle.create_proxy();
        let mut contexte = WebContext::new(Some(c.donnees.clone()));
        let f = fenetre(
            &boucle,
            Rect {
                x: 0,
                y: 0,
                l: 800,
                h: 640,
            },
            false,
        )?;
        let v = WebViewBuilder::new_with_web_context(&mut contexte)
            .with_custom_protocol("sonde".into(), move |_id, req| {
                let range = req
                    .headers()
                    .get("range")
                    .and_then(|x| x.to_str().ok())
                    .map(str::to_string);
                if req.uri().path() == "/fichier" {
                    repondre(&octets, type_fichier, range.as_deref())
                } else {
                    repondre(&page_octets, "text/html", None)
                }
            })
            .with_url("http://sonde.localhost/")
            .with_ipc_handler(move |req| {
                let _ = proxy.send_event(req.body().clone());
            })
            .with_additional_browser_args(ARGUMENTS_NAVIGATEUR)
            .build(&f)
            .map_err(|e| format!("webview: {e}"))?;
        f.set_visible(true);
        let limite = Instant::now() + Duration::from_secs(40);
        let mut garde = Some((f, v, contexte));
        boucle.run(move |evenement, _, flot| {
            *flot = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100));
            let mut sortir = Instant::now() > limite;
            if let Event::UserEvent(m) = evenement {
                if let Ok(mut l) = LECTURE.lock() {
                    if m == "fin" {
                        sortir = true;
                    } else {
                        l.push(m);
                    }
                    ecrire_etat(&etat, &l);
                }
            }
            if sortir {
                if let Ok(mut l) = LECTURE.lock() {
                    l.push("fin 1".into());
                    ecrire_etat(&etat, &l);
                }
                garde.take();
                *flot = ControlFlow::Exit;
            }
        });
    }
}
