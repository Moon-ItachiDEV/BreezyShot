//! Fenêtre « de contrôle » de la sonde défilante (S9, S2 (a), S7 aperçu). Windows seulement.
//!
//! Imite `fenetres::creer_fenetre_outil` de l'app : toujours au-dessus, sans focus, hors de
//! la barre des tâches, exclue des captures (`WDA_EXCLUDEFROMCAPTURE`), traversée par les
//! clics (`WS_EX_LAYERED | WS_EX_TRANSPARENT`) sauf sur la « pilule » quand la souris y est
//! (surveillance toutes les 30 ms, comme le bandeau).
//!
//!   controle --genre natif   --etat E --commandes C --zone x,y,l,h --pilule x,y,l,h
//!   controle --genre webview --etat E --commandes C --zone x,y,l,h --pilule x,y,l,h
//!   controle --genre creations --etat E --nombre 20 [--sans-gardien]
//!   controle --genre apercu --etat E --images a.png;b.png
//!
//! Le fichier d'état E est réécrit d'un coup (temporaire puis renommage), une ligne
//! « cle valeur » par mesure. Le fichier de commandes C (écrit par la sonde de la même
//! façon) est lu puis supprimé : « affinite <n> », « quitter ».
//! Le voile est visible à l'écran (bleu sombre, opacité 35 %) sauf sur la zone, la pilule
//! est blanche : sans l'exclusion, une copie d'écran les verrait.

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
    use sonde_defilante::{arguments, ecrire_etat, pixels_reels, Rect};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::{Duration, Instant};
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, PAINTSTRUCT,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::*;

    /// Arguments de l'app pour ses vues Web (`fenetres::ARGUMENTS_NAVIGATEUR`).
    const ARGUMENTS_NAVIGATEUR: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion --autoplay-policy=no-user-gesture-required";

    struct Config {
        etat: PathBuf,
        commandes: Option<PathBuf>,
        zone: Rect,
        pilule: Rect,
        debut: Instant,
    }

    static CONFIG: OnceLock<Config> = OnceLock::new();
    static MOLETTES: AtomicU32 = AtomicU32::new(0);
    static MOLETTES_PILULE: AtomicU32 = AtomicU32::new(0);
    static COMMANDES: AtomicU32 = AtomicU32::new(0);
    static PRET_MS: AtomicU32 = AtomicU32::new(0);
    static DOM_MS: AtomicU32 = AtomicU32::new(0);
    static TRAVERSE: AtomicBool = AtomicBool::new(true);
    static QUITTER: AtomicBool = AtomicBool::new(false);
    static ERREURS: Mutex<Vec<String>> = Mutex::new(Vec::new());

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
        let lire = |cle: &str, defaut: Rect| -> Rect {
            a.get(cle)
                .and_then(|t| Rect::lire(t).ok())
                .unwrap_or(defaut)
        };
        let b = bureau();
        let config = Config {
            etat: etat.clone(),
            commandes: a.get("commandes").map(PathBuf::from),
            zone: lire(
                "zone",
                Rect {
                    x: b.x + 100,
                    y: b.y + 100,
                    l: 400,
                    h: 300,
                },
            ),
            pilule: lire(
                "pilule",
                Rect {
                    x: b.x + 10,
                    y: b.y + 10,
                    l: 160,
                    h: 40,
                },
            ),
            debut: Instant::now(),
        };
        let _ = CONFIG.set(config);
        let resultat = match genre.as_str() {
            "natif" => natif(),
            "webview" => webview::controle(),
            "creations" => webview::creations(
                a.get("nombre").and_then(|n| n.parse().ok()).unwrap_or(20),
                a.contains_key("sans-gardien"),
            ),
            "apercu" => webview::apercu(a.get("images").cloned().unwrap_or_default()),
            autre => Err(format!("genre inconnu « {autre} »")),
        };
        if let Err(e) = resultat {
            ecrire_etat(&etat, &[format!("erreur {e}")]);
            std::process::exit(1);
        }
    }

    // -----------------------------------------------------------------------------------
    // Ce qui est commun aux deux fenêtres de contrôle.

    /// Pose (ou retire) les bits qui laissent passer souris et molette. La variante native
    /// ne garde que `WS_EX_TRANSPARENT` (retirer `WS_EX_LAYERED` la rendrait opaque) ; la
    /// variante WebView2 fait exactement comme `systeme::traverser_clics`.
    fn traverser(h: HWND, traverser: bool, bits: u32) {
        // SAFETY : fenêtre de ce processus ; lecture et écriture de son style étendu.
        unsafe {
            let actuel = GetWindowLongW(h, GWL_EXSTYLE);
            let voulu = if traverser {
                actuel | bits as i32
            } else {
                actuel & !(bits as i32)
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
        TRAVERSE.store(traverser, Ordering::SeqCst);
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

    fn souris() -> Option<(i32, i32)> {
        let mut p = POINT::default();
        // SAFETY : `p` est un POINT valide.
        unsafe { GetCursorPos(&mut p) }.ok().map(|_| (p.x, p.y))
    }

    /// Un tour de surveillance : pilule, commandes, état. Rend faux s'il faut quitter.
    fn tour(h: HWND, bits: u32) -> bool {
        let Some(c) = CONFIG.get() else { return false };
        if let Some((x, y)) = souris() {
            let dedans = c.pilule.contient(x, y);
            if dedans == TRAVERSE.load(Ordering::SeqCst) {
                traverser(h, !dedans, bits);
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
                    COMMANDES.fetch_add(1, Ordering::SeqCst);
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
            format!("dom_ms {}", DOM_MS.load(Ordering::SeqCst)),
            format!("pret_ms {}", PRET_MS.load(Ordering::SeqCst)),
            format!("visible {}", visible as u8),
            format!("affinite {lue}"),
            format!("exstyle 0x{style:08X}"),
            format!("traverse {}", TRAVERSE.load(Ordering::SeqCst) as u8),
            format!("molettes {}", MOLETTES.load(Ordering::SeqCst)),
            format!("molettes_pilule {}", MOLETTES_PILULE.load(Ordering::SeqCst)),
            format!("commandes {}", COMMANDES.load(Ordering::SeqCst)),
            format!("depuis_ms {}", c.debut.elapsed().as_millis()),
        ];
        if let Ok(v) = ERREURS.lock() {
            for e in v.iter() {
                lignes.push(format!("erreur {e}"));
            }
        }
        ecrire_etat(&c.etat, &lignes);
        !QUITTER.load(Ordering::SeqCst)
    }

    // -----------------------------------------------------------------------------------
    // Variante native : une fenêtre Win32 en couches (couleur clé pour la zone).

    const CLE: COLORREF = COLORREF(0x00FF00FF); // magenta : trou de la zone
    const VOILE: COLORREF = COLORREF(0x00503C1E); // bleu sombre (BGR)
    const BLANC: COLORREF = COLORREF(0x00FFFFFF);

    fn local(r: Rect, origine: Rect) -> RECT {
        RECT {
            left: r.x - origine.x,
            top: r.y - origine.y,
            right: r.x - origine.x + r.l,
            bottom: r.y - origine.y + r.h,
        }
    }

    unsafe extern "system" fn procedure(h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        match msg {
            WM_MOUSEWHEEL => {
                MOLETTES.fetch_add(1, Ordering::SeqCst);
                if let (Some(c), Some((x, y))) = (CONFIG.get(), souris()) {
                    if c.pilule.contient(x, y) {
                        MOLETTES_PILULE.fetch_add(1, Ordering::SeqCst);
                    }
                }
                LRESULT(0)
            }
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            WM_ERASEBKGND => LRESULT(1),
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                // SAFETY : peinture de notre fenêtre entre BeginPaint et EndPaint.
                unsafe {
                    let dc = BeginPaint(h, &mut ps);
                    let b = bureau();
                    let tout = RECT {
                        left: 0,
                        top: 0,
                        right: b.l,
                        bottom: b.h,
                    };
                    let voile = CreateSolidBrush(VOILE);
                    let cle = CreateSolidBrush(CLE);
                    let blanc = CreateSolidBrush(BLANC);
                    FillRect(dc, &tout, voile);
                    if let Some(c) = CONFIG.get() {
                        FillRect(dc, &local(c.zone, b), cle);
                        FillRect(dc, &local(c.pilule, b), blanc);
                    }
                    let _ = DeleteObject(voile.into());
                    let _ = DeleteObject(cle.into());
                    let _ = DeleteObject(blanc.into());
                    let _ = EndPaint(h, &ps);
                }
                LRESULT(0)
            }
            WM_TIMER => {
                if !tour(h, WS_EX_TRANSPARENT.0) {
                    // SAFETY : notre fenêtre.
                    unsafe {
                        let _ = DestroyWindow(h);
                    }
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                // SAFETY : fin de la boucle de ce fil.
                unsafe { PostQuitMessage(0) };
                LRESULT(0)
            }
            // SAFETY : traitement par défaut.
            _ => unsafe { DefWindowProcW(h, msg, w, l) },
        }
    }

    fn natif() -> Result<(), String> {
        let debut = Instant::now();
        let b = bureau();
        // SAFETY : création et boucle d'une fenêtre de ce fil ; tout est détruit à la fin.
        unsafe {
            let instance =
                GetModuleHandleW(PCWSTR::null()).map_err(|e| format!("GetModuleHandleW: {e}"))?;
            let classe = WNDCLASSW {
                lpfnWndProc: Some(procedure),
                hInstance: instance.into(),
                lpszClassName: w!("BreezyShotSondeControle"),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                ..Default::default()
            };
            if RegisterClassW(&classe) == 0 {
                return Err("RegisterClassW a échoué".into());
            }
            let h = CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_TOPMOST
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE,
                w!("BreezyShotSondeControle"),
                w!("BreezyShot sonde controle"),
                WS_POPUP,
                b.x,
                b.y,
                b.l,
                b.h,
                None,
                None,
                Some(instance.into()),
                None,
            )
            .map_err(|e| format!("CreateWindowExW: {e}"))?;
            SetLayeredWindowAttributes(h, CLE, 89, LWA_ALPHA | LWA_COLORKEY)
                .map_err(|e| format!("SetLayeredWindowAttributes: {e}"))?;
            affinite(h, WDA_EXCLUDEFROMCAPTURE.0);
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
            PRET_MS.store(debut.elapsed().as_millis().max(1) as u32, Ordering::SeqCst);
            TRAVERSE.store(true, Ordering::SeqCst);
            tour(h, WS_EX_TRANSPARENT.0);
            SetTimer(Some(h), 1, 30, None);
            let mut m = MSG::default();
            while GetMessageW(&mut m, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&m);
                DispatchMessageW(&m);
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------------------
    // Variante WebView2 (tao + wry, mêmes réglages que `creer_fenetre_outil`).

    pub mod webview {
        use super::*;
        use tao::dpi::{PhysicalPosition, PhysicalSize};
        use tao::event::{Event, StartCause};
        use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopWindowTarget};
        use tao::platform::windows::{WindowBuilderExtWindows, WindowExtWindows};
        use tao::window::{Window, WindowBuilder};
        use wry::{WebContext, WebView, WebViewBuilder, WebViewBuilderExtWindows};

        fn dossier_donnees() -> PathBuf {
            std::env::temp_dir().join(format!("breezyshot-sonde-webview2-{}", std::process::id()))
        }

        fn fenetre(
            cible: &EventLoopWindowTarget<String>,
            r: Rect,
            dessus: bool,
        ) -> Result<Window, String> {
            WindowBuilder::new()
                .with_title("BreezyShot sonde controle")
                .with_position(PhysicalPosition::new(r.x, r.y))
                .with_inner_size(PhysicalSize::new(r.l as u32, r.h as u32))
                .with_visible(false)
                .with_focused(false)
                .with_focusable(false)
                .with_decorations(false)
                .with_transparent(true)
                .with_undecorated_shadow(false)
                .with_always_on_top(dessus)
                .with_skip_taskbar(true)
                .with_resizable(false)
                .with_content_protection(dessus)
                .build(cible)
                .map_err(|e| format!("fenetre tao: {e}"))
        }

        fn vue(
            contexte: &mut WebContext,
            f: &Window,
            page: String,
            proxy: tao::event_loop::EventLoopProxy<String>,
            transparente: bool,
        ) -> Result<WebView, String> {
            WebViewBuilder::new_with_web_context(contexte)
                .with_transparent(transparente)
                .with_focused(false)
                .with_html(page)
                .with_ipc_handler(move |req| {
                    let _ = proxy.send_event(req.body().clone());
                })
                .with_additional_browser_args(ARGUMENTS_NAVIGATEUR)
                .build(f)
                .map_err(|e| format!("webview: {e}"))
        }

        fn hwnd(f: &Window) -> HWND {
            HWND(f.hwnd() as *mut core::ffi::c_void)
        }

        /// La page de contrôle : voile partout sauf la zone (ombre d'un cadre posé sur la
        /// zone), pilule blanche ; chaque molette reçue est dite à Rust.
        fn page_controle(b: Rect, zone: Rect, pilule: Rect) -> String {
            format!(
                r#"<!doctype html><html><head><meta charset="utf-8"><style>
html,body{{margin:0;height:100%;overflow:hidden;background:transparent}}
#zone{{position:fixed;left:{zx}px;top:{zy}px;width:{zl}px;height:{zh}px;
box-shadow:0 0 0 20000px rgba(30,60,80,.35);outline:2px solid #3b82f6}}
#pilule{{position:fixed;left:{px}px;top:{py}px;width:{pl}px;height:{ph}px;background:#fff;border-radius:{rr}px}}
</style></head><body><div id="zone"></div><div id="pilule"></div><script>
document.addEventListener('wheel',function(e){{
  var p=document.getElementById('pilule').getBoundingClientRect();
  var sur=e.clientX>=p.left&&e.clientX<p.right&&e.clientY>=p.top&&e.clientY<p.bottom;
  window.ipc.postMessage('molette '+(sur?1:0));
}},{{passive:true}});
window.ipc.postMessage('dom');
requestAnimationFrame(function(){{requestAnimationFrame(function(){{window.ipc.postMessage('pret');}});}});
</script></body></html>"#,
                zx = zone.x - b.x,
                zy = zone.y - b.y,
                zl = zone.l,
                zh = zone.h,
                px = pilule.x - b.x,
                py = pilule.y - b.y,
                pl = pilule.l,
                ph = pilule.h,
                rr = pilule.h / 2,
            )
        }

        const BITS_APP: u32 = WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0;

        pub fn controle() -> Result<(), String> {
            let c = CONFIG.get().ok_or("config")?;
            let b = bureau();
            let boucle = EventLoopBuilder::<String>::with_user_event().build();
            let proxy = boucle.create_proxy();
            let mut contexte = WebContext::new(Some(dossier_donnees()));
            let f = fenetre(&boucle, b, true)?;
            let h = hwnd(&f);
            style_outil(h);
            affinite(h, WDA_EXCLUDEFROMCAPTURE.0);
            traverser(h, true, BITS_APP);
            let v = vue(
                &mut contexte,
                &f,
                page_controle(b, c.zone, c.pilule),
                proxy,
                true,
            )?;
            let debut = c.debut;
            tour(h, BITS_APP);
            let mut garde = Some((f, v, contexte));
            boucle.run(move |evenement, _, flot| {
                *flot = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(30));
                match evenement {
                    Event::UserEvent(m) => {
                        if m == "dom" {
                            // La page a répondu : montrée comme `montrer_sans_activer`.
                            DOM_MS
                                .store(debut.elapsed().as_millis().max(1) as u32, Ordering::SeqCst);
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
                        } else if m == "pret" {
                            PRET_MS
                                .store(debut.elapsed().as_millis().max(1) as u32, Ordering::SeqCst);
                        } else if let Some(sur) = m.strip_prefix("molette ") {
                            MOLETTES.fetch_add(1, Ordering::SeqCst);
                            if sur == "1" {
                                MOLETTES_PILULE.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                    }
                    Event::NewEvents(StartCause::ResumeTimeReached { .. })
                        if !tour(h, BITS_APP) =>
                    {
                        garde.take();
                        *flot = ControlFlow::Exit;
                    }
                    _ => {}
                }
            });
        }

        /// S9 (c) : `nombre` créations de suite d'une fenêtre de contrôle cachée, du début
        /// de la création à la page qui répond (« dom » : son script a tourné, c'est le
        /// `defilante_pret` de l'app) et à deux images dessinées (« pret » : peut ne jamais
        /// venir tant que la fenêtre est cachée). La première est à froid.
        /// Avec un gardien (par défaut), la première fenêtre reste vivante, cachée, pendant
        /// les suivantes (comme une app qui a déjà une vue Web) ; sans gardien, chaque
        /// fenêtre est détruite avant la suivante. Un fil regarde toutes les 2 ms si la
        /// fenêtre en création devient visible avant d'être prête.
        pub fn creations(nombre: usize, sans_gardien: bool) -> Result<(), String> {
            let c = CONFIG.get().ok_or("config")?;
            let etat = c.etat.clone();
            let lancement = c.debut;
            let b = bureau();
            let boucle = EventLoopBuilder::<String>::with_user_event().build();
            let proxy = boucle.create_proxy();
            let courant = Arc::new(AtomicIsize::new(0));
            let vue_visible = Arc::new(AtomicBool::new(false));
            {
                let courant = Arc::clone(&courant);
                let vue_visible = Arc::clone(&vue_visible);
                std::thread::spawn(move || loop {
                    let h = courant.load(Ordering::SeqCst);
                    if h != 0 {
                        // SAFETY : simple lecture, possible depuis n'importe quel fil.
                        if unsafe { IsWindowVisible(HWND(h as *mut core::ffi::c_void)) }.as_bool() {
                            vue_visible.store(true, Ordering::SeqCst);
                        }
                    }
                    std::thread::sleep(Duration::from_millis(2));
                });
            }
            let mut contexte = WebContext::new(Some(dossier_donnees()));
            let mut lignes: Vec<String> = vec![format!("gardien {}", (!sans_gardien) as u8)];
            let mut gardien: Option<(Window, WebView)> = None;
            let mut en_cours: Option<(Window, WebView, Instant, usize)> = None;
            let mut dom: Option<Duration> = None;
            let mut faites = 0usize;
            let page = page_controle(
                b,
                Rect {
                    x: b.x + 100,
                    y: b.y + 100,
                    l: 300,
                    h: 200,
                },
                Rect {
                    x: b.x + 10,
                    y: b.y + 10,
                    l: 160,
                    h: 40,
                },
            );
            let mut limite = Instant::now() + Duration::from_secs(10);
            boucle.run(move |evenement, cible, flot| {
                *flot = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(5));
                let mut fini_celle_ci = None;
                match evenement {
                    Event::UserEvent(m) if m == "dom" => {
                        if let Some((_, _, t0, _)) = &en_cours {
                            dom.get_or_insert(t0.elapsed());
                        }
                    }
                    Event::UserEvent(m) if m == "pret" => {
                        if let Some((_, _, t0, i)) = &en_cours {
                            fini_celle_ci = Some((Some(t0.elapsed()), *i));
                        }
                    }
                    Event::NewEvents(_) => {
                        if en_cours.is_none() && faites < nombre {
                            let t0 = Instant::now();
                            vue_visible.store(false, Ordering::SeqCst);
                            dom = None;
                            match fenetre(cible, b, true).and_then(|f| {
                                let h = hwnd(&f);
                                courant.store(h.0 as isize, Ordering::SeqCst);
                                style_outil(h);
                                affinite(h, WDA_EXCLUDEFROMCAPTURE.0);
                                traverser(h, true, BITS_APP);
                                let v = vue(&mut contexte, &f, page.clone(), proxy.clone(), true)?;
                                Ok((f, v))
                            }) {
                                Ok((f, v)) => {
                                    en_cours = Some((f, v, t0, faites + 1));
                                    limite = Instant::now() + Duration::from_secs(10);
                                }
                                Err(e) => {
                                    lignes.push(format!("creation {} echec {e}", faites + 1));
                                    faites += 1;
                                }
                            }
                        } else if let Some((_, _, t0, i)) = &en_cours {
                            // Sans « pret » 3 s après « dom » (fenêtre cachée), ou 10 s en tout.
                            let attente_finie =
                                dom.is_some_and(|d| t0.elapsed() > d + Duration::from_secs(3));
                            if Instant::now() > limite || attente_finie {
                                fini_celle_ci = Some((None, *i));
                            }
                        }
                    }
                    _ => {}
                }
                if let Some((pret, i)) = fini_celle_ci {
                    let vue = vue_visible.load(Ordering::SeqCst) as u8;
                    let ms = |d: Option<Duration>| {
                        d.map(|d| format!("{:.1}", d.as_secs_f64() * 1000.0))
                            .unwrap_or_else(|| "absent".into())
                    };
                    // Du lancement du processus à la page qui répond (création à froid
                    // dans un processus neuf : ce que voit la première capture).
                    let depuis_lancement = en_cours
                        .as_ref()
                        .and_then(|e| dom.map(|d| e.2.duration_since(lancement) + d));
                    lignes.push(format!(
                        "creation {i} dom_ms {} pret_ms {} visible_avant_pret {vue} lancement_dom_ms {}",
                        ms(dom),
                        ms(pret),
                        ms(depuis_lancement)
                    ));
                    courant.store(0, Ordering::SeqCst);
                    if let Some((f, v, _, _)) = en_cours.take() {
                        if gardien.is_none() && !sans_gardien {
                            gardien = Some((f, v));
                        } else {
                            drop(v);
                            drop(f);
                        }
                    }
                    faites += 1;
                    let mut avec_fin = lignes.clone();
                    if faites >= nombre {
                        avec_fin.push("fin 1".into());
                    }
                    ecrire_etat(&etat, &avec_fin);
                    if faites >= nombre {
                        gardien.take();
                        *flot = ControlFlow::Exit;
                    }
                }
            });
        }

        /// S7 : la vue Web (WebView2) montre-t-elle de très hautes images ? Chaque image est
        /// décodée (`img.decode()`), dessinée dans une `<canvas>` de sa taille, et le pixel
        /// du bas à droite relu.
        pub fn apercu(images: String) -> Result<(), String> {
            let c = CONFIG.get().ok_or("config")?;
            let etat = c.etat.clone();
            let fichiers: Vec<PathBuf> = images
                .split(';')
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .collect();
            let n = fichiers.len();
            let boucle = EventLoopBuilder::<String>::with_user_event().build();
            let proxy = boucle.create_proxy();
            let mut contexte = WebContext::new(Some(dossier_donnees()));
            let f = fenetre(
                &boucle,
                Rect {
                    x: 0,
                    y: 0,
                    l: 640,
                    h: 480,
                },
                false,
            )?;
            let page = format!(
                r#"<!doctype html><html><head><meta charset="utf-8"></head><body style="margin:0;background:#fff">
<div id="v"></div><script>
async function un(i){{
  var t0=performance.now(), img=new Image();
  img.src='http://sonde.localhost/image/'+i;
  try{{ await img.decode(); }}catch(e){{ return 'apercu '+i+' erreur decode '+e; }}
  var t1=performance.now(), w=img.naturalWidth, h=img.naturalHeight, ok=0, err='';
  try{{
    var c=document.createElement('canvas'); c.width=w; c.height=h;
    var x=c.getContext('2d'); x.drawImage(img,0,0);
    var p=x.getImageData(w-1,h-1,1,1).data; ok=(p[3]===255&&(p[0]|p[1]|p[2])>0)?1:0;
    c.width=1; c.height=1;
  }}catch(e){{ err=' erreur_canvas '+String(e).replace(/\s+/g,'_'); }}
  var t2=performance.now();
  var vue=document.createElement('img'); vue.src=img.src; vue.style.width='200px'; document.getElementById('v').appendChild(vue);
  return 'apercu '+i+' '+w+'x'+h+' decode_ms '+(t1-t0).toFixed(1)+' canvas '+ok+' canvas_ms '+(t2-t1).toFixed(1)+err;
}}
(async function(){{ for(var i=0;i<{n};i++){{ window.ipc.postMessage(await un(i)); }} window.ipc.postMessage('fin'); }})();
</script></body></html>"#
            );
            let page_octets: Vec<u8> = page.into_bytes();
            let v = WebViewBuilder::new_with_web_context(&mut contexte)
                .with_custom_protocol("sonde".into(), move |_id, req| {
                    let chemin = req.uri().path().to_string();
                    let corps: Vec<u8>;
                    let mut genre = "text/html";
                    if let Some(i) = chemin
                        .strip_prefix("/image/")
                        .and_then(|s| s.parse::<usize>().ok())
                    {
                        corps = fichiers
                            .get(i)
                            .and_then(|f| std::fs::read(f).ok())
                            .unwrap_or_default();
                        genre = "image/png";
                    } else {
                        corps = page_octets.clone();
                    }
                    wry::http::Response::builder()
                        .header("Content-Type", genre)
                        .body(std::borrow::Cow::Owned(corps))
                        .unwrap_or_else(|_| {
                            wry::http::Response::new(std::borrow::Cow::Borrowed(&[][..]))
                        })
                })
                .with_url("http://sonde.localhost/")
                .with_ipc_handler(move |req| {
                    let _ = proxy.send_event(req.body().clone());
                })
                .with_additional_browser_args(ARGUMENTS_NAVIGATEUR)
                .build(&f)
                .map_err(|e| format!("webview: {e}"))?;
            f.set_visible(true);
            let mut lignes: Vec<String> = Vec::new();
            let limite = Instant::now() + Duration::from_secs(90);
            let mut garde = Some((f, v, contexte));
            boucle.run(move |evenement, _, flot| {
                *flot = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100));
                let mut fin = Instant::now() > limite;
                if let Event::UserEvent(m) = evenement {
                    if m == "fin" {
                        fin = true;
                    } else {
                        lignes.push(m);
                    }
                    ecrire_etat(&etat, &lignes);
                }
                if fin {
                    lignes.push("fin 1".into());
                    ecrire_etat(&etat, &lignes);
                    garde.take();
                    *flot = ControlFlow::Exit;
                }
            });
        }
    }
}
