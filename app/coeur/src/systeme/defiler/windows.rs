//! Défiler une autre app sous Windows (chantier B, 0.6).
//!
//! - Pages sous un clic (S4, §18) : les fenêtres filles d'abord (rendu de Chromium
//!   `Chrome_RenderWidgetHostHWND`, liste de l'Explorateur `DirectUIHWND` dans
//!   `SHELLDLL_DefView`, fenêtre à barre verticale qui a de quoi défiler, sa zone cliente
//!   sans la barre) : lectures du système, sans message, quelques millisecondes. UI
//!   Automation (`ScrollPattern`, `VerticallyScrollable`) seulement pour les fenêtres où rien
//!   n'est trouvé (WPF, WinUI), dans un fil MTA à lui, attendu au plus `BUDGET_PAGES`.
//! - Moyens (R3) : `WM_MOUSEWHEEL` posté à la fenêtre fille sous le centre de la zone, avec
//!   les coordonnées écran du centre ; UI Automation `Scroll(SmallIncrement)` ; molette
//!   `SendInput` marquée (`MARQUE`), seulement si la garde `garde_molette` le permet.
//!   Jamais de touches, jamais `SetScrollPercent`.
//! - UI Automation vit dans le fil `defilante-automation`, un par défileur, entré dans le
//!   MTA (`CoInitializeEx` MTA, rendu à sa fin ; l'app tient peut-être déjà le MTA par
//!   `CoIncrementMTAUsage`, `systeme/texte` : ce fil s'y joint). Ses objets COM n'en
//!   sortent jamais ; le défileur lui parle par messages et ne l'attend jamais longtemps
//!   (150 ms pour une position, 1 s pour un pas, `RECHERCHE_MAX` pour la recherche).
//!   `AutoSetFocus` est coupé : UI Automation ne prend jamais le focus à la page. Chromium
//!   n'ouvre son arbre d'accessibilité qu'à la première demande (0,3 à 0,4 s mesurés) : la
//!   recherche est refaite deux fois, à 400 ms d'écart.
//! - Molette de l'utilisateur (R4) : crochet `WH_MOUSE_LL` posé dans le fil
//!   `defilante-molette`, qui n'a que sa boucle de messages ; le crochet ne fait que mettre
//!   le cran dans un canal (jamais bloquant) ; le rappel est appelé par le fil
//!   `defilante-molette-rappel`. Un rappel lent ne retarde donc jamais le crochet
//!   (`LowLevelHooksTimeout` le retirerait sans prévenir).
//!
//! Journal : classes de fenêtre, nombres et codes seulement (jamais un titre).

use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TryRecvError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use ::windows::core::{Interface, BOOL, HRESULT, PCWSTR};
use ::windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use ::windows::Win32::Graphics::Gdi::{ClientToScreen, ScreenToClient};
use ::windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use ::windows::Win32::System::LibraryLoader::GetModuleHandleW;
use ::windows::Win32::System::Threading::GetCurrentThreadId;
use ::windows::Win32::System::Variant::VARIANT;
use ::windows::Win32::UI::Accessibility::{
    CUIAutomation, CUIAutomation8, IUIAutomation, IUIAutomation2, IUIAutomationCacheRequest,
    IUIAutomationCondition, IUIAutomationElement, IUIAutomationScrollPattern,
    ScrollAmount_NoAmount, ScrollAmount_SmallDecrement, ScrollAmount_SmallIncrement,
    TreeScope_Children, TreeScope_Descendants, TreeScope_Subtree, UIA_BoundingRectanglePropertyId,
    UIA_ControlTypePropertyId, UIA_IsScrollPatternAvailablePropertyId, UIA_ScrollBarControlTypeId,
    UIA_ScrollPatternId, UIA_ScrollVerticallyScrollablePropertyId,
};
use ::windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_WHEEL, MOUSEINPUT,
};
use ::windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, ChildWindowFromPointEx, DispatchMessageW, EnumChildWindows, GetAncestor,
    GetClassNameW, GetClientRect, GetCursorPos, GetForegroundWindow, GetMessageW, GetScrollInfo,
    GetWindowLongW, IsWindow, IsWindowVisible, PeekMessageW, PostMessageW, PostThreadMessageW,
    SetWindowsHookExW, SystemParametersInfoW, TranslateMessage, UnhookWindowsHookEx,
    WindowFromPoint, CWP_SKIPDISABLED, CWP_SKIPINVISIBLE, CWP_SKIPTRANSPARENT, GA_PARENT, GA_ROOT,
    GWL_STYLE, HC_ACTION, MSG, MSLLHOOKSTRUCT, PM_NOREMOVE, SB_VERT, SCROLLINFO, SIF_PAGE,
    SIF_RANGE, SPI_GETMOUSEWHEELROUTING, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WH_MOUSE_LL,
    WM_MOUSEWHEEL, WM_QUIT, WM_USER, WS_VSCROLL,
};

use super::{
    assembler, barre_utile, centre, cran_entendu, garde_molette, page_par_classe,
    parametres_molette, pixels_entre, plus_petit_sous, position_lue, sans_barre, survol_actif,
    Choix, Envoi, Etalon, FenetreInfo, Molette, Moyen, PageTrouvee, Position, RappelMolette, Rect,
    BUDGET_PAGES, CRAN, MARQUE,
};

/// Attente d'une position lue par UI Automation.
const ATTENTE_POSITION: Duration = Duration::from_millis(150);

/// Attente de la réponse d'un pas d'UI Automation (au-delà, il est compté comme parti :
/// les images diront s'il a porté).
const ATTENTE_PAS: Duration = Duration::from_secs(1);

/// La recherche du `ScrollPattern` de la zone n'est jamais attendue au-delà de ce délai
/// après `preparer` (Chromium : trois essais à 400 ms d'écart).
const RECHERCHE_MAX: Duration = Duration::from_millis(1500);

/// Délais d'UI Automation (connexion et transaction) : une app qui ne répond pas ne garde
/// pas le fil plus longtemps.
const DELAI_UIA_MS: u32 = 1000;

/// Au plus tant d'éléments UI Automation regardés par fenêtre (S4).
const ELEMENTS_MAX: i32 = 64;

/// Au plus tant de fenêtres filles regardées par fenêtre.
const FILLES_MAX: usize = 4000;

/// Attente de l'arrêt du fil du crochet (`Drop` d'`Ecoute`).
const ARRET_MAX: Duration = Duration::from_secs(1);

/// Attente de la pose du crochet.
const POSE_MAX: Duration = Duration::from_secs(2);

/// `UIA_E_ELEMENTNOTAVAILABLE` : l'élément n'existe plus.
const ELEMENT_DISPARU: HRESULT = HRESULT(0x8004_0201_u32 as i32);

fn hwnd(h: isize) -> HWND {
    HWND(h as *mut core::ffi::c_void)
}

fn brut(h: HWND) -> isize {
    h.0 as isize
}

fn rect_de(r: RECT) -> Rect {
    Rect::depuis_bords(r.left, r.top, r.right, r.bottom)
}

fn code(e: &::windows::core::Error) -> String {
    format!("{:#010x}", e.code().0 as u32)
}

/// Nom de la classe de la fenêtre (lu sans message).
fn classe(h: HWND) -> String {
    let mut tampon = [0u16; 64];
    // SAFETY : tampon dont la longueur est passée.
    let n = unsafe { GetClassNameW(h, &mut tampon) };
    let n = (n.max(0) as usize).min(tampon.len());
    String::from_utf16_lossy(&tampon[..n])
}

fn existe(h: HWND) -> bool {
    // SAFETY : lecture d'état, sans message.
    unsafe { IsWindow(Some(h)) }.as_bool()
}

fn racine_de(h: HWND) -> HWND {
    // SAFETY : lecture d'état, sans message.
    unsafe { GetAncestor(h, GA_ROOT) }
}

/// La zone cliente de `h` en pixels du bureau (sans bord ni barres de défilement).
fn zone_cliente(h: HWND) -> Option<Rect> {
    let mut r = RECT::default();
    // SAFETY : RECT de ce fil ; lectures sans message.
    unsafe { GetClientRect(h, &mut r) }.ok()?;
    let mut haut_gauche = POINT {
        x: r.left,
        y: r.top,
    };
    // SAFETY : POINT de ce fil.
    if !unsafe { ClientToScreen(h, &mut haut_gauche) }.as_bool() {
        return None;
    }
    let rect = Rect {
        x: haut_gauche.x,
        y: haut_gauche.y,
        largeur: r.right - r.left,
        hauteur: r.bottom - r.top,
    };
    (!rect.est_vide()).then_some(rect)
}

/// Une barre verticale montrée, qui a de quoi défiler. `GetScrollInfo` sur `SB_VERT` lit
/// l'état gardé par le système (aucun message à la fenêtre).
fn a_une_barre_verticale(h: HWND) -> bool {
    // SAFETY : lecture d'état, sans message.
    let style = unsafe { GetWindowLongW(h, GWL_STYLE) } as u32;
    if style & WS_VSCROLL.0 == 0 {
        return false;
    }
    let mut info = SCROLLINFO {
        cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
        fMask: SIF_RANGE | SIF_PAGE,
        ..Default::default()
    };
    // SAFETY : SCROLLINFO dont la taille est renseignée.
    unsafe { GetScrollInfo(h, SB_VERT, &mut info) }.is_ok()
        && barre_utile(info.nMin, info.nMax, info.nPage)
}

/// La fenêtre est-elle une page (par sa classe, ou par sa barre verticale) ?
fn est_page(h: HWND) -> bool {
    // SAFETY : lecture d'état, sans message.
    let parent = unsafe { GetAncestor(h, GA_PARENT) };
    page_par_classe(&classe(h), &classe(parent)) || a_une_barre_verticale(h)
}

/// Ce que l'énumération des fenêtres filles ramasse.
struct Filles {
    liste: Vec<PageTrouvee>,
    vues: usize,
}

/// Appelée par `EnumChildWindows` pour chaque descendante.
unsafe extern "system" fn rappel_fille(h: HWND, donnee: LPARAM) -> BOOL {
    // SAFETY : `donnee` est l'adresse des `Filles` de `pages_filles`, vivantes pendant
    // l'énumération.
    let filles = unsafe { &mut *(donnee.0 as *mut Filles) };
    filles.vues += 1;
    if filles.vues > FILLES_MAX {
        return BOOL(0);
    }
    // SAFETY : lecture d'état, sans message.
    if unsafe { IsWindowVisible(h) }.as_bool() && est_page(h) {
        if let Some(rect) = zone_cliente(h) {
            filles.liste.push(PageTrouvee {
                rect,
                fenetre: brut(h),
            });
        }
    }
    BOOL(1)
}

/// Les pages de la fenêtre de premier niveau `racine` qui se voient par les fenêtres :
/// elle-même si elle a une barre verticale, puis ses descendantes.
fn pages_filles(racine: HWND) -> Vec<PageTrouvee> {
    let mut filles = Filles {
        liste: Vec::new(),
        vues: 0,
    };
    if !existe(racine) {
        return filles.liste;
    }
    if a_une_barre_verticale(racine) {
        if let Some(rect) = zone_cliente(racine) {
            filles.liste.push(PageTrouvee {
                rect,
                fenetre: brut(racine),
            });
        }
    }
    // SAFETY : le rappel ne garde pas l'adresse de `filles` au-delà de l'appel.
    let _ = unsafe {
        EnumChildWindows(
            Some(racine),
            Some(rappel_fille),
            LPARAM(&mut filles as *mut Filles as isize),
        )
    };
    filles.liste
}

pub fn pages(fenetres: &[FenetreInfo], max: usize) -> Vec<PageTrouvee> {
    let debut = Instant::now();
    let n = fenetres.len().min(max);
    let fenetres = &fenetres[..n];
    let bords: Vec<Rect> = fenetres.iter().map(|f| f.rect).collect();
    let mut trouvees: Vec<Vec<PageTrouvee>> = fenetres
        .iter()
        .map(|f| {
            if f.fenetre == 0 {
                Vec::new()
            } else {
                pages_filles(hwnd(f.fenetre))
            }
        })
        .collect();
    let par_filles = trouvees.iter().filter(|t| !t.is_empty()).count();
    let manquent: Vec<(usize, isize)> = trouvees
        .iter()
        .enumerate()
        .filter(|(i, t)| t.is_empty() && fenetres[*i].fenetre != 0)
        .map(|(i, _)| (i, fenetres[i].fenetre))
        .collect();
    let (mut par_automation, mut en_retard) = (0usize, 0usize);
    if !manquent.is_empty() {
        let echeance = debut + BUDGET_PAGES;
        let attendues = manquent.len();
        let (tx, rx) = mpsc::channel();
        let lance = thread::Builder::new()
            .name("defilante-pages-automation".into())
            .spawn(move || pages_automation(manquent, echeance, tx));
        match lance {
            Ok(_) => {
                let mut recues = 0;
                while recues < attendues {
                    let reste = echeance.saturating_duration_since(Instant::now());
                    match rx.recv_timeout(reste) {
                        Ok((i, p)) => {
                            recues += 1;
                            if !p.is_empty() {
                                par_automation += 1;
                            }
                            trouvees[i] = p;
                        }
                        Err(_) => break,
                    }
                }
                en_retard = attendues - recues;
            }
            Err(e) => log::warn!("defiler pages: fil d'automation impossible ({e})"),
        }
    }
    let liste = assembler(&bords, &trouvees);
    log::info!(
        "defiler pages: {} (filles {par_filles}, automation {par_automation}, en retard \
         {en_retard}) sur {n} fenetres en {} ms",
        liste.len(),
        debut.elapsed().as_millis()
    );
    liste
}

// ---------------------------------------------------------------------------------------
// UI Automation (toujours dans un fil MTA à lui).

/// COM ouvert en MTA pour ce fil ; rendu à la destruction (après tous ses objets).
struct Com;

impl Com {
    fn ouvrir() -> Result<Com, String> {
        // SAFETY : sans réservé ; équilibré par `Drop`.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr.is_ok() {
            Ok(Com)
        } else {
            Err(format!("CoInitializeEx {:#010x}", hr.0 as u32))
        }
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        // SAFETY : équilibre le `CoInitializeEx` réussi de ce fil.
        unsafe { CoUninitialize() };
    }
}

/// Le client UI Automation de ce fil : `CUIAutomation8` si possible (délais réglables,
/// `AutoSetFocus` coupé), sinon `CUIAutomation`.
fn automation() -> ::windows::core::Result<IUIAutomation> {
    // SAFETY : COM ouvert sur ce fil (`Com`).
    let auto: IUIAutomation =
        match unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) } {
            Ok(a) => a,
            // SAFETY : idem.
            Err(_) => unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }?,
        };
    if let Ok(a2) = auto.cast::<IUIAutomation2>() {
        // SAFETY : réglages du client, sans effet sur les pages.
        unsafe {
            let _ = a2.SetAutoSetFocus(false);
            let _ = a2.SetConnectionTimeout(DELAI_UIA_MS);
            let _ = a2.SetTransactionTimeout(DELAI_UIA_MS);
        }
    }
    Ok(auto)
}

/// Les éléments qui ont un `ScrollPattern` et défilent verticalement.
fn condition_defilable(auto: &IUIAutomation) -> ::windows::core::Result<IUIAutomationCondition> {
    // SAFETY : appels COM du fil du client.
    unsafe {
        let a = auto.CreatePropertyCondition(
            UIA_IsScrollPatternAvailablePropertyId,
            &VARIANT::from(true),
        )?;
        let b = auto.CreatePropertyCondition(
            UIA_ScrollVerticallyScrollablePropertyId,
            &VARIANT::from(true),
        )?;
        auto.CreateAndCondition(&a, &b)
    }
}

/// Les barres de défilement.
fn condition_barre(auto: &IUIAutomation) -> ::windows::core::Result<IUIAutomationCondition> {
    // SAFETY : appel COM du fil du client.
    unsafe {
        auto.CreatePropertyCondition(
            UIA_ControlTypePropertyId,
            &VARIANT::from(UIA_ScrollBarControlTypeId.0),
        )
    }
}

/// Le rectangle de chaque élément trouvé, ramené avec lui.
fn cache_rectangle(auto: &IUIAutomation) -> ::windows::core::Result<IUIAutomationCacheRequest> {
    // SAFETY : appels COM du fil du client.
    unsafe {
        let cache = auto.CreateCacheRequest()?;
        cache.AddProperty(UIA_BoundingRectanglePropertyId)?;
        Ok(cache)
    }
}

/// Les rectangles des barres de défilement filles de `element` (à défaut, descendantes).
fn barres_de(element: &IUIAutomationElement, condition: &IUIAutomationCondition) -> Vec<Rect> {
    let mut barres = Vec::new();
    for portee in [TreeScope_Children, TreeScope_Descendants] {
        // SAFETY : appels COM du fil du client.
        let Ok(liste) = (unsafe { element.FindAll(portee, condition) }) else {
            continue;
        };
        // SAFETY : idem.
        let n = unsafe { liste.Length() }
            .unwrap_or(0)
            .clamp(0, ELEMENTS_MAX);
        for k in 0..n {
            // SAFETY : indice dans la liste.
            if let Ok(rect) = unsafe {
                liste
                    .GetElement(k)
                    .and_then(|b| b.CurrentBoundingRectangle())
            } {
                barres.push(rect_de(rect));
            }
        }
        if !barres.is_empty() {
            break;
        }
    }
    barres
}

/// Ce qu'UI Automation trouve de défilable dans la fenêtre `h` (S4, complément).
fn pages_d_une_fenetre(
    auto: &IUIAutomation,
    defilable: &IUIAutomationCondition,
    barre: Option<&IUIAutomationCondition>,
    cache: Option<&IUIAutomationCacheRequest>,
    h: HWND,
    echeance: Instant,
) -> Vec<PageTrouvee> {
    let mut pages = Vec::new();
    // SAFETY : appels COM du fil du client.
    let Ok(racine) = (unsafe { auto.ElementFromHandle(h) }) else {
        return pages;
    };
    // SAFETY : idem.
    let liste = unsafe {
        match cache {
            Some(c) => racine.FindAllBuildCache(TreeScope_Subtree, defilable, c),
            None => racine.FindAll(TreeScope_Subtree, defilable),
        }
    };
    let Ok(liste) = liste else {
        return pages;
    };
    // SAFETY : idem.
    let n = unsafe { liste.Length() }
        .unwrap_or(0)
        .clamp(0, ELEMENTS_MAX);
    for k in 0..n {
        if Instant::now() >= echeance {
            break;
        }
        // SAFETY : indice dans la liste.
        let Ok(element) = (unsafe { liste.GetElement(k) }) else {
            continue;
        };
        // SAFETY : appels COM du fil du client.
        let rect = unsafe {
            if cache.is_some() {
                element.CachedBoundingRectangle()
            } else {
                element.CurrentBoundingRectangle()
            }
        };
        let Ok(rect) = rect.map(rect_de) else {
            continue;
        };
        if rect.est_vide() {
            continue;
        }
        let barres = barre.map(|c| barres_de(&element, c)).unwrap_or_default();
        pages.push(PageTrouvee {
            rect: sans_barre(rect, &barres),
            fenetre: brut(h),
        });
    }
    pages
}

/// Le fil `defilante-pages-automation` : une réponse par fenêtre, tant que l'échéance
/// n'est pas passée. Au-delà, plus personne ne l'attend : il finit ce qu'il fait et s'en
/// va.
fn pages_automation(
    manquent: Vec<(usize, isize)>,
    echeance: Instant,
    tx: Sender<(usize, Vec<PageTrouvee>)>,
) {
    let Ok(_com) = Com::ouvrir() else {
        return;
    };
    let Ok(auto) = automation() else {
        return;
    };
    let Ok(defilable) = condition_defilable(&auto) else {
        return;
    };
    let barre = condition_barre(&auto).ok();
    let cache = cache_rectangle(&auto).ok();
    for (i, h) in manquent {
        if Instant::now() >= echeance {
            break;
        }
        let pages = pages_d_une_fenetre(
            &auto,
            &defilable,
            barre.as_ref(),
            cache.as_ref(),
            hwnd(h),
            echeance,
        );
        if tx.send((i, pages)).is_err() {
            break;
        }
    }
}

/// Le `ScrollPattern` de la zone, et la hauteur de son élément.
struct Trouve {
    motif: IUIAutomationScrollPattern,
    hauteur: i32,
}

/// `element` défile-t-il verticalement, sous `point` ?
fn defilable_sous(element: &IUIAutomationElement, point: (i32, i32)) -> Option<Trouve> {
    // SAFETY : appels COM du fil du client.
    unsafe {
        let rect = rect_de(element.CurrentBoundingRectangle().ok()?);
        if !rect.contient(point) {
            return None;
        }
        let motif: IUIAutomationScrollPattern =
            element.GetCurrentPatternAs(UIA_ScrollPatternId).ok()?;
        if !motif.CurrentVerticallyScrollable().ok()?.as_bool() {
            return None;
        }
        Some(Trouve {
            motif,
            hauteur: rect.hauteur,
        })
    }
}

/// Cherche l'élément qui défile sous `point` : la fenêtre cible elle-même, puis ses
/// descendants (le plus petit qui contient le point), puis la même chose depuis la racine.
fn chercher(
    auto: &IUIAutomation,
    cible: isize,
    racine: isize,
    point: (i32, i32),
) -> Option<Trouve> {
    let condition = condition_defilable(auto).ok();
    let mut vues = Vec::new();
    for h in [cible, racine] {
        if h == 0 || vues.contains(&h) {
            continue;
        }
        vues.push(h);
        // SAFETY : appel COM du fil du client.
        let Ok(element) = (unsafe { auto.ElementFromHandle(hwnd(h)) }) else {
            continue;
        };
        if let Some(t) = defilable_sous(&element, point) {
            return Some(t);
        }
        let Some(condition) = &condition else {
            continue;
        };
        // SAFETY : idem.
        let Ok(liste) = (unsafe { element.FindAll(TreeScope_Descendants, condition) }) else {
            continue;
        };
        // SAFETY : idem.
        let n = unsafe { liste.Length() }
            .unwrap_or(0)
            .clamp(0, ELEMENTS_MAX);
        let mut elements = Vec::new();
        let mut rects = Vec::new();
        for k in 0..n {
            // SAFETY : indice dans la liste.
            let Ok(e) = (unsafe { liste.GetElement(k) }) else {
                continue;
            };
            // SAFETY : appel COM du fil du client.
            if let Ok(r) = unsafe { e.CurrentBoundingRectangle() } {
                rects.push(rect_de(r));
                elements.push(e);
            }
        }
        if let Some(i) = plus_petit_sous(&rects, point) {
            if let Some(t) = defilable_sous(&elements[i], point) {
                return Some(t);
            }
        }
    }
    None
}

fn lire_position(motif: &IUIAutomationScrollPattern) -> Option<Position> {
    // SAFETY : appels COM du fil du client.
    unsafe {
        let defilable = motif.CurrentVerticallyScrollable().ok()?.as_bool();
        let pour_cent = motif.CurrentVerticalScrollPercent().ok()?;
        let vue = motif.CurrentVerticalViewSize().ok()?;
        position_lue(defilable, pour_cent, vue)
    }
}

/// `unites` pas `SmallIncrement` (négatif : `SmallDecrement`). `Err` seulement si
/// l'élément n'existe plus avant le premier pas ; toute autre erreur est laissée aux images.
fn faire_defiler(motif: &IUIAutomationScrollPattern, unites: i32) -> Result<(), String> {
    let sens = if unites > 0 {
        ScrollAmount_SmallIncrement
    } else {
        ScrollAmount_SmallDecrement
    };
    for k in 0..unites.unsigned_abs() {
        // SAFETY : appel COM du fil du client.
        if let Err(e) = unsafe { motif.Scroll(ScrollAmount_NoAmount, sens) } {
            if k == 0 && e.code() == ELEMENT_DISPARU {
                return Err(format!("Scroll {}", code(&e)));
            }
            break;
        }
    }
    Ok(())
}

/// Ce que le défileur demande au fil d'UI Automation.
enum Demande {
    Position(SyncSender<Option<Position>>),
    Defiler(i32, SyncSender<Result<(), String>>),
}

/// Réponse quand aucun `ScrollPattern` n'est (encore) là.
fn repondre_sans(d: Demande) {
    match d {
        Demande::Position(r) => {
            let _ = r.send(None);
        }
        Demande::Defiler(_, r) => {
            let _ = r.send(Err("aucun ScrollPattern".into()));
        }
    }
}

/// Le fil `defilante-automation` d'un défileur : cherche le `ScrollPattern` de la zone
/// (deux essais de plus, à `pause` d'écart, pour Chromium), dit le résultat, puis répond
/// aux demandes jusqu'à ce que le défileur soit détruit.
fn travailler(
    cible: isize,
    racine: isize,
    point: (i32, i32),
    chromium: bool,
    demandes: Receiver<Demande>,
    recherche: SyncSender<Option<i32>>,
) {
    let _com = match Com::ouvrir() {
        Ok(c) => c,
        Err(e) => {
            log::info!("defiler automation: {e}");
            let _ = recherche.send(None);
            return;
        }
    };
    let auto = match automation() {
        Ok(a) => a,
        Err(e) => {
            log::info!("defiler automation: client impossible ({})", code(&e));
            let _ = recherche.send(None);
            return;
        }
    };
    let (essais, pause) = if chromium {
        (3, Duration::from_millis(400))
    } else {
        (2, Duration::from_millis(300))
    };
    let debut = Instant::now();
    let mut trouve = None;
    let mut essai = 0;
    while essai < essais {
        essai += 1;
        trouve = chercher(&auto, cible, racine, point);
        if trouve.is_some() || essai == essais {
            break;
        }
        let fin = Instant::now() + pause;
        loop {
            let reste = fin.saturating_duration_since(Instant::now());
            if reste.is_zero() {
                break;
            }
            match demandes.recv_timeout(reste) {
                Ok(d) => repondre_sans(d),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }
    log::info!(
        "defiler automation: {} en {} ms ({essai} essai(s))",
        if trouve.is_some() {
            "page trouvee"
        } else {
            "aucune page"
        },
        debut.elapsed().as_millis()
    );
    let _ = recherche.send(trouve.as_ref().map(|t| t.hauteur));
    match trouve {
        Some(t) => {
            while let Ok(d) = demandes.recv() {
                match d {
                    Demande::Position(r) => {
                        let _ = r.send(lire_position(&t.motif));
                    }
                    Demande::Defiler(n, r) => {
                        let _ = r.send(faire_defiler(&t.motif, n));
                    }
                }
            }
        }
        None => {
            while let Ok(d) = demandes.recv() {
                repondre_sans(d);
            }
        }
    }
}

/// Où en est la recherche du `ScrollPattern`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Recherche {
    EnCours,
    Trouvee { hauteur: i32 },
    Absente,
}

/// Le lien du défileur avec son fil d'UI Automation. Le détruire arrête le fil (sans
/// l'attendre).
#[derive(Debug)]
struct Automation {
    demandes: Sender<Demande>,
    recherche: Receiver<Option<i32>>,
    etat: Cell<Recherche>,
    depuis: Instant,
}

impl Automation {
    fn lancer(
        cible: isize,
        racine: isize,
        point: (i32, i32),
        chromium: bool,
    ) -> Option<Automation> {
        let (dtx, drx) = mpsc::channel();
        let (rtx, rrx) = mpsc::sync_channel(1);
        let lance = thread::Builder::new()
            .name("defilante-automation".into())
            .spawn(move || travailler(cible, racine, point, chromium, drx, rtx));
        match lance {
            Ok(_) => Some(Automation {
                demandes: dtx,
                recherche: rrx,
                etat: Cell::new(Recherche::EnCours),
                depuis: Instant::now(),
            }),
            Err(e) => {
                log::warn!("defiler automation: fil impossible ({e})");
                None
            }
        }
    }

    fn noter(&self, r: Option<i32>) -> Recherche {
        let etat = match r {
            Some(hauteur) => Recherche::Trouvee { hauteur },
            None => Recherche::Absente,
        };
        self.etat.set(etat);
        etat
    }

    /// L'état de la recherche, sans attendre.
    fn etat(&self) -> Recherche {
        if self.etat.get() == Recherche::EnCours {
            match self.recherche.try_recv() {
                Ok(r) => return self.noter(r),
                Err(TryRecvError::Disconnected) => return self.noter(None),
                Err(TryRecvError::Empty) => {}
            }
        }
        self.etat.get()
    }

    /// L'état de la recherche, en l'attendant au plus jusqu'à `RECHERCHE_MAX` après son
    /// départ (encore en cours au-delà : `EnCours`).
    fn attendre(&self) -> Recherche {
        if self.etat() == Recherche::EnCours {
            let reste = (self.depuis + RECHERCHE_MAX).saturating_duration_since(Instant::now());
            match self.recherche.recv_timeout(reste) {
                Ok(r) => return self.noter(r),
                Err(RecvTimeoutError::Disconnected) => return self.noter(None),
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
        self.etat.get()
    }

    fn hauteur(&self) -> Option<i32> {
        match self.etat() {
            Recherche::Trouvee { hauteur } => Some(hauteur),
            _ => None,
        }
    }

    fn position(&self) -> Option<Position> {
        self.hauteur()?;
        let (tx, rx) = mpsc::sync_channel(1);
        self.demandes.send(Demande::Position(tx)).ok()?;
        rx.recv_timeout(ATTENTE_POSITION).ok().flatten()
    }

    fn defiler(&self, unites: i32) -> Result<(), String> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.demandes
            .send(Demande::Defiler(unites, tx))
            .map_err(|_| "fil d'automation arrete".to_string())?;
        match rx.recv_timeout(ATTENTE_PAS) {
            Ok(r) => r,
            // Parti, sans réponse à temps : les images diront.
            Err(RecvTimeoutError::Timeout) => Ok(()),
            Err(RecvTimeoutError::Disconnected) => Err("fil d'automation arrete".into()),
        }
    }
}

// ---------------------------------------------------------------------------------------
// Le défileur.

/// La dernière commande partie.
#[derive(Debug, Clone, Copy)]
struct Derniere {
    moyen: Moyen,
    unites: i32,
    /// La position lue juste avant (moyen `Automation` seulement).
    position: Option<Position>,
}

#[derive(Debug)]
pub struct Defileur {
    zone: Rect,
    centre: (i32, i32),
    /// La fenêtre qui reçoit `WM_MOUSEWHEEL` (0 : aucune).
    cible: isize,
    /// Sa racine (0 : aucune).
    racine: isize,
    choix: Choix,
    etalons: [Etalon; 3],
    automation: Option<Automation>,
    derniere: Option<Derniere>,
}

/// La fenêtre de premier niveau (d'une autre app) sous `point` : la première de la liste
/// des fenêtres visibles du Gel (BreezyShot exclu, premier plan d'abord) qui le contient.
fn racine_sous(point: (i32, i32)) -> Option<HWND> {
    crate::systeme::gel::fenetres_visibles()
        .into_iter()
        .find(|f| f.fenetre != 0 && f.rect.contient(point))
        .map(|f| racine_de(hwnd(f.fenetre)))
}

/// La fenêtre la plus profonde de `racine` sous `point`, sans les fenêtres cachées,
/// désactivées ou transparentes (Chromium pose une fenêtre transparente sur sa page).
fn plus_profonde_sous(racine: HWND, point: (i32, i32)) -> HWND {
    let mut h = racine;
    for _ in 0..32 {
        let mut p = POINT {
            x: point.0,
            y: point.1,
        };
        // SAFETY : POINT de ce fil ; lectures sans message.
        if !unsafe { ScreenToClient(h, &mut p) }.as_bool() {
            break;
        }
        // SAFETY : idem.
        let enfant = unsafe {
            ChildWindowFromPointEx(
                h,
                p,
                CWP_SKIPINVISIBLE | CWP_SKIPDISABLED | CWP_SKIPTRANSPARENT,
            )
        };
        if enfant.is_invalid() || enfant == h {
            break;
        }
        h = enfant;
    }
    h
}

/// La fenêtre qui recevra `WM_MOUSEWHEEL` : la page cliquée si c'est une fenêtre fille sous
/// le centre ; sinon la plus petite page de la racine sous le centre ; sinon la plus
/// profonde fenêtre sous le centre.
fn cible_sous(donnee: Option<HWND>, racine: Option<HWND>, point: (i32, i32)) -> Option<HWND> {
    if let Some(h) = donnee {
        if racine_de(h) != h && zone_cliente(h).is_some_and(|r| r.contient(point)) {
            return Some(h);
        }
    }
    let racine = racine?;
    let pages = pages_filles(racine);
    let rects: Vec<Rect> = pages.iter().map(|p| p.rect).collect();
    if let Some(i) = plus_petit_sous(&rects, point) {
        return Some(hwnd(pages[i].fenetre));
    }
    Some(plus_profonde_sous(racine, point))
}

/// `SPI_GETMOUSEWHEELROUTING`, `None` s'il est illisible.
fn routage() -> Option<u32> {
    let mut valeur = 0u32;
    // SAFETY : u32 attendu par cette action.
    unsafe {
        SystemParametersInfoW(
            SPI_GETMOUSEWHEELROUTING,
            0,
            Some(&mut valeur as *mut u32 as *mut core::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }
    .ok()
    .map(|()| valeur)
}

fn curseur() -> Option<(i32, i32)> {
    let mut p = POINT::default();
    // SAFETY : POINT de ce fil.
    unsafe { GetCursorPos(&mut p) }.ok().map(|()| (p.x, p.y))
}

/// La racine de la fenêtre que la souris toucherait en `point` (les fenêtres traversées
/// par les clics, comme la fenêtre de contrôle hors de sa pilule, ne comptent pas).
fn racine_au_point(point: (i32, i32)) -> Option<isize> {
    // SAFETY : lecture d'état, sans message.
    let h = unsafe {
        WindowFromPoint(POINT {
            x: point.0,
            y: point.1,
        })
    };
    (!h.is_invalid()).then(|| brut(racine_de(h)))
}

fn premier_plan() -> Option<isize> {
    // SAFETY : sans paramètre.
    let h = unsafe { GetForegroundWindow() };
    (!h.is_invalid()).then(|| brut(racine_de(h)))
}

pub fn preparer(zone: Rect, fenetre: Option<isize>) -> Result<Defileur, String> {
    if zone.est_vide() {
        return Err("zone vide".into());
    }
    let point = centre(zone);
    let donnee = fenetre.map(hwnd).filter(|h| existe(*h));
    let racine = match donnee {
        Some(h) => Some(racine_de(h)).filter(|r| !r.is_invalid()),
        None => racine_sous(point),
    };
    let cible = cible_sous(donnee, racine, point);
    let classe_cible = cible.map(classe).unwrap_or_default();
    let classe_racine = racine.map(classe).unwrap_or_default();
    let chromium = classe_racine.starts_with("Chrome_WidgetWin")
        || classe_cible == "Chrome_RenderWidgetHostHWND";
    let mut choix = Choix::nouveau();
    if cible.is_none() {
        choix.exclure(Moyen::Message);
    }
    if racine.is_none() {
        choix.exclure(Moyen::Molette);
    }
    let automation = match (cible, racine) {
        (None, None) => None,
        (c, r) => Automation::lancer(
            c.or(r).map_or(0, brut),
            r.or(c).map_or(0, brut),
            point,
            chromium,
        ),
    };
    if automation.is_none() {
        choix.exclure(Moyen::Automation);
    }
    log::info!(
        "defiler cible {} racine {} moyen {}",
        if classe_cible.is_empty() {
            "aucune"
        } else {
            classe_cible.as_str()
        },
        if classe_racine.is_empty() {
            "aucune"
        } else {
            classe_racine.as_str()
        },
        choix.moyen().map_or("aucun", Moyen::nom)
    );
    Ok(Defileur {
        zone,
        centre: point,
        cible: cible.map_or(0, brut),
        racine: racine.map_or(0, brut),
        choix,
        etalons: [Etalon::default(); 3],
        automation,
        derniere: None,
    })
}

impl Defileur {
    /// Le moyen `m` peut-il servir ? (`Automation` : attend la fin de la recherche, borné.)
    fn disponible(&self, m: Moyen) -> bool {
        match m {
            Moyen::Message => self.cible != 0 && existe(hwnd(self.cible)),
            Moyen::Automation => self
                .automation
                .as_ref()
                .is_some_and(|a| matches!(a.attendre(), Recherche::Trouvee { .. })),
            Moyen::Molette => self.racine != 0,
        }
    }

    /// Laisse les moyens en cours qui ne peuvent pas servir ; rend celui qui reste.
    fn assurer(&mut self) -> Option<Moyen> {
        while let Some(m) = self.choix.moyen() {
            if self.disponible(m) {
                break;
            }
            let suivant = self.choix.exclure(m);
            log::info!(
                "defiler {} indisponible, suite {}",
                m.nom(),
                suivant.map_or("aucun", Moyen::nom)
            );
        }
        self.choix.moyen()
    }

    fn par_message(&self, unites: i32) -> Result<Envoi, String> {
        let cible = hwnd(self.cible);
        if self.cible == 0 || !existe(cible) {
            return Err("fenetre fermee".into());
        }
        let delta = (if unites > 0 { -CRAN } else { CRAN }) as i16;
        let (w, l) = parametres_molette(delta, self.centre);
        let mut postes = 0;
        for _ in 0..unites.unsigned_abs() {
            // SAFETY : message sans pointeur, posté (jamais attendu).
            match unsafe { PostMessageW(Some(cible), WM_MOUSEWHEEL, WPARAM(w), LPARAM(l)) } {
                Ok(()) => postes += 1,
                Err(e) if postes == 0 => return Err(format!("PostMessageW {}", code(&e))),
                Err(_) => break,
            }
        }
        Ok(Envoi::Envoye)
    }

    fn par_automation(&self, unites: i32) -> Result<Envoi, String> {
        let Some(a) = &self.automation else {
            return Err("automation absente".into());
        };
        if !matches!(a.attendre(), Recherche::Trouvee { .. }) {
            return Err("aucun ScrollPattern".into());
        }
        a.defiler(unites).map(|()| Envoi::Envoye)
    }

    fn par_molette(&self, unites: i32) -> Result<Envoi, String> {
        let pointeur = curseur();
        let garde = garde_molette(
            pointeur,
            self.zone,
            pointeur.and_then(racine_au_point),
            self.racine,
            premier_plan(),
            survol_actif(routage()),
        );
        if garde == Envoi::Saute {
            return Ok(Envoi::Saute);
        }
        let delta = if unites > 0 { -CRAN } else { CRAN };
        let cran = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    mouseData: delta as u32,
                    dwFlags: MOUSEEVENTF_WHEEL,
                    dwExtraInfo: MARQUE,
                    ..Default::default()
                },
            },
        };
        let entrees = vec![cran; unites.unsigned_abs() as usize];
        // SAFETY : entrées dont la taille est passée.
        let parties = unsafe { SendInput(&entrees, std::mem::size_of::<INPUT>() as i32) };
        // 0 : l'entrée est bloquée (bureau sécurisé, autre fil) ; à retenter.
        Ok(if parties == 0 {
            Envoi::Saute
        } else {
            Envoi::Envoye
        })
    }

    /// Règle l'étalon d'`Automation` sur la position lue (les images n'ont rien dit).
    fn etalonner(&mut self) {
        let Some(d) = self.derniere.take() else {
            return;
        };
        if d.moyen != Moyen::Automation {
            return;
        }
        let (Some(avant), Some(a)) = (d.position, self.automation.as_ref()) else {
            return;
        };
        let (Some(apres), Some(hauteur)) = (a.position(), a.hauteur()) else {
            return;
        };
        if let Some(px) = pixels_entre(avant, apres, hauteur) {
            self.etalons[d.moyen.rang()].constater(d.unites, px);
        }
    }
}

pub fn moyen(d: &Defileur) -> Option<Moyen> {
    d.choix.moyen()
}

pub fn envoyes(d: &Defileur) -> u32 {
    d.choix.envoyes()
}

pub fn abandonner(d: &mut Defileur) -> Option<Moyen> {
    let avant = d.choix.moyen();
    d.choix.abandonner();
    if d.choix.moyen() != avant {
        d.derniere = None;
    }
    d.assurer()
}

pub fn constater(d: &mut Defileur, avance: u32) {
    let Some(derniere) = d.derniere.take() else {
        return;
    };
    if avance == 0 || d.choix.moyen() != Some(derniere.moyen) {
        return;
    }
    d.choix.retenir();
    let pixels = avance as f64 * derniere.unites.signum() as f64;
    d.etalons[derniere.moyen.rang()].constater(derniere.unites, pixels);
}

pub fn descendre(d: &mut Defileur, pixels: i32) -> Result<Envoi, String> {
    if pixels == 0 {
        return Ok(Envoi::Saute);
    }
    d.etalonner();
    while let Some(m) = d.assurer() {
        let unites = d.etalons[m.rang()].unites(pixels, m.unites_max());
        let position = match (m, d.automation.as_ref()) {
            (Moyen::Automation, Some(a)) => a.position(),
            _ => None,
        };
        let essai = match m {
            Moyen::Message => d.par_message(unites),
            Moyen::Automation => d.par_automation(unites),
            Moyen::Molette => d.par_molette(unites),
        };
        match essai {
            Ok(Envoi::Envoye) => {
                d.choix.envoye();
                d.etalons[m.rang()].envoye(unites);
                d.derniere = Some(Derniere {
                    moyen: m,
                    unites,
                    position,
                });
                return Ok(Envoi::Envoye);
            }
            Ok(Envoi::Saute) => return Ok(Envoi::Saute),
            Err(cause) => {
                let suivant = d.choix.exclure(m);
                log::info!(
                    "defiler {} impossible ({cause}), suite {}",
                    m.nom(),
                    suivant.map_or("aucun", Moyen::nom)
                );
            }
        }
    }
    Ok(Envoi::Saute)
}

pub fn position(d: &Defileur) -> Option<Position> {
    d.automation.as_ref()?.position()
}

// ---------------------------------------------------------------------------------------
// L'écoute de la molette.

thread_local! {
    /// Le canal du crochet, dans le fil `defilante-molette` seulement.
    static ENVOI: RefCell<Option<Sender<Molette>>> = const { RefCell::new(None) };
}

/// Crochets posés et pas encore retirés (pour les tests).
#[cfg(test)]
static CROCHETS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Le crochet `WH_MOUSE_LL` : lit le cran, le met dans le canal (jamais bloquant), passe.
unsafe extern "system" fn rappel_crochet(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && wparam.0 == WM_MOUSEWHEEL as usize && lparam.0 != 0 {
        // SAFETY : pour `WH_MOUSE_LL`, `lParam` pointe sur un `MSLLHOOKSTRUCT` valide le
        // temps de l'appel.
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        if let Some(m) = cran_entendu(info.mouseData, info.dwExtraInfo, (info.pt.x, info.pt.y)) {
            let _ = ENVOI.try_with(|e| {
                if let Ok(e) = e.try_borrow() {
                    if let Some(tx) = e.as_ref() {
                        let _ = tx.send(m);
                    }
                }
            });
        }
    }
    // SAFETY : passe au crochet suivant, sans rien changer.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Le fil `defilante-molette` : pose le crochet, le dit, tourne sa boucle de messages
/// jusqu'à `WM_QUIT`, retire le crochet, le dit.
fn boucle_crochet(
    tx: Sender<Molette>,
    pret: SyncSender<Result<u32, String>>,
    abandonne: Arc<AtomicBool>,
    fini: SyncSender<()>,
    actif: Arc<AtomicBool>,
) {
    let mut msg = MSG::default();
    // SAFETY : crée la file de messages de ce fil avant que `Drop` puisse y poster.
    unsafe {
        let _ = PeekMessageW(&mut msg, None, WM_USER, WM_USER, PM_NOREMOVE);
    }
    // SAFETY : sans paramètre.
    let fil = unsafe { GetCurrentThreadId() };
    ENVOI.with(|e| *e.borrow_mut() = Some(tx));
    // SAFETY : module du programme courant.
    let module = unsafe { GetModuleHandleW(PCWSTR::null()) }
        .ok()
        .map(HINSTANCE::from);
    // SAFETY : procédure de ce module, qui vit autant que le programme.
    let crochet = match unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(rappel_crochet), module, 0) } {
        Ok(c) => c,
        Err(e) => {
            ENVOI.with(|envoi| envoi.borrow_mut().take());
            let _ = pret.send(Err(format!("SetWindowsHookExW {}", code(&e))));
            return;
        }
    };
    #[cfg(test)]
    CROCHETS.fetch_add(1, Ordering::SeqCst);
    actif.store(true, Ordering::SeqCst);
    // Personne n'attend plus (pose trop lente) : on retire tout de suite.
    if pret.send(Ok(fil)).is_ok() && !abandonne.load(Ordering::SeqCst) {
        loop {
            // SAFETY : file de messages de ce fil.
            let r = unsafe { GetMessageW(&mut msg, None, 0, 0) };
            if r.0 <= 0 {
                break;
            }
            // SAFETY : message de ce fil.
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
    // SAFETY : le crochet posé ci-dessus, retiré une fois.
    let retire = unsafe { UnhookWindowsHookEx(crochet) };
    #[cfg(test)]
    if retire.is_ok() {
        CROCHETS.fetch_sub(1, Ordering::SeqCst);
    }
    if let Err(e) = retire {
        log::warn!("defiler molette: crochet non retire ({})", code(&e));
    }
    actif.store(false, Ordering::SeqCst);
    // Le canal fermé arrête le fil du rappel.
    ENVOI.with(|e| e.borrow_mut().take());
    let _ = fini.send(());
}

#[derive(Debug)]
pub struct Ecoute {
    fil: u32,
    actif: Arc<AtomicBool>,
    fini: Receiver<()>,
    poignee: Option<JoinHandle<()>>,
}

pub fn ecouter_molette(rappel: RappelMolette) -> Result<Ecoute, String> {
    let (tx, rx) = mpsc::channel::<Molette>();
    let (pret_tx, pret_rx) = mpsc::sync_channel(1);
    let (fini_tx, fini_rx) = mpsc::sync_channel(1);
    let actif = Arc::new(AtomicBool::new(false));
    let actif_fil = Arc::clone(&actif);
    let abandonne = Arc::new(AtomicBool::new(false));
    let abandonne_fil = Arc::clone(&abandonne);
    thread::Builder::new()
        .name("defilante-molette-rappel".into())
        .spawn(move || {
            for m in rx {
                rappel(m);
            }
        })
        .map_err(|e| format!("fil du rappel: {e}"))?;
    let poignee = thread::Builder::new()
        .name("defilante-molette".into())
        .spawn(move || boucle_crochet(tx, pret_tx, abandonne_fil, fini_tx, actif_fil))
        .map_err(|e| format!("fil du crochet: {e}"))?;
    match pret_rx.recv_timeout(POSE_MAX) {
        Ok(Ok(fil)) => Ok(Ecoute {
            fil,
            actif,
            fini: fini_rx,
            poignee: Some(poignee),
        }),
        Ok(Err(e)) => {
            let _ = poignee.join();
            Err(e)
        }
        Err(_) => {
            // Le fil voit l'abandon après sa réponse ; une réponse arrivée entre-temps est
            // reprise ici pour l'arrêter.
            abandonne.store(true, Ordering::SeqCst);
            if let Ok(Ok(fil)) = pret_rx.try_recv() {
                // SAFETY : message sans données, à la file du fil du crochet.
                let _ = unsafe { PostThreadMessageW(fil, WM_QUIT, WPARAM(0), LPARAM(0)) };
            }
            Err("crochet pas pose a temps".into())
        }
    }
}

pub fn active(e: &Ecoute) -> bool {
    e.actif.load(Ordering::SeqCst)
}

impl Drop for Ecoute {
    fn drop(&mut self) {
        // SAFETY : message sans données, à la file du fil du crochet.
        let _ = unsafe { PostThreadMessageW(self.fil, WM_QUIT, WPARAM(0), LPARAM(0)) };
        if self.fini.recv_timeout(ARRET_MAX).is_ok() {
            if let Some(p) = self.poignee.take() {
                let _ = p.join();
            }
        } else {
            log::warn!("defiler molette: fil du crochet sans reponse a l'arret");
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tests Windows, un à la fois (verrou `VERROU`) : ils posent des fenêtres, bougent le
    //! pointeur et envoient des molettes. Les fenêtres d'essai sont en bas à droite de
    //! l'écran principal, loin de celles du Gel (en haut à gauche).

    use super::*;
    use std::sync::atomic::{AtomicI32, AtomicIsize, AtomicUsize};
    use std::sync::{Mutex, MutexGuard, Once};

    use ::windows::core::w;
    use ::windows::Win32::Foundation::{GetLastError, ERROR_CLASS_ALREADY_EXISTS};
    use ::windows::Win32::UI::HiDpi::{
        SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };
    use ::windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowRect, LoadCursorW, RegisterClassW,
        SetCursorPos, IDC_ARROW, SB_BOTTOM, SB_LINEDOWN, SB_LINEUP, SB_PAGEDOWN, SB_PAGEUP,
        SB_THUMBPOSITION, SB_THUMBTRACK, SB_TOP, SIF_POS, WM_VSCROLL, WNDCLASSW, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
    };

    use crate::systeme::defiler as facade;
    use crate::systeme::gel::{self, Coins};

    static VERROU: Mutex<()> = Mutex::new(());
    static ECHELLE: Once = Once::new();

    fn seance() -> MutexGuard<'static, ()> {
        ECHELLE.call_once(|| {
            // Comme l'app (manifeste). Un échec veut dire que c'est déjà posé.
            // SAFETY : sans tampon.
            let _ = unsafe {
                SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
            };
        });
        VERROU.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Attend `condition` au plus `delai`.
    fn attendre(delai: Duration, condition: impl Fn() -> bool) -> bool {
        let fin = Instant::now() + delai;
        while Instant::now() < fin {
            if condition() {
                return true;
            }
            thread::sleep(Duration::from_millis(10));
        }
        condition()
    }

    const CLASSE: PCWSTR = w!("BreezyShotEssaiDefiler");
    const LARGEUR: i32 = 320;
    const HAUTEUR: i32 = 240;
    /// Hauteur de la page d'essai, en pixels.
    const CONTENU: i32 = 6000;
    /// Pixels par cran de molette (`WM_MOUSEWHEEL`).
    const PAR_CRAN: i32 = 40;
    /// Pixels par ligne (`WM_VSCROLL`, `SB_LINEDOWN` : ce qu'UI Automation envoie).
    const PAR_LIGNE: i32 = 20;

    /// La fenêtre qui défile (son HWND) ; l'autre est la sentinelle.
    static PAGE: AtomicIsize = AtomicIsize::new(0);
    static POSITION_PAGE: AtomicI32 = AtomicI32::new(0);
    static POSITION_SENTINELLE: AtomicI32 = AtomicI32::new(0);

    fn regler_barre(h: HWND, position: i32) {
        ::windows::core::link!("user32.dll" "system" fn SetScrollInfo(hwnd: HWND, nbar: i32, lpsi: *const SCROLLINFO, redraw: BOOL) -> i32);
        let info = SCROLLINFO {
            cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
            fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
            nMin: 0,
            nMax: CONTENU - 1,
            nPage: HAUTEUR as u32,
            nPos: position,
            nTrackPos: 0,
        };
        // SAFETY : fenêtre de ce fil ; SCROLLINFO dont la taille est renseignée.
        unsafe {
            SetScrollInfo(h, SB_VERT.0, &info, BOOL(1));
        }
    }

    /// Défile de `d` pixels, borné à la page.
    fn bouger(h: HWND, d: i32) {
        let position = if brut(h) == PAGE.load(Ordering::SeqCst) {
            &POSITION_PAGE
        } else {
            &POSITION_SENTINELLE
        };
        let nouvelle = (position.load(Ordering::SeqCst) + d).clamp(0, CONTENU - HAUTEUR);
        position.store(nouvelle, Ordering::SeqCst);
        regler_barre(h, nouvelle);
    }

    unsafe extern "system" fn procedure(
        h: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_MOUSEWHEEL => {
                let delta = (wparam.0 >> 16) as u16 as i16 as i32;
                bouger(h, -(delta / CRAN) * PAR_CRAN);
                LRESULT(0)
            }
            WM_VSCROLL => {
                let actuelle = if brut(h) == PAGE.load(Ordering::SeqCst) {
                    POSITION_PAGE.load(Ordering::SeqCst)
                } else {
                    POSITION_SENTINELLE.load(Ordering::SeqCst)
                };
                let commande = (wparam.0 & 0xFFFF) as i32;
                let d = match commande {
                    c if c == SB_LINEDOWN.0 => PAR_LIGNE,
                    c if c == SB_LINEUP.0 => -PAR_LIGNE,
                    c if c == SB_PAGEDOWN.0 => HAUTEUR,
                    c if c == SB_PAGEUP.0 => -HAUTEUR,
                    c if c == SB_TOP.0 => -actuelle,
                    c if c == SB_BOTTOM.0 => CONTENU,
                    c if c == SB_THUMBTRACK.0 || c == SB_THUMBPOSITION.0 => {
                        ((wparam.0 >> 16) & 0xFFFF) as i32 - actuelle
                    }
                    _ => 0,
                };
                bouger(h, d);
                LRESULT(0)
            }
            // SAFETY : traitement par défaut.
            _ => unsafe { DefWindowProcW(h, message, wparam, lparam) },
        }
    }

    /// La page d'essai (barre verticale, `WM_MOUSEWHEEL`, `WM_VSCROLL`) et, juste à sa
    /// gauche, une sentinelle de même sorte qui ne doit jamais bouger. Leur fil à elles
    /// tourne une boucle de messages (UI Automation leur parle depuis un autre fil).
    struct Fenetres {
        page: isize,
        sentinelle: isize,
        fil: u32,
        poignee: Option<JoinHandle<()>>,
    }

    impl Fenetres {
        fn ouvrir() -> Fenetres {
            let principal = gel::ecrans()
                .into_iter()
                .find(|e| e.principal)
                .expect("un ecran principal");
            let u = principal.zone_utile;
            let page = Rect {
                x: u.x + u.largeur - LARGEUR - 40,
                y: u.y + u.hauteur - HAUTEUR - 40,
                largeur: LARGEUR,
                hauteur: HAUTEUR,
            };
            let sentinelle = Rect {
                x: page.x - LARGEUR,
                ..page
            };
            POSITION_PAGE.store(0, Ordering::SeqCst);
            POSITION_SENTINELLE.store(0, Ordering::SeqCst);
            let (tx, rx) = mpsc::channel();
            let poignee = thread::spawn(move || {
                // SAFETY : classe de ce module ; fenêtres de ce fil, détruites à la fin.
                unsafe {
                    let instance: HINSTANCE = GetModuleHandleW(PCWSTR::null()).unwrap().into();
                    let classe = WNDCLASSW {
                        lpfnWndProc: Some(procedure),
                        hInstance: instance,
                        lpszClassName: CLASSE,
                        hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                        ..Default::default()
                    };
                    if RegisterClassW(&classe) == 0 {
                        assert_eq!(GetLastError(), ERROR_CLASS_ALREADY_EXISTS);
                    }
                    let creer = |r: Rect| {
                        CreateWindowExW(
                            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                            CLASSE,
                            w!(""),
                            WS_POPUP | WS_VSCROLL | WS_VISIBLE,
                            r.x,
                            r.y,
                            r.largeur,
                            r.hauteur,
                            None,
                            None,
                            Some(instance),
                            None,
                        )
                        .unwrap()
                    };
                    let p = creer(page);
                    PAGE.store(brut(p), Ordering::SeqCst);
                    regler_barre(p, 0);
                    let s = creer(sentinelle);
                    regler_barre(s, 0);
                    tx.send((brut(p), brut(s), GetCurrentThreadId())).unwrap();
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                    let _ = DestroyWindow(p);
                    let _ = DestroyWindow(s);
                }
            });
            let (page, sentinelle, fil) = rx
                .recv_timeout(Duration::from_secs(5))
                .expect("fenetres d'essai creees");
            thread::sleep(Duration::from_millis(100));
            Fenetres {
                page,
                sentinelle,
                fil,
                poignee: Some(poignee),
            }
        }

        fn info(&self, h: isize) -> FenetreInfo {
            let mut r = RECT::default();
            // SAFETY : RECT de ce fil.
            unsafe { GetWindowRect(hwnd(h), &mut r) }.unwrap();
            FenetreInfo {
                rect: rect_de(r),
                titre: String::new(),
                coins: Coins::Droits,
                fenetre: h,
            }
        }

        fn zone(&self) -> Rect {
            zone_cliente(hwnd(self.page)).expect("zone cliente de la page")
        }
    }

    impl Drop for Fenetres {
        fn drop(&mut self) {
            // SAFETY : message sans données.
            let _ = unsafe { PostThreadMessageW(self.fil, WM_QUIT, WPARAM(0), LPARAM(0)) };
            if let Some(p) = self.poignee.take() {
                let _ = p.join();
            }
        }
    }

    fn envoyer_molette(delta: i32, extra: usize) {
        let cran = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    mouseData: delta as u32,
                    dwFlags: MOUSEEVENTF_WHEEL,
                    dwExtraInfo: extra,
                    ..Default::default()
                },
            },
        };
        // SAFETY : une entrée dont la taille est passée.
        let n = unsafe { SendInput(&[cran], std::mem::size_of::<INPUT>() as i32) };
        assert_eq!(n, 1, "SendInput");
    }

    fn poser_pointeur(p: (i32, i32)) {
        // SAFETY : sans tampon.
        let _ = unsafe { SetCursorPos(p.0, p.1) };
    }

    #[test]
    fn pages_trouve_la_fenetre_qui_defile_sans_sa_barre() {
        let _s = seance();
        let f = Fenetres::ouvrir();
        let zone = f.zone();
        assert!(
            zone.largeur < LARGEUR,
            "la barre verticale n'est pas dans la page"
        );
        let infos = [f.info(f.page), f.info(f.sentinelle)];
        let debut = Instant::now();
        let liste = facade::pages(&infos, 8);
        let duree = debut.elapsed();
        assert_eq!(
            liste.first(),
            Some(&PageTrouvee {
                rect: zone,
                fenetre: f.page
            }),
            "{liste:?}"
        );
        assert!(liste.iter().any(|p| p.fenetre == f.sentinelle), "{liste:?}");
        assert!(duree < BUDGET_PAGES, "pages en {duree:?}");
        // La page devant cache la partie de la sentinelle qu'elle couvre.
        let devant = FenetreInfo {
            rect: Rect {
                x: infos[1].rect.x + 100,
                ..infos[1].rect
            },
            ..f.info(f.page)
        };
        let liste = facade::pages(&[devant, f.info(f.sentinelle)], 8);
        let sentinelle = liste
            .iter()
            .find(|p| p.fenetre == f.sentinelle)
            .expect("sentinelle en partie visible");
        assert!(sentinelle.rect.droite() <= (infos[1].rect.x + 100) as i64);
    }

    #[test]
    fn message_poste_fait_defiler_la_page_pas_la_sentinelle() {
        let _s = seance();
        let f = Fenetres::ouvrir();
        let zone = f.zone();
        let mut d = facade::preparer(zone, Some(f.page)).unwrap();
        assert_eq!(d.moyen(), Some(Moyen::Message));
        assert_eq!(d.descendre(100).unwrap(), Envoi::Envoye);
        assert!(
            attendre(Duration::from_secs(2), || POSITION_PAGE
                .load(Ordering::SeqCst)
                > 0),
            "la page n'a pas bougé"
        );
        // Premier pas : un cran.
        assert_eq!(POSITION_PAGE.load(Ordering::SeqCst), PAR_CRAN);
        assert_eq!(d.envoyes(), 1);
        // Les images ont vu 40 px : le pas suivant de 100 px fait deux crans.
        d.constater(PAR_CRAN as u32);
        assert_eq!(d.descendre(100).unwrap(), Envoi::Envoye);
        assert!(attendre(Duration::from_secs(2), || {
            POSITION_PAGE.load(Ordering::SeqCst) == 3 * PAR_CRAN
        }));
        // Remonter.
        assert_eq!(d.descendre(-40).unwrap(), Envoi::Envoye);
        assert!(attendre(Duration::from_secs(2), || {
            POSITION_PAGE.load(Ordering::SeqCst) == 2 * PAR_CRAN
        }));
        // Gardé : abandonner ne change plus de moyen.
        assert_eq!(d.abandonner(), Some(Moyen::Message));
        assert_eq!(d.descendre(0).unwrap(), Envoi::Saute);
        thread::sleep(Duration::from_millis(100));
        assert_eq!(
            POSITION_SENTINELLE.load(Ordering::SeqCst),
            0,
            "la sentinelle a bougé"
        );
    }

    #[test]
    fn automation_fait_defiler_et_lit_la_position() {
        let _s = seance();
        let f = Fenetres::ouvrir();
        let mut d = facade::preparer(f.zone(), Some(f.page)).unwrap();
        // « Rien n'a bougé » : le moyen suivant.
        let suivant = d.abandonner();
        if suivant == Some(Moyen::Automation) {
            assert_eq!(d.descendre(50).unwrap(), Envoi::Envoye);
            assert!(
                attendre(Duration::from_secs(2), || POSITION_PAGE
                    .load(Ordering::SeqCst)
                    > 0),
                "la page n'a pas bougé par UI Automation"
            );
            assert!(
                attendre(Duration::from_secs(2), || d
                    .position()
                    .is_some_and(|p| p.pour_cent > 0.0)),
                "position lue {:?}",
                d.position()
            );
        } else {
            // Sans `ScrollPattern` (proxy d'UI Automation absent) : la molette suit.
            eprintln!("fenetre d'essai sans ScrollPattern : moyen suivant {suivant:?}");
            assert_eq!(suivant, Some(Moyen::Molette));
            assert_eq!(d.position(), None);
        }
        thread::sleep(Duration::from_millis(100));
        assert_eq!(
            POSITION_SENTINELLE.load(Ordering::SeqCst),
            0,
            "la sentinelle a bougé"
        );
    }

    #[test]
    fn molette_envoyee_seulement_le_pointeur_sur_la_zone() {
        let _s = seance();
        let f = Fenetres::ouvrir();
        let zone = f.zone();
        let mut d = facade::preparer(zone, Some(f.page)).unwrap();
        for _ in 0..3 {
            if d.moyen() == Some(Moyen::Molette) {
                break;
            }
            d.abandonner();
        }
        assert_eq!(d.moyen(), Some(Moyen::Molette));
        // Le pointeur sur la sentinelle : rien ne part.
        poser_pointeur(centre(zone_cliente(hwnd(f.sentinelle)).unwrap()));
        assert_eq!(d.descendre(100).unwrap(), Envoi::Saute);
        thread::sleep(Duration::from_millis(100));
        assert_eq!(POSITION_SENTINELLE.load(Ordering::SeqCst), 0);
        assert_eq!(POSITION_PAGE.load(Ordering::SeqCst), 0);
        // Le pointeur sur la zone : part si la garde le permet (« faire défiler les
        // fenêtres inactives » mis ; la fenêtre d'essai ne prend jamais le premier plan).
        let survol = survol_actif(routage());
        let mut parti = false;
        for _ in 0..30 {
            poser_pointeur(centre(zone));
            if d.descendre(100).unwrap() == Envoi::Envoye {
                parti = true;
                break;
            }
            // Une autre fenêtre de l'essai est peut-être passée dessus : on retente.
            thread::sleep(Duration::from_millis(100));
        }
        if survol {
            assert!(parti, "molette jamais envoyee sur la zone");
            assert!(
                attendre(Duration::from_secs(2), || POSITION_PAGE
                    .load(Ordering::SeqCst)
                    > 0),
                "la page n'a pas bougé par la molette"
            );
        } else {
            eprintln!("faire defiler les fenetres inactives coupe : molette retenue");
            assert!(!parti);
        }
        thread::sleep(Duration::from_millis(100));
        assert_eq!(
            POSITION_SENTINELLE.load(Ordering::SeqCst),
            0,
            "la sentinelle a bougé"
        );
    }

    #[test]
    fn preparer_refuse_une_zone_vide() {
        let _s = seance();
        assert!(facade::preparer(Rect::default(), None).is_err());
    }

    #[test]
    fn ecoute_entend_un_cran_sans_marque_et_ignore_le_cran_marque() {
        let _s = seance();
        // Les crans vont à la page d'essai, qui les prend sans rien déranger.
        let f = Fenetres::ouvrir();
        poser_pointeur(centre(f.zone()));
        let (tx, rx) = mpsc::channel();
        let ecoute = facade::ecouter_molette(move |m| {
            let _ = tx.send(m);
        })
        .unwrap();
        assert!(ecoute.active());
        envoyer_molette(CRAN, MARQUE); // vers le haut, de l'app
        envoyer_molette(-CRAN, 0); // vers le bas, de « l'utilisateur »
        let m = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("cran sans marque entendu");
        assert_eq!(m.crans, 1, "le premier entendu est le cran sans marque");
        assert!(
            rx.recv_timeout(Duration::from_millis(300)).is_err(),
            "le cran marqué n'est pas entendu"
        );
        drop(ecoute);
    }

    /// Dit quand le rappel est détruit (son fil est fini).
    struct Temoin(Arc<AtomicBool>);

    impl Drop for Temoin {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn le_crochet_est_retire_a_la_destruction() {
        let _s = seance();
        let f = Fenetres::ouvrir();
        poser_pointeur(centre(f.zone()));
        let avant = CROCHETS.load(Ordering::SeqCst);
        let detruit = Arc::new(AtomicBool::new(false));
        let temoin = Temoin(Arc::clone(&detruit));
        let recus = Arc::new(AtomicUsize::new(0));
        let compte = Arc::clone(&recus);
        let ecoute = facade::ecouter_molette(move |_| {
            let _ = &temoin;
            compte.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
        assert!(ecoute.active());
        assert_eq!(CROCHETS.load(Ordering::SeqCst), avant + 1);
        envoyer_molette(-CRAN, 0);
        assert!(attendre(Duration::from_secs(2), || recus
            .load(Ordering::SeqCst)
            == 1));
        drop(ecoute);
        // `Drop` a attendu le fil du crochet : le crochet est retiré.
        assert_eq!(
            CROCHETS.load(Ordering::SeqCst),
            avant,
            "crochet encore pose"
        );
        assert!(
            attendre(Duration::from_secs(2), || detruit.load(Ordering::SeqCst)),
            "le fil du rappel ne s'est pas arrete"
        );
        envoyer_molette(-CRAN, 0);
        thread::sleep(Duration::from_millis(300));
        assert_eq!(recus.load(Ordering::SeqCst), 1);
    }
}
