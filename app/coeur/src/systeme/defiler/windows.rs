//! Défiler une autre app sous Windows (chantier B, 0.6).
//!
//! - Pages sous un clic (S4, §18) : les fenêtres filles d'abord (rendu de Chromium
//!   `Chrome_RenderWidgetHostHWND`, liste de l'Explorateur `DirectUIHWND` dans
//!   `SHELLDLL_DefView`, fenêtre à barre verticale qui a de quoi défiler, sa zone cliente
//!   sans la barre) : lectures du système, sans message, quelques millisecondes. UI
//!   Automation (`ScrollPattern`, `VerticallyScrollable`) seulement pour les fenêtres où rien
//!   n'est trouvé (WPF, WinUI) et qui ne sont pas écartées (barre des tâches), une fenêtre
//!   par fil d'un bassin de `FILS_PAGES` fils MTA durables (fil
//!   `defilante-pages-automation`, client créé une fois par fil, à la première visée
//!   défilante, `preparer_pages`, une création à la fois ; raté, il est recréé à la demande
//!   suivante ; un fil sans client rend la demande à la file et s'en va tant qu'un autre en
//!   a un, n'y répond par une erreur que si aucun n'en a ; une fouille ratée sur une erreur
//!   du client est refaite une fois, sur un client neuf créé avant d'écarter l'ancien ; un
//!   fil s'en va après `INACTIF_MAX` sans demande, compté à part pour chacun) : une
//!   fenêtre lente ne retient pas les autres. Parcours en largeur de la vue des contrôles,
//!   borné (`pages_d_une_fenetre`). Première liste au plus tard à `BUDGET_PAGES`, pages
//!   tardives ajoutées jusqu'à `PAGES_TARDIVES_MAX` (`pages.rs`). Journal : `defiler
//!   pages:` (forme de l'essai 14), `defiler pages detail:` (lancement, COM et client,
//!   classe, durée et éléments de chaque fenêtre), `defiler pages tardives:`, `defiler
//!   pages fin:`. Une page fille est coupée à la zone cliente de ses ancêtres et privée de
//!   ses sœurs plus haut dans l'ordre d'empilement.
//! - Moyens (R3) : `WM_MOUSEWHEEL` posté à la fenêtre fille sous le centre de la zone, avec
//!   les coordonnées écran du centre (en pixels logiques pour une app non adaptée au DPI) ;
//!   UI Automation `Scroll(SmallIncrement)` ; molette `SendInput` marquée (`MARQUE`). Chaque
//!   pas passe par la garde pure `garde` (touches, fenêtre sous le centre ou le pointeur,
//!   focus, menu, capture) ; la cible et sa racine sont comparées à leur processus, leur fil
//!   et leur classe lus à `preparer` (HWND réutilisé). Jamais de touches, jamais
//!   `SetScrollPercent`.
//! - UI Automation vit dans le fil `defilante-automation`, un par défileur, entré dans le
//!   MTA (`CoInitializeEx` MTA, rendu à sa fin ; l'app tient peut-être déjà le MTA par
//!   `CoIncrementMTAUsage`, `systeme/texte` : ce fil s'y joint). Ses objets COM n'en
//!   sortent jamais ; le défileur lui parle par messages, chacun avec son échéance : le fil
//!   laisse une demande en retard sans l'exécuter, et arrête un pas à l'échéance entre deux
//!   `Scroll`. Le défileur n'attend jamais la recherche (`Saute` tant qu'elle court, au plus
//!   `RECHERCHE_MAX`), 80 ms une position, 200 ms un pas. `AutoSetFocus` est coupé : UI
//!   Automation ne prend jamais le focus à la page. Chromium n'ouvre son arbre
//!   d'accessibilité qu'à la première demande (0,3 à 0,4 s mesurés) : la recherche est
//!   refaite deux fois, à 400 ms d'écart.
//! - Molette de l'utilisateur (R4) : crochet `WH_MOUSE_LL` posé dans le fil
//!   `defilante-molette` (priorité « temps critique »), qui n'a que sa boucle de messages ;
//!   le crochet ne fait que mettre le cran dans un canal (jamais bloquant) ; le rappel est
//!   appelé par le fil `defilante-molette-rappel`, jamais après la destruction de l'écoute.
//!   Un rappel lent ne retarde donc jamais le crochet (`LowLevelHooksTimeout` le retirerait
//!   sans prévenir).
//!
//! Journal : classes de fenêtre coupées au premier `[`, nombres et codes seulement (jamais
//! un titre).

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex, TryLockError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use ::windows::core::{Interface, BOOL, HRESULT, PCWSTR};
use ::windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use ::windows::Win32::Graphics::Gdi::{ClientToScreen, ScreenToClient};
use ::windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use ::windows::Win32::System::LibraryLoader::GetModuleHandleW;
use ::windows::Win32::System::Threading::{
    GetCurrentThread, GetCurrentThreadId, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL,
};
use ::windows::Win32::System::Variant::VARIANT;
use ::windows::Win32::UI::Accessibility::{
    CUIAutomation, CUIAutomation8, IUIAutomation, IUIAutomation2, IUIAutomationCacheRequest,
    IUIAutomationCondition, IUIAutomationElement, IUIAutomationScrollPattern,
    IUIAutomationTreeWalker, ScrollAmount_NoAmount, ScrollAmount_SmallDecrement,
    ScrollAmount_SmallIncrement, TreeScope_Children, TreeScope_Descendants,
    UIA_BoundingRectanglePropertyId, UIA_ControlTypePropertyId,
    UIA_IsScrollPatternAvailablePropertyId, UIA_ScrollBarControlTypeId, UIA_ScrollPatternId,
    UIA_ScrollVerticalScrollPercentPropertyId, UIA_ScrollVerticalViewSizePropertyId,
    UIA_ScrollVerticallyScrollablePropertyId, UIA_PROPERTY_ID,
};
use ::windows::Win32::UI::HiDpi::PhysicalToLogicalPointForPerMonitorDPI;
use ::windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_WHEEL, MOUSEINPUT,
    VIRTUAL_KEY, VK_CONTROL, VK_LBUTTON, VK_LWIN, VK_MBUTTON, VK_MENU, VK_RBUTTON, VK_RWIN,
    VK_SHIFT, VK_XBUTTON1, VK_XBUTTON2,
};
use ::windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, ChildWindowFromPointEx, DispatchMessageW, EnumChildWindows, GetAncestor,
    GetClassNameW, GetClientRect, GetCursorPos, GetForegroundWindow, GetGUIThreadInfo, GetMessageW,
    GetScrollInfo, GetWindow, GetWindowLongW, GetWindowRect, GetWindowThreadProcessId, IsChild,
    IsWindow, IsWindowVisible, PeekMessageW, PostMessageW, PostThreadMessageW, SetWindowsHookExW,
    SystemParametersInfoW, TranslateMessage, UnhookWindowsHookEx, WindowFromPoint,
    CWP_SKIPDISABLED, CWP_SKIPINVISIBLE, CWP_SKIPTRANSPARENT, GA_PARENT, GA_ROOT, GUITHREADINFO,
    GUI_INMENUMODE, GUI_INMOVESIZE, GUI_POPUPMENUMODE, GWL_EXSTYLE, GWL_STYLE, GW_HWNDPREV,
    HC_ACTION, MSG, MSLLHOOKSTRUCT, PM_NOREMOVE, SB_VERT, SCROLLINFO, SIF_PAGE, SIF_RANGE,
    SPI_GETMOUSEWHEELROUTING, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WH_MOUSE_LL, WM_MOUSEWHEEL,
    WM_QUIT, WM_USER, WS_EX_TRANSPARENT, WS_VSCROLL,
};

use super::pages::{
    a_fouiller, ecartee, ligne_detail, ligne_fin, ligne_pages, ligne_tardives,
    refaire_avec_un_client_neuf, EtatAutomation, Lancement, Rates, Regard, Suivi, Vue,
};
use super::{
    barre_utile, centre, classe_pour_journal, cran_entendu, decalage_px, garde,
    meilleur_recouvrement, page_par_classe, parametres_molette, plus_petit_sous, position_lue,
    retirer, sans_barre, survol_actif, Choix, Constat, Depart, Envoi, Etalon, FenetreInfo, Molette,
    Moyen, PageTrouvee, Position, RappelMolette, Rect, CRAN, MARQUE, PAGES_TARDIVES_MAX,
};

/// Attente d'une position lue par UI Automation.
const ATTENTE_POSITION: Duration = Duration::from_millis(80);

/// Attente de la réponse d'un pas d'UI Automation. Au-delà, le pas est `Saute` (il a pu
/// partir en partie : les images le diront) ; `RETARDS_MAX` fois de suite, le moyen est
/// laissé.
const ATTENTE_PAS: Duration = Duration::from_millis(200);

/// Le fil d'UI Automation ne commence plus rien à moins de ceci de l'échéance d'une
/// demande : sa réponse arrive avant que le défileur cesse de l'attendre.
const MARGE: Duration = Duration::from_millis(30);

/// Pas d'UI Automation sans réponse à temps, de suite, avant de laisser le moyen.
const RETARDS_MAX: u32 = 3;

/// Au plus tant de sœurs regardées par ancêtre d'une page fille.
const SOEURS_MAX: usize = 256;

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

/// Attente, en tout, de la fin d'un rappel en cours et de l'arrêt du fil du crochet (`Drop`
/// d'`Ecoute`).
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

/// Le processus, le fil et la classe d'une fenêtre : un HWND réutilisé par une autre
/// fenêtre ne les a pas tous trois.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Identite {
    processus: u32,
    fil: u32,
    classe: String,
}

fn identite(h: HWND) -> Option<Identite> {
    let mut processus = 0u32;
    // SAFETY : u32 de ce fil ; lecture d'état, sans message.
    let fil = unsafe { GetWindowThreadProcessId(h, Some(&mut processus)) };
    (fil != 0).then(|| Identite {
        processus,
        fil,
        classe: classe(h),
    })
}

/// Le rectangle de la fenêtre `h` en pixels du bureau.
fn rect_fenetre(h: HWND) -> Option<Rect> {
    let mut r = RECT::default();
    // SAFETY : RECT de ce fil ; lecture d'état, sans message.
    unsafe { GetWindowRect(h, &mut r) }.ok()?;
    let rect = rect_de(r);
    (!rect.est_vide()).then_some(rect)
}

/// Une sœur qui cache ce qui est sous elle : visible, non traversée par les clics, et pas la
/// fenêtre `Intermediate D3D Window` que Chromium pose sur sa page.
fn couvre(h: HWND) -> bool {
    // SAFETY : lectures d'état, sans message.
    let (visible, etendu) = unsafe { (IsWindowVisible(h), GetWindowLongW(h, GWL_EXSTYLE)) };
    visible.as_bool()
        && (etendu as u32) & WS_EX_TRANSPARENT.0 == 0
        && classe(h) != "Intermediate D3D Window"
}

/// Ce qui reste visible de `rect`, la zone d'une fenêtre fille `h` : coupée à la zone
/// cliente de chaque ancêtre jusqu'à la racine, privée des sœurs plus haut qu'elle (et que
/// chaque ancêtre) dans l'ordre d'empilement. `None` si rien ne reste.
fn part_visible(h: HWND, rect: Rect) -> Option<Rect> {
    let racine = racine_de(h);
    let mut reste = rect;
    let mut courante = h;
    for _ in 0..32 {
        if courante == racine || courante.is_invalid() {
            break;
        }
        let mut soeur = courante;
        for _ in 0..SOEURS_MAX {
            // SAFETY : lecture d'état, sans message.
            let Ok(s) = (unsafe { GetWindow(soeur, GW_HWNDPREV) }) else {
                break;
            };
            if s.is_invalid() {
                break;
            }
            soeur = s;
            if couvre(s) {
                if let Some(b) = rect_fenetre(s) {
                    reste = retirer(reste, b)?;
                }
            }
        }
        // SAFETY : idem.
        let parent = unsafe { GetAncestor(courante, GA_PARENT) };
        if parent.is_invalid() {
            break;
        }
        reste = reste.intersection(zone_cliente(parent)?)?;
        courante = parent;
    }
    Some(reste)
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
        if let Some(rect) = zone_cliente(h).and_then(|r| part_visible(h, r)) {
            filles.liste.push(PageTrouvee {
                rect,
                fenetre: brut(h),
            });
        }
    }
    BOOL(1)
}

/// Les pages de la fenêtre de premier niveau `racine` qui se voient par les fenêtres :
/// elle-même si elle a une barre verticale, puis ses descendantes (coupées à ce qui se voit
/// d'elles dans la racine).
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

pub fn preparer_pages() {
    let b = bassin();
    let mut etat = b.etat();
    garnir(&b, &mut etat);
}

pub fn chercher_pages(
    fenetres: &[FenetreInfo],
    max: usize,
    depart: Depart,
    rappel: &mut dyn FnMut(&[PageTrouvee]),
) {
    let debut = Instant::now();
    let n = fenetres.len().min(max);
    let fenetres = &fenetres[..n];
    let bords: Vec<Rect> = fenetres.iter().map(|f| f.rect).collect();
    let mut vues: Vec<Vue> = Vec::with_capacity(n);
    let mut recues: Vec<Option<Vec<PageTrouvee>>> = Vec::with_capacity(n);
    let mut manquent: Vec<(usize, isize)> = Vec::new();
    for (i, f) in fenetres.iter().enumerate() {
        if f.fenetre == 0 {
            vues.push(Vue {
                classe: "aucune".into(),
                regard: Regard::SansFenetre,
            });
            recues.push(Some(Vec::new()));
            continue;
        }
        let h = hwnd(f.fenetre);
        let c = classe(h);
        let filles = pages_filles(h);
        let regard = if !filles.is_empty() {
            Regard::Filles {
                pages: filles.len(),
            }
        } else if ecartee(&c) {
            Regard::Ecartee
        } else {
            manquent.push((i, f.fenetre));
            Regard::EnFile
        };
        recues.push((!matches!(regard, Regard::EnFile)).then_some(filles));
        vues.push(Vue {
            classe: classe_pour_journal(&c).to_owned(),
            regard,
        });
    }
    let duree_filles = debut.elapsed();
    let mut reception = None;
    let mut bassin_demande = None;
    if !manquent.is_empty() {
        let (tx, rx) = mpsc::channel();
        let echeance = debut + PAGES_TARDIVES_MAX;
        let b = bassin();
        let rates_avant = demander(
            &b,
            manquent.iter().map(|&(i, h)| DemandePage {
                i,
                h,
                echeance,
                reponse: tx.clone(),
            }),
        );
        bassin_demande = Some((b, rates_avant));
        // Seules les demandes tiennent l'envoi : toutes servies ou laissées, la réception
        // le voit.
        drop(tx);
        reception = Some(rx);
    }
    let mut lancement = Lancement {
        fil: depart.fil.saturating_duration_since(depart.demandee),
        liste: debut.saturating_duration_since(depart.fil),
        filles: duree_filles,
        automation: EtatAutomation::Inutile,
        rates: None,
    };
    // L'état d'UI Automation et les clients ratés, lus au moment d'écrire (ceux de cette
    // recherche compris).
    let lire = |l: &mut Lancement| {
        if let Some((b, avant)) = &bassin_demande {
            let etat = b.etat();
            l.automation = etat.automation(Instant::now());
            l.rates = etat.rates(*avant);
        }
    };
    let mut suivi = Suivi::nouveau(bords, recues);
    let mut premiere = true;
    let mut retard_au_budget = false;
    let mut taille = 0;
    loop {
        if let Some(liste) = suivi.proposer(debut.elapsed()) {
            taille = liste.len();
            if premiere {
                log::info!("{}", ligne_pages(taille, &vues, debut.elapsed()));
                lire(&mut lancement);
                log::info!("{}", ligne_detail(&lancement, &vues));
                retard_au_budget = vues.iter().any(|v| v.regard.en_retard());
                premiere = false;
            } else {
                log::info!("{}", ligne_tardives(taille, &vues, debut.elapsed()));
            }
            rappel(&liste);
        }
        if suivi.fini(debut.elapsed()) {
            break;
        }
        let Some(rx) = reception.as_ref() else {
            // Rien à attendre (aucune fenêtre pour UI Automation) : `proposer` a tout rendu.
            break;
        };
        let reste = suivi.echeance().saturating_sub(debut.elapsed());
        match rx.recv_timeout(reste) {
            Ok(Nouvelle::Debut { i, a }) => {
                if let Some(v) = vues.get_mut(i).filter(|v| v.regard == Regard::EnFile) {
                    v.regard = Regard::EnCours {
                        depuis: a.saturating_duration_since(debut),
                    };
                }
            }
            Ok(Nouvelle::Fin { i, pages, fouille }) => {
                if let Some(v) = vues.get_mut(i) {
                    v.regard = fouille.regard(pages.len(), debut);
                }
                suivi.recevoir(i, pages, debut.elapsed());
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                // Plus personne ne répondra (demandes laissées à leur échéance, fils
                // arrêtés) : ce qui manque reste en retard au journal.
                suivi.abandonner();
                reception = None;
            }
        }
    }
    // La fin, si une fenêtre était en retard au budget ou si des clients ont raté depuis.
    let pendant_avant = lancement.rates.as_ref().map_or(0, |r| r.pendant);
    lire(&mut lancement);
    let rates_apres = lancement
        .rates
        .as_ref()
        .filter(|r| r.pendant > pendant_avant);
    if retard_au_budget || rates_apres.is_some() {
        log::info!(
            "{}",
            ligne_fin(taille, &vues, debut.elapsed(), lancement.rates.as_ref())
        );
    }
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
    match auto.cast::<IUIAutomation2>() {
        Ok(a2) => {
            // SAFETY : réglages du client, sans effet sur les pages.
            let poses = unsafe {
                let _ = a2.SetAutoSetFocus(false);
                a2.SetConnectionTimeout(DELAI_UIA_MS)
                    .and_then(|()| a2.SetTransactionTimeout(DELAI_UIA_MS))
            };
            // Sans eux, un appel peut attendre bien plus que `DELAI_UIA_MS` : dit une fois
            // par client, la recherche rend quand même à temps (`recv_timeout`).
            if let Err(e) = poses {
                log::info!("defiler automation: delais non poses {:#010X}", e.code().0);
            }
        }
        Err(_) => log::info!("defiler automation: client sans delais (IUIAutomation2 absent)"),
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

// ---------------------------------------------------------------------------------------
// UI Automation des pages : un bassin de fils MTA durables, chacun avec son client.

/// Fils du bassin : une fenêtre qui bloque (jusqu'à `DELAI_UIA_MS` par appel) n'en retient
/// qu'un, les autres fenêtres passent par les autres.
const FILS_PAGES: usize = 4;

/// Un fil du bassin sans demande pendant ce délai (attente du verrou de la file comprise)
/// s'en va avec son client (la licence a pu être retirée) ; la visée suivante le recrée.
const INACTIF_MAX: Duration = Duration::from_secs(60);

/// Au plus tant d'éléments ouverts (une demande de leurs enfants chacun) par fenêtre.
const NOEUDS_MAX: u32 = 48;

/// Au plus tant d'enfants regardés par élément ouvert.
const ENFANTS_MAX: i32 = 256;

/// Au plus tant de niveaux sous la fenêtre.
const PROFONDEUR_MAX: u32 = 12;

/// Au plus tant de pages par fenêtre.
const PAGES_PAR_FENETRE: usize = 16;

/// Les derniers enfants d'une page regardés pour y trouver ses barres de défilement (WPF :
/// la barre verticale puis l'horizontale, après le contenu).
const DERNIERS_ENFANTS: usize = 3;

/// Une fenêtre à fouiller, et où répondre.
struct DemandePage {
    /// Son rang dans la liste de la recherche.
    i: usize,
    h: isize,
    /// Au-delà, plus personne n'attend : une demande pas commencée est laissée, une fouille
    /// en cours s'arrête entre deux appels.
    echeance: Instant,
    reponse: Sender<Nouvelle>,
}

/// Ce que le bassin dit d'une demande.
enum Nouvelle {
    Debut {
        i: usize,
        a: Instant,
    },
    Fin {
        i: usize,
        pages: Vec<PageTrouvee>,
        fouille: Fouille,
    },
}

/// Ce qu'a coûté la fouille d'une fenêtre.
#[derive(Debug, Default)]
struct Fouille {
    duree: Duration,
    fin: Option<Instant>,
    elements: u32,
    noeuds: u32,
    coupee: bool,
    erreur: Option<String>,
    /// Le code de l'erreur d'UI Automation, s'il y en a un (pour refaire la fouille).
    echec: Option<u32>,
}

impl Fouille {
    fn erreur(e: String) -> Fouille {
        Fouille {
            erreur: Some(e),
            ..Fouille::default()
        }
    }

    fn regard(self, pages: usize, debut: Instant) -> Regard {
        match self.erreur {
            Some(code) => Regard::Erreur {
                duree: self.duree,
                code,
            },
            None => Regard::Automation {
                duree: self.duree,
                fin: self
                    .fin
                    .map(|f| f.saturating_duration_since(debut))
                    .unwrap_or_default(),
                elements: self.elements,
                noeuds: self.noeuds,
                pages,
                coupee: self.coupee,
            },
        }
    }
}

/// Ce que le bassin sait de ses fils (sous son verrou `etat`).
#[derive(Debug)]
struct EtatBassin {
    /// Fils lancés au plus (`FILS_PAGES` ; moins pour un essai).
    fils_max: usize,
    /// Fils vivants (comptés au lancement, décomptés à leur départ, panique comprise).
    vivants: usize,
    /// Fils vivants qui ont un client.
    avec_client: usize,
    /// Fils qui ont fini leur première création, réussie ou non, depuis `debut`.
    prepares: usize,
    /// Le bassin s'est (re)mis à préparer ses fils à cet instant (aucun fil vivant avant).
    debut: Instant,
    /// Le premier client prêt depuis `debut` : COM, création, et l'instant.
    premier: Option<(Duration, Duration, Instant)>,
    /// Créations de client ratées depuis la création du bassin, et la dernière.
    rates: u32,
    dernier_rate: Option<String>,
}

impl EtatBassin {
    /// Où en est UI Automation, pour le journal : prêt tant qu'un fil a un client ;
    /// impossible si tous ceux qui ont essayé ont raté ; en préparation sinon.
    fn automation(&self, maintenant: Instant) -> EtatAutomation {
        match self.premier {
            Some((com, client, a)) if self.avec_client > 0 => EtatAutomation::Prete {
                com,
                client,
                depuis: maintenant.saturating_duration_since(a),
            },
            _ if self.prepares > 0 && self.avec_client == 0 => EtatAutomation::Impossible(
                self.dernier_rate
                    .clone()
                    .unwrap_or_else(|| "aucun client".into()),
            ),
            _ => EtatAutomation::EnPreparation {
                depuis: maintenant.saturating_duration_since(self.debut),
            },
        }
    }

    /// Les clients ratés, dont ceux venus depuis que le compteur valait `avant`.
    fn rates(&self, avant: u32) -> Option<Rates> {
        self.dernier_rate.as_ref().map(|d| Rates {
            total: self.rates,
            pendant: self.rates.saturating_sub(avant),
            dernier: d.clone(),
        })
    }
}

/// Le bassin : la file des demandes et ce qu'il sait de ses fils, partagés par eux.
struct Bassin {
    envoi: Sender<DemandePage>,
    file: Mutex<Receiver<DemandePage>>,
    etat: Mutex<EtatBassin>,
    /// Essais : `Some(None)`, le premier fil qui crée un client rate toujours ;
    /// `Some(Some(fil))`, ce fil-là.
    #[cfg(test)]
    maudit: Mutex<Option<Option<thread::ThreadId>>>,
}

impl Bassin {
    fn nouveau(fils_max: usize) -> Arc<Bassin> {
        let (envoi, file) = mpsc::channel();
        Arc::new(Bassin {
            envoi,
            file: Mutex::new(file),
            etat: Mutex::new(EtatBassin {
                fils_max,
                vivants: 0,
                avec_client: 0,
                prepares: 0,
                debut: Instant::now(),
                premier: None,
                rates: 0,
                dernier_rate: None,
            }),
            #[cfg(test)]
            maudit: Mutex::new(None),
        })
    }

    fn etat(&self) -> std::sync::MutexGuard<'_, EtatBassin> {
        self.etat.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Essais : vrai si ce fil doit rater sa création de client.
    #[cfg(test)]
    fn rater_ici(&self) -> bool {
        let mut m = self.maudit.lock().unwrap_or_else(|e| e.into_inner());
        let ici = thread::current().id();
        match *m {
            None => false,
            Some(None) => {
                *m = Some(Some(ici));
                true
            }
            Some(Some(fil)) => fil == ici,
        }
    }
}

/// Créé à la première visée défilante (`preparer_pages`), jamais sans licence, puis gardé.
/// Ordre des verrous : la file, puis l'état (un fil qui s'en va) ; jamais l'inverse.
static BASSIN: Mutex<Option<Arc<Bassin>>> = Mutex::new(None);

fn bassin() -> Arc<Bassin> {
    let mut g = BASSIN.lock().unwrap_or_else(|e| e.into_inner());
    Arc::clone(g.get_or_insert_with(|| Bassin::nouveau(FILS_PAGES)))
}

/// Lance les fils qui manquent. Sous le verrou de l'état.
fn garnir(b: &Arc<Bassin>, etat: &mut EtatBassin) {
    if etat.vivants == 0 {
        etat.debut = Instant::now();
        etat.premier = None;
        etat.prepares = 0;
        etat.avec_client = 0;
    }
    while etat.vivants < etat.fils_max {
        let pour = Arc::clone(b);
        let lance = thread::Builder::new()
            .name("defilante-pages-automation".into())
            .spawn(move || fil_du_bassin(pour));
        match lance {
            Ok(_) => etat.vivants += 1,
            Err(e) => {
                log::warn!("defiler pages: fil d'automation impossible ({e})");
                break;
            }
        }
    }
}

/// Envoie les demandes au bassin, puis le complète (après l'envoi : un fil qui s'en va
/// sous le verrou de l'état a vu la demande, ou il est déjà décompté et relancé ici). Rend
/// le compteur des clients ratés avant cette recherche.
fn demander(b: &Arc<Bassin>, demandes: impl Iterator<Item = DemandePage>) -> u32 {
    for d in demandes {
        let _ = b.envoi.send(d);
    }
    let mut etat = b.etat();
    garnir(b, &mut etat);
    etat.rates
}

/// Ce dont une fouille a besoin, créé une fois par fil du bassin.
struct Client {
    auto: IUIAutomation,
    vrai: IUIAutomationCondition,
    /// Rectangle, type de contrôle, `ScrollPattern` présent, défile verticalement :
    /// ramenés avec chaque élément (aucun aller-retour de plus pour les lire).
    cache: IUIAutomationCacheRequest,
    marcheur: IUIAutomationTreeWalker,
}

/// Les créations de clients, une à la fois dans tout le processus. Supposé, pas établi :
/// quatre fils qui créent chacun le leur au même instant, à la première visée, ont pu
/// rendre l'`E_FAIL` du passage 98 (l'étape de l'échec est maintenant au journal).
static CREATION: Mutex<()> = Mutex::new(());

#[cfg(test)]
thread_local! {
    /// Essais : tant de créations de client à faire rater sur ce fil.
    static CLIENTS_RATES: Cell<u32> = const { Cell::new(0) };
}

/// Essais de création d'un client, et leur écart.
const CREATIONS_CLIENT: u32 = 2;
const ENTRE_CREATIONS_CLIENT: Duration = Duration::from_millis(20);

impl Client {
    /// Un client, ou l'étape et le code de l'échec (`client <étape> <code>`).
    fn creer() -> Result<Client, String> {
        let _une_a_la_fois = CREATION.lock().unwrap_or_else(|e| e.into_inner());
        #[cfg(test)]
        if CLIENTS_RATES.with(|c| {
            let n = c.get();
            c.set(n.saturating_sub(1));
            n > 0
        }) {
            return Err("client essai 0x80004005".into());
        }
        let etape = |nom: &str| {
            let nom = nom.to_owned();
            move |e: ::windows::core::Error| format!("client {nom} {}", code(&e))
        };
        let auto = automation().map_err(etape("CoCreateInstance"))?;
        // SAFETY : appels COM du fil du client.
        unsafe {
            let vrai = auto.CreateTrueCondition().map_err(etape("condition"))?;
            let cache = auto.CreateCacheRequest().map_err(etape("cache"))?;
            for p in [
                UIA_BoundingRectanglePropertyId,
                UIA_ControlTypePropertyId,
                UIA_IsScrollPatternAvailablePropertyId,
                UIA_ScrollVerticallyScrollablePropertyId,
            ] {
                cache.AddProperty(p).map_err(etape("propriete"))?;
            }
            let marcheur = auto.ControlViewWalker().map_err(etape("marcheur"))?;
            Ok(Client {
                auto,
                vrai,
                cache,
                marcheur,
            })
        }
    }
}

/// COM et le client d'un fil du bassin, et sa place dans les comptes du bassin. Un client
/// qui manque (création ratée) est recréé à la demande suivante ; un client n'est remplacé
/// qu'une fois le neuf créé. Détruit (fin du fil, panique comprise), il décompte ce qui ne
/// l'a pas encore été.
struct Poste {
    // Le client part avant COM (ordre des champs : ordre de destruction).
    client: Result<Client, String>,
    com: Result<Com, String>,
    /// `None` : un poste d'essai, hors de tout bassin.
    bassin: Option<Arc<Bassin>>,
    /// Compté dans `vivants` (lancé par `garnir`).
    compte_vivant: bool,
    /// Compté dans `avec_client`.
    compte_client: bool,
}

impl Poste {
    /// COM (MTA) et un client, pour un fil lancé par `garnir` dans `bassin` (`None` : un
    /// essai).
    fn ouvrir(bassin: Option<Arc<Bassin>>) -> Poste {
        let debut = Instant::now();
        let com = Com::ouvrir();
        let duree_com = debut.elapsed();
        let mut poste = Poste {
            client: Err("pas encore cree".into()),
            com,
            compte_vivant: bassin.is_some(),
            bassin,
            compte_client: false,
        };
        poste.client = poste.creer();
        let duree_client = debut.elapsed().saturating_sub(duree_com);
        if let Some(b) = &poste.bassin {
            let mut etat = b.etat();
            etat.prepares += 1;
            if poste.client.is_ok() {
                if etat.premier.is_none() || etat.avec_client == 0 {
                    etat.premier = Some((duree_com, duree_client, Instant::now()));
                }
                etat.avec_client += 1;
                poste.compte_client = true;
            }
        }
        poste
    }

    /// Un client sur ce fil (COM ouvert) : `CREATIONS_CLIENT` essais, chaque échec noté au
    /// bassin.
    fn creer(&self) -> Result<Client, String> {
        if let Err(e) = &self.com {
            return Err(e.clone());
        }
        let mut dernier = String::new();
        for essai in 0..CREATIONS_CLIENT {
            if essai > 0 {
                thread::sleep(ENTRE_CREATIONS_CLIENT);
            }
            #[cfg(test)]
            let essaye = match &self.bassin {
                Some(b) if b.rater_ici() => Err("client essai 0x80004005".to_owned()),
                _ => Client::creer(),
            };
            #[cfg(not(test))]
            let essaye = Client::creer();
            match essaye {
                Ok(c) => return Ok(c),
                Err(e) => {
                    if let Some(b) = &self.bassin {
                        let mut etat = b.etat();
                        etat.rates += 1;
                        etat.dernier_rate = Some(e.clone());
                    }
                    dernier = e;
                }
            }
        }
        Err(dernier)
    }

    /// Le client du fil, recréé s'il manque.
    fn client(&mut self) -> Result<&Client, String> {
        if self.client.is_err() {
            let debut = Instant::now();
            let neuf = self.creer();
            if neuf.is_ok() && !self.compte_client {
                if let Some(b) = &self.bassin {
                    let mut etat = b.etat();
                    if etat.avec_client == 0 {
                        etat.premier = Some((Duration::ZERO, debut.elapsed(), Instant::now()));
                    }
                    etat.avec_client += 1;
                }
                self.compte_client = self.bassin.is_some();
            }
            self.client = neuf;
        }
        self.client.as_ref().map_err(Clone::clone)
    }

    /// Un client neuf à la place de celui du fil, seulement s'il se crée ; sinon l'ancien
    /// reste, et `None`.
    fn remplacer(&mut self) -> Option<&Client> {
        let neuf = self.creer().ok()?;
        self.client = Ok(neuf);
        self.client.as_ref().ok()
    }

    /// Le fil s'en va : décompté, sous le verrou de l'état que tient l'appelant.
    fn partir(&mut self, etat: &mut EtatBassin) {
        if self.compte_vivant {
            etat.vivants = etat.vivants.saturating_sub(1);
            self.compte_vivant = false;
        }
        if self.compte_client {
            etat.avec_client = etat.avec_client.saturating_sub(1);
            self.compte_client = false;
        }
    }
}

impl Drop for Poste {
    fn drop(&mut self) {
        if !(self.compte_vivant || self.compte_client) {
            return;
        }
        if let Some(b) = self.bassin.clone() {
            let mut etat = b.etat();
            self.partir(&mut etat);
        }
    }
}

/// Un fil du bassin : COM (MTA) et son client une fois, puis les demandes, jusqu'à
/// `INACTIF_MAX` sans demande pour lui (compté depuis sa dernière demande, attente du
/// verrou de la file comprise : les fils s'en vont ensemble).
fn fil_du_bassin(b: Arc<Bassin>) {
    let mut poste = Poste::ouvrir(Some(Arc::clone(&b)));
    let mut actif = Instant::now();
    loop {
        let demande = {
            let rx = b.file.lock().unwrap_or_else(|e| e.into_inner());
            let reste = (actif + INACTIF_MAX).saturating_duration_since(Instant::now());
            match rx.recv_timeout(reste) {
                Ok(d) => d,
                Err(_) => {
                    // Sous le verrou de l'état : une demande envoyée avant est vue ici, une
                    // envoyée après trouve un fil de moins et en relance un.
                    let mut etat = b.etat();
                    match rx.try_recv() {
                        Ok(d) => d,
                        Err(_) => {
                            poste.partir(&mut etat);
                            break;
                        }
                    }
                }
            }
        };
        match servir(&mut poste, demande) {
            Suite::Servie => {}
            Suite::SansClient(d, e) => {
                // Un autre fil a un client : la demande retourne à la file pour lui, et ce
                // fil s'en va (la visée suivante en relance un). Aucun fil n'en a : l'erreur
                // est répondue tout de suite, et ce fil reste pour réessayer.
                let mut etat = b.etat();
                if etat.avec_client > 0 {
                    let _ = b.envoi.send(d);
                    poste.partir(&mut etat);
                    break;
                }
                drop(etat);
                repondre_sans_client(d, e);
            }
        }
        actif = Instant::now();
    }
}

/// Ce qu'il advient d'une demande prise par un fil.
enum Suite {
    /// Fouillée et répondue, ou laissée (échéance passée, plus personne n'attend).
    Servie,
    /// Pas de client sur ce fil, même recréé : rien n'a été répondu.
    SansClient(DemandePage, String),
}

/// Répond l'erreur de création du client pour la demande `d`.
fn repondre_sans_client(d: DemandePage, e: String) {
    let maintenant = Instant::now();
    let _ = d.reponse.send(Nouvelle::Fin {
        i: d.i,
        pages: Vec::new(),
        fouille: Fouille {
            erreur: Some(e),
            fin: Some(maintenant),
            ..Fouille::default()
        },
    });
}

/// Une demande : laissée si son échéance est passée, sinon fouillée et répondue. Le
/// client du fil est recréé s'il manque ; s'il manque encore, la demande revient à
/// l'appelant sans réponse (`Suite::SansClient`) : un fil sans client ne répond jamais une
/// erreur à la place d'un fil qui en a un (passage 98). Une fouille refusée pour une raison
/// du client est refaite une fois avec un client neuf (`refaire_avec_un_client_neuf`).
fn servir(poste: &mut Poste, d: DemandePage) -> Suite {
    let debut = Instant::now();
    if debut >= d.echeance {
        return Suite::Servie;
    }
    if let Err(e) = poste.client() {
        return Suite::SansClient(d, e);
    }
    if d.reponse
        .send(Nouvelle::Debut { i: d.i, a: debut })
        .is_err()
    {
        // Plus personne n'attend cette recherche.
        return Suite::Servie;
    }
    let h = hwnd(d.h);
    let mut resultat = match poste.client() {
        Ok(c) => pages_d_une_fenetre(c, h, d.echeance),
        Err(e) => (Vec::new(), Fouille::erreur(e)),
    };
    if let Some(echec) = resultat.1.echec {
        if refaire_avec_un_client_neuf(echec, existe(h), Instant::now() < d.echeance) {
            if let Some(c) = poste.remplacer() {
                resultat = pages_d_une_fenetre(c, h, d.echeance);
            }
        }
    }
    let (pages, mut fouille) = resultat;
    fouille.duree = debut.elapsed();
    fouille.fin = Some(Instant::now());
    let _ = d.reponse.send(Nouvelle::Fin {
        i: d.i,
        pages,
        fouille,
    });
    Suite::Servie
}

/// Un booléen ramené dans le cache de `e` (faux s'il manque ou n'est pas un booléen).
fn booleen_cache(e: &IUIAutomationElement, propriete: UIA_PROPERTY_ID) -> bool {
    // SAFETY : lecture du cache de l'élément, sans appel à la page.
    unsafe { e.GetCachedPropertyValue(propriete) }
        .ok()
        .and_then(|v| bool::try_from(&v).ok())
        .unwrap_or(false)
}

/// `e` a un `ScrollPattern` et défile verticalement (d'après son cache).
fn defile_cache(e: &IUIAutomationElement) -> bool {
    booleen_cache(e, UIA_IsScrollPatternAvailablePropertyId)
        && booleen_cache(e, UIA_ScrollVerticallyScrollablePropertyId)
}

/// Les rectangles des barres de défilement parmi les `DERNIERS_ENFANTS` derniers enfants de
/// `element` (vue des contrôles) : quelques appels, au lieu de parcourir son contenu.
fn barres_de(c: &Client, element: &IUIAutomationElement, echeance: Instant) -> Vec<Rect> {
    let mut barres = Vec::new();
    if Instant::now() >= echeance {
        return barres;
    }
    // SAFETY : appels COM du fil du client.
    let mut courant = unsafe { c.marcheur.GetLastChildElementBuildCache(element, &c.cache) }.ok();
    for _ in 0..DERNIERS_ENFANTS {
        let Some(e) = courant else {
            break;
        };
        if Instant::now() >= echeance {
            break;
        }
        // SAFETY : lectures du cache de l'élément.
        let (genre, rect) = unsafe { (e.CachedControlType(), e.CachedBoundingRectangle()) };
        if let (true, Ok(r)) = (genre.is_ok_and(|g| g == UIA_ScrollBarControlTypeId), rect) {
            barres.push(rect_de(r));
        }
        // SAFETY : appel COM du fil du client.
        courant = unsafe { c.marcheur.GetPreviousSiblingElementBuildCache(&e, &c.cache) }.ok();
    }
    barres
}

/// Ce qu'UI Automation trouve de défilable dans la fenêtre `h` (S4, complément) : un
/// parcours en largeur de la vue des contrôles, une demande d'enfants par élément ouvert
/// (propriétés ramenées dans le cache), sans ouvrir ce qui défile (c'est la page) ni les
/// feuilles (`a_fouiller`). Borné par `echeance`, `NOEUDS_MAX`, `ENFANTS_MAX`,
/// `PROFONDEUR_MAX` et `PAGES_PAR_FENETRE`. Un `FindAll(TreeScope_Subtree)` lisait chaque
/// élément de la page (un témoin WPF de 120 à 400 images : autant d'allers-retours par le
/// fil de l'app) avant de rendre quoi que ce soit.
fn pages_d_une_fenetre(c: &Client, h: HWND, echeance: Instant) -> (Vec<PageTrouvee>, Fouille) {
    let mut fouille = Fouille::default();
    let mut pages = Vec::new();
    // SAFETY : appel COM du fil du client.
    let racine = match unsafe { c.auto.ElementFromHandleBuildCache(h, &c.cache) } {
        Ok(r) => r,
        Err(e) => {
            fouille.erreur = Some(code(&e));
            fouille.echec = Some(e.code().0 as u32);
            return (pages, fouille);
        }
    };
    fouille.elements = 1;
    let mut file: VecDeque<(IUIAutomationElement, u32)> = VecDeque::new();
    if defile_cache(&racine) {
        garder_page(c, &racine, h, echeance, &mut pages);
    } else {
        file.push_back((racine, 0));
    }
    'fouille: while let Some((element, profondeur)) = file.pop_front() {
        if Instant::now() >= echeance || fouille.noeuds >= NOEUDS_MAX {
            fouille.coupee = true;
            break;
        }
        fouille.noeuds += 1;
        // SAFETY : appel COM du fil du client.
        let Ok(enfants) =
            (unsafe { element.FindAllBuildCache(TreeScope_Children, &c.vrai, &c.cache) })
        else {
            continue;
        };
        // SAFETY : idem.
        let n = unsafe { enfants.Length() }
            .unwrap_or(0)
            .clamp(0, ENFANTS_MAX);
        for k in 0..n {
            // SAFETY : indice dans la liste.
            let Ok(enfant) = (unsafe { enfants.GetElement(k) }) else {
                continue;
            };
            fouille.elements += 1;
            if defile_cache(&enfant) {
                garder_page(c, &enfant, h, echeance, &mut pages);
                if pages.len() >= PAGES_PAR_FENETRE {
                    break 'fouille;
                }
                if Instant::now() >= echeance {
                    fouille.coupee = true;
                    break 'fouille;
                }
                continue;
            }
            // SAFETY : lecture du cache de l'élément.
            let genre = unsafe { enfant.CachedControlType() }.map_or(0, |t| t.0);
            if profondeur + 1 < PROFONDEUR_MAX && a_fouiller(genre) {
                file.push_back((enfant, profondeur + 1));
            }
        }
    }
    (pages, fouille)
}

/// Garde `element` (qui défile) comme page de la fenêtre `h`, sans sa barre verticale
/// (cherchée seulement avant `echeance` : au-delà, la page garde sa barre).
fn garder_page(
    c: &Client,
    element: &IUIAutomationElement,
    h: HWND,
    echeance: Instant,
    pages: &mut Vec<PageTrouvee>,
) {
    // SAFETY : lecture du cache de l'élément.
    let Ok(rect) = (unsafe { element.CachedBoundingRectangle() }).map(rect_de) else {
        return;
    };
    if rect.est_vide() {
        return;
    }
    let barres = barres_de(c, element, echeance);
    pages.push(PageTrouvee {
        rect: sans_barre(rect, &barres),
        fenetre: brut(h),
    });
}

/// Le `ScrollPattern` de la zone, et la hauteur de son élément.
struct Trouve {
    element: IUIAutomationElement,
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
            element: element.clone(),
            motif,
            hauteur: rect.hauteur,
        })
    }
}

/// Cherche l'élément qui défile sous `point` : la fenêtre cible elle-même, puis ses
/// descendants (parmi ceux qui contiennent le point, celui qui recouvre le mieux `zone` :
/// la page plutôt qu'un bloc intérieur), puis la même chose depuis la racine.
fn chercher(
    auto: &IUIAutomation,
    cible: isize,
    racine: isize,
    point: (i32, i32),
    zone: Rect,
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
            if let Ok(r) = unsafe { e.CurrentBoundingRectangle() }.map(rect_de) {
                if r.contient(point) {
                    rects.push(r);
                    elements.push(e);
                }
            }
        }
        if let Some(i) = meilleur_recouvrement(&rects, zone) {
            if let Some(t) = defilable_sous(&elements[i], point) {
                return Some(t);
            }
        }
    }
    None
}

/// Les trois propriétés de la position, le rectangle de l'élément et le `ScrollPattern`,
/// ramenés en une seule demande à l'autre app (Chromium répond lentement à chaque lecture).
fn cache_position(auto: &IUIAutomation) -> ::windows::core::Result<IUIAutomationCacheRequest> {
    // SAFETY : appels COM du fil du client.
    unsafe {
        let cache = auto.CreateCacheRequest()?;
        cache.AddPattern(UIA_ScrollPatternId)?;
        cache.AddProperty(UIA_ScrollVerticallyScrollablePropertyId)?;
        cache.AddProperty(UIA_ScrollVerticalScrollPercentPropertyId)?;
        cache.AddProperty(UIA_ScrollVerticalViewSizePropertyId)?;
        cache.AddProperty(UIA_BoundingRectanglePropertyId)?;
        Ok(cache)
    }
}

/// La position et la hauteur de la vue, en une demande (`BuildUpdatedCache`) ; à défaut,
/// en lectures une à une (hauteur illisible : celle de la recherche).
fn lire_position(t: &Trouve, cache: Option<&IUIAutomationCacheRequest>) -> Option<Position> {
    // SAFETY : appels COM du fil du client.
    let en_cache = cache.and_then(|c| unsafe {
        let element = t.element.BuildUpdatedCache(c).ok()?;
        let motif: IUIAutomationScrollPattern =
            element.GetCachedPatternAs(UIA_ScrollPatternId).ok()?;
        Some((
            motif.CachedVerticallyScrollable().ok()?.as_bool(),
            motif.CachedVerticalScrollPercent().ok()?,
            motif.CachedVerticalViewSize().ok()?,
            element
                .CachedBoundingRectangle()
                .ok()
                .map(|r| rect_de(r).hauteur),
        ))
    });
    let (defilable, pour_cent, vue, hauteur) = match en_cache {
        Some(lu) => lu,
        // SAFETY : idem.
        None => unsafe {
            (
                t.motif.CurrentVerticallyScrollable().ok()?.as_bool(),
                t.motif.CurrentVerticalScrollPercent().ok()?,
                t.motif.CurrentVerticalViewSize().ok()?,
                t.element
                    .CurrentBoundingRectangle()
                    .ok()
                    .map(|r| rect_de(r).hauteur),
            )
        },
    };
    let hauteur = hauteur.filter(|h| *h > 0).unwrap_or(t.hauteur);
    position_lue(defilable, pour_cent, vue, hauteur)
}

/// Jusqu'à `unites` pas `SmallIncrement` (négatif : `SmallDecrement`), aucun commencé après
/// `echeance` ; rend ceux qui ont réussi. `Err` seulement si l'élément n'existe plus avant
/// le premier pas ; toute autre erreur est laissée aux images.
fn faire_defiler(
    motif: &IUIAutomationScrollPattern,
    unites: i32,
    echeance: Instant,
) -> Result<u32, String> {
    let sens = if unites > 0 {
        ScrollAmount_SmallIncrement
    } else {
        ScrollAmount_SmallDecrement
    };
    let mut faits = 0;
    for k in 0..unites.unsigned_abs() {
        if Instant::now() >= echeance {
            break;
        }
        // SAFETY : appel COM du fil du client.
        match unsafe { motif.Scroll(ScrollAmount_NoAmount, sens) } {
            Ok(()) => faits += 1,
            Err(e) if k == 0 && e.code() == ELEMENT_DISPARU => {
                return Err(format!("Scroll {}", code(&e)));
            }
            Err(_) => break,
        }
    }
    Ok(faits)
}

/// Ce que le défileur demande au fil d'UI Automation. L'`Instant` est l'échéance : passée,
/// la demande est laissée sans être exécutée (personne n'attend plus sa réponse).
enum Demande {
    Position(Instant, SyncSender<Option<Position>>),
    Defiler(i32, Instant, SyncSender<Result<u32, String>>),
}

impl Demande {
    fn en_retard(&self) -> bool {
        let echeance = match self {
            Demande::Position(e, _) | Demande::Defiler(_, e, _) => *e,
        };
        Instant::now() >= echeance
    }
}

/// Réponse quand aucun `ScrollPattern` n'est (encore) là.
fn repondre_sans(d: Demande) {
    match d {
        Demande::Position(_, r) => {
            let _ = r.send(None);
        }
        Demande::Defiler(_, _, r) => {
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
    zone: Rect,
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
    let point = centre(zone);
    let debut = Instant::now();
    let mut trouve = None;
    let mut essai = 0;
    while essai < essais {
        essai += 1;
        trouve = chercher(&auto, cible, racine, point, zone);
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
            let cache = cache_position(&auto).ok();
            while let Ok(d) = demandes.recv() {
                if d.en_retard() {
                    continue;
                }
                match d {
                    Demande::Position(_, r) => {
                        let _ = r.send(lire_position(&t, cache.as_ref()));
                    }
                    Demande::Defiler(n, echeance, r) => {
                        let _ = r.send(faire_defiler(&t.motif, n, echeance));
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
/// l'attendre). Rien ici n'attend la recherche.
#[derive(Debug)]
struct Automation {
    demandes: Sender<Demande>,
    recherche: Receiver<Option<i32>>,
    etat: Cell<Recherche>,
    depuis: Instant,
    /// Pas sans réponse à temps, de suite.
    retards: Cell<u32>,
}

impl Automation {
    fn lancer(cible: isize, racine: isize, zone: Rect, chromium: bool) -> Option<Automation> {
        let (dtx, drx) = mpsc::channel();
        let (rtx, rrx) = mpsc::sync_channel(1);
        let lance = thread::Builder::new()
            .name("defilante-automation".into())
            .spawn(move || travailler(cible, racine, zone, chromium, drx, rtx));
        match lance {
            Ok(_) => Some(Automation {
                demandes: dtx,
                recherche: rrx,
                etat: Cell::new(Recherche::EnCours),
                depuis: Instant::now(),
                retards: Cell::new(0),
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

    /// L'état de la recherche, sans attendre ; encore en cours après `RECHERCHE_MAX` :
    /// `Absente`.
    fn etat(&self) -> Recherche {
        if self.etat.get() == Recherche::EnCours {
            match self.recherche.try_recv() {
                Ok(r) => return self.noter(r),
                Err(TryRecvError::Disconnected) => return self.noter(None),
                Err(TryRecvError::Empty) if self.depuis.elapsed() >= RECHERCHE_MAX => {
                    log::info!("defiler automation: recherche trop longue, laissee");
                    return self.noter(None);
                }
                Err(TryRecvError::Empty) => {}
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

    /// La position lue, attendue au plus `ATTENTE_POSITION`.
    fn position(&self) -> Option<Position> {
        self.hauteur()?;
        let (tx, rx) = mpsc::sync_channel(1);
        let echeance = Instant::now() + ATTENTE_POSITION - MARGE;
        self.demandes.send(Demande::Position(echeance, tx)).ok()?;
        rx.recv_timeout(ATTENTE_POSITION).ok().flatten()
    }

    /// `unites` pas, attendus au plus `ATTENTE_PAS` : `Some(faits)`, ou `None` sans réponse à
    /// temps. `Err` : le fil est arrêté, l'élément a disparu, ou `RETARDS_MAX` pas de suite
    /// sont restés sans réponse.
    fn defiler(&self, unites: i32) -> Result<Option<u32>, String> {
        let (tx, rx) = mpsc::sync_channel(1);
        let echeance = Instant::now() + ATTENTE_PAS - MARGE;
        self.demandes
            .send(Demande::Defiler(unites, echeance, tx))
            .map_err(|_| "fil d'automation arrete".to_string())?;
        match rx.recv_timeout(ATTENTE_PAS) {
            Ok(r) => {
                self.retards.set(0);
                r.map(Some)
            }
            // Laissée par le fil (en retard) ou pas de réponse à temps.
            Err(_) => {
                let n = self.retards.get() + 1;
                self.retards.set(n);
                if n >= RETARDS_MAX {
                    Err(format!("{n} pas sans reponse"))
                } else {
                    Ok(None)
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Le défileur.

/// La dernière commande partie.
#[derive(Debug, Clone, Copy)]
struct Derniere {
    moyen: Moyen,
    /// Unités parties, avec leur signe (un cran partiel en fraction de cran).
    unites: f64,
    /// Un cran partiel (`Message`).
    partiel: bool,
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
    /// Processus, fil et classe de la cible et de la racine, lus à `preparer`.
    identite_cible: Option<Identite>,
    identite_racine: Option<Identite>,
    choix: Choix,
    etalons: [Etalon; 3],
    automation: Option<Automation>,
    derniere: Option<Derniere>,
    /// L'appelant a déjà dit ce que les images ont vu (`constater`) : la position n'est plus
    /// lue pour régler les pas d'`Automation`.
    par_images: bool,
    /// Un pas sauté pour une touche tenue a été dit au journal (une fois par capture).
    touches_dites: bool,
}

/// Un moyen peut-il servir maintenant ?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dispo {
    Oui,
    /// UI Automation cherche encore (au plus `RECHERCHE_MAX` après `preparer`).
    PasEncore,
    Non,
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

/// Ctrl, Maj, Alt, Win ou un bouton de souris est-il enfoncé ? (Ctrl + molette change le zoom
/// de la page, Maj la fait défiler de côté.)
fn touches_tenues() -> bool {
    const TOUCHES: [VIRTUAL_KEY; 10] = [
        VK_CONTROL,
        VK_SHIFT,
        VK_MENU,
        VK_LWIN,
        VK_RWIN,
        VK_LBUTTON,
        VK_RBUTTON,
        VK_MBUTTON,
        VK_XBUTTON1,
        VK_XBUTTON2,
    ];
    TOUCHES.iter().any(|vk| {
        // SAFETY : lecture d'état, sans message.
        let etat = unsafe { GetAsyncKeyState(i32::from(vk.0)) };
        (etat as u16) & 0x8000 != 0
    })
}

/// L'état d'interface d'un fil (`0` : celui du premier plan).
fn interface_du_fil(fil: u32) -> Option<GUITHREADINFO> {
    let mut info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    // SAFETY : GUITHREADINFO dont la taille est renseignée.
    unsafe { GetGUIThreadInfo(fil, &mut info) }
        .ok()
        .map(|()| info)
}

/// Un menu, une capture de souris, un déplacement ou un redimensionnement en cours :
/// la molette irait ailleurs que sous le pointeur.
fn fil_pris(info: &GUITHREADINFO) -> bool {
    let modes = GUI_INMENUMODE.0 | GUI_POPUPMENUMODE.0 | GUI_INMOVESIZE.0;
    info.flags.0 & modes != 0
        || !info.hwndCapture.is_invalid()
        || !info.hwndMoveSize.is_invalid()
        || !info.hwndMenuOwner.is_invalid()
}

/// Pour la molette envoyée à `h` : le focus de son fil est-il `h`, l'un de ses ancêtres ou
/// l'une de ses descendantes ; son fil, ou celui du premier plan, est-il pris (illisible :
/// pris) ?
fn focus_et_fils(h: HWND) -> (bool, bool) {
    // SAFETY : lecture d'état, sans message.
    let fil = unsafe { GetWindowThreadProcessId(h, None) };
    let Some(info) = (fil != 0).then(|| interface_du_fil(fil)).flatten() else {
        return (false, true);
    };
    let focus = info.hwndFocus;
    // SAFETY : lectures d'état, sans message.
    let dans = !focus.is_invalid()
        && (focus == h || unsafe { IsChild(h, focus).as_bool() || IsChild(focus, h).as_bool() });
    let devant = interface_du_fil(0).is_some_and(|i| fil_pris(&i));
    (dans, fil_pris(&info) || devant)
}

/// `point` (pixels réels) tel que l'attend `h` : en pixels logiques si `h` n'est pas
/// adaptée au DPI (Windows convertit les vraies molettes, pas un message posté).
fn point_pour(h: HWND, point: (i32, i32)) -> (i32, i32) {
    let mut p = POINT {
        x: point.0,
        y: point.1,
    };
    // SAFETY : POINT de ce fil.
    if unsafe { PhysicalToLogicalPointForPerMonitorDPI(Some(h), &mut p) }.as_bool() {
        (p.x, p.y)
    } else {
        point
    }
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
    let identite_cible = cible.and_then(identite);
    let identite_racine = racine.and_then(identite);
    let classe_cible = identite_cible
        .as_ref()
        .map(|i| i.classe.clone())
        .unwrap_or_default();
    let classe_racine = identite_racine
        .as_ref()
        .map(|i| i.classe.clone())
        .unwrap_or_default();
    let chromium = classe_racine.starts_with("Chrome_WidgetWin")
        || classe_cible == "Chrome_RenderWidgetHostHWND";
    let mut choix = Choix::nouveau();
    if identite_cible.is_none() || identite_racine.is_none() {
        choix.exclure(Moyen::Message);
    }
    if identite_racine.is_none() {
        choix.exclure(Moyen::Molette);
    }
    let automation = match (cible, racine) {
        (None, None) => None,
        (c, r) => Automation::lancer(
            c.or(r).map_or(0, brut),
            r.or(c).map_or(0, brut),
            zone,
            chromium,
        ),
    };
    if automation.is_none() {
        choix.exclure(Moyen::Automation);
    }
    log::info!(
        "defiler cible {} racine {} moyen {}",
        classe_pour_journal(&classe_cible),
        classe_pour_journal(&classe_racine),
        choix.moyen().map_or("aucun", Moyen::nom)
    );
    Ok(Defileur {
        zone,
        centre: point,
        cible: cible.map_or(0, brut),
        racine: racine.map_or(0, brut),
        identite_cible,
        identite_racine,
        choix,
        etalons: [Etalon::default(); 3],
        automation,
        derniere: None,
        par_images: false,
        touches_dites: false,
    })
}

impl Defileur {
    /// Le moyen `m` peut-il servir ? Sans attendre.
    fn disponible(&self, m: Moyen) -> Dispo {
        let oui = |b: bool| if b { Dispo::Oui } else { Dispo::Non };
        match m {
            Moyen::Message => oui(self.cible != 0 && existe(hwnd(self.cible))),
            Moyen::Automation => match self.automation.as_ref().map(Automation::etat) {
                Some(Recherche::Trouvee { .. }) => Dispo::Oui,
                Some(Recherche::EnCours) => Dispo::PasEncore,
                _ => Dispo::Non,
            },
            Moyen::Molette => oui(self.racine != 0),
        }
    }

    /// Laisse les moyens en cours qui ne peuvent pas servir ; rend celui qui reste (peut-être
    /// `Automation` dont la recherche court encore). N'attend pas.
    fn assurer(&mut self) -> Option<Moyen> {
        while let Some(m) = self.choix.moyen() {
            if self.disponible(m) != Dispo::Non {
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

    /// La cible et sa racine sont-elles encore les fenêtres de `preparer` (même processus,
    /// même fil, même classe ; la cible encore dans la racine) ?
    fn verifier(&self, m: Moyen) -> Result<(), String> {
        if m == Moyen::Automation {
            return Ok(());
        }
        let racine = hwnd(self.racine);
        if self.racine == 0 || identite(racine) != self.identite_racine {
            return Err("fenetre changee".into());
        }
        if m == Moyen::Message {
            let cible = hwnd(self.cible);
            if self.cible == 0 || !existe(cible) {
                return Err("fenetre fermee".into());
            }
            if identite(cible) != self.identite_cible || racine_de(cible) != racine {
                return Err("fenetre changee".into());
            }
        }
        Ok(())
    }

    /// Ce que la garde de `m` doit savoir, lu maintenant.
    fn constat(&self, m: Moyen) -> Constat {
        let mut c = Constat {
            touches_tenues: touches_tenues(),
            ..Constat::default()
        };
        match m {
            Moyen::Automation => {}
            Moyen::Message => c.racine_sous_centre = racine_au_point(self.centre),
            Moyen::Molette => {
                c.pointeur = curseur();
                c.racine_sous_pointeur = c.pointeur.and_then(racine_au_point);
                c.premier_plan = premier_plan();
                c.survol_actif = survol_actif(routage());
                // La fenêtre fille visée ; fermée, la racine (vérifiée par `verifier`).
                let vise = if self.cible != 0 && existe(hwnd(self.cible)) {
                    self.cible
                } else {
                    self.racine
                };
                (c.focus_dans_cible, c.fil_occupe) = focus_et_fils(hwnd(vise));
            }
        }
        c
    }

    /// Poste `n` fois `WM_MOUSEWHEEL` de `delta` ; rend combien sont partis (au moins un).
    fn par_message(&self, delta: i16, n: u32) -> Result<u32, String> {
        let cible = hwnd(self.cible);
        let (w, l) = parametres_molette(delta, point_pour(cible, self.centre));
        let mut postes = 0;
        for _ in 0..n {
            // SAFETY : message sans pointeur, posté (jamais attendu).
            match unsafe { PostMessageW(Some(cible), WM_MOUSEWHEEL, WPARAM(w), LPARAM(l)) } {
                Ok(()) => postes += 1,
                Err(e) if postes == 0 => return Err(format!("PostMessageW {}", code(&e))),
                Err(_) => break,
            }
        }
        Ok(postes)
    }

    /// Envoie `n` crans de `delta` marqués ; rend combien sont partis (0 : entrée bloquée,
    /// bureau sécurisé, à retenter).
    fn par_molette(&self, delta: i32, n: u32) -> u32 {
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
        let entrees = vec![cran; n as usize];
        // SAFETY : entrées dont la taille est passée.
        unsafe { SendInput(&entrees, std::mem::size_of::<INPUT>() as i32) }
    }

    /// Envoie un pas de `pixels` par `m` (gardes passées). `Ok(None)` : rien n'est parti,
    /// à retenter ; `Err` : `m` ne peut plus servir.
    fn envoyer(&mut self, m: Moyen, pixels: i32) -> Result<Option<Derniere>, String> {
        let etalon = self.etalons[m.rang()];
        let signe = f64::from(pixels.signum());
        let vers_le_bas = pixels > 0;
        let derniere = |unites: f64, partiel: bool, position: Option<Position>| Derniere {
            moyen: m,
            unites: unites * signe,
            partiel,
            position,
        };
        match m {
            Moyen::Message => {
                let (delta, n, partiel) = match etalon.delta_partiel(pixels, self.zone.hauteur) {
                    Some(p) => (p, 1, true),
                    None => (
                        CRAN,
                        etalon.unites(pixels, m.unites_max()).unsigned_abs(),
                        false,
                    ),
                };
                let delta = if vers_le_bas { -delta } else { delta } as i16;
                let postes = self.par_message(delta, n)?;
                let unites = if partiel {
                    f64::from(delta.unsigned_abs()) / f64::from(CRAN)
                } else {
                    f64::from(postes)
                };
                Ok(Some(derniere(unites, partiel, None)))
            }
            Moyen::Automation => {
                let Some(a) = self.automation.as_ref() else {
                    return Err("automation absente".into());
                };
                // Une seule position lue par pas : elle règle le pas précédent et sert
                // d'avant à celui-ci (seulement si l'appelant ne dit rien des images).
                let lue = if self.par_images { None } else { a.position() };
                if let (Some(d), Some(apres)) = (self.derniere.take(), lue) {
                    if let (Moyen::Automation, Some(avant)) = (d.moyen, d.position) {
                        if let Some(px) = decalage_px(&avant, &apres) {
                            self.etalons[m.rang()].constater(d.unites, f64::from(px));
                        }
                    }
                }
                let unites = self.etalons[m.rang()].unites(pixels, m.unites_max());
                Ok(match a.defiler(unites)? {
                    Some(faits) if faits > 0 => Some(derniere(f64::from(faits), false, lue)),
                    _ => None,
                })
            }
            Moyen::Molette => {
                let n = etalon.unites(pixels, m.unites_max()).unsigned_abs();
                let delta = if vers_le_bas { -CRAN } else { CRAN };
                let parties = self.par_molette(delta, n);
                Ok((parties > 0).then(|| derniere(f64::from(parties), false, None)))
            }
        }
    }
}

pub fn moyen(d: &Defileur) -> Option<Moyen> {
    d.choix.moyen()
}

pub fn envoyes(d: &Defileur) -> u32 {
    d.choix.envoyes()
}

pub fn avance_par_cran(d: &Defileur) -> Option<u32> {
    d.choix
        .moyen()
        .and_then(|m| d.etalons[m.rang()].par_unite())
}

pub fn abandonner(d: &mut Defileur) -> Option<Moyen> {
    let avant = d.choix.moyen();
    d.choix.abandonner();
    d.derniere = None;
    let suivant = d.assurer();
    log::info!(
        "defiler {} laisse, suite {}",
        avant.map_or("aucun", Moyen::nom),
        suivant.map_or("aucun", Moyen::nom)
    );
    suivant
}

pub fn constater(d: &mut Defileur, avance: u32) {
    let Some(derniere) = d.derniere.take() else {
        return;
    };
    if d.choix.moyen() != Some(derniere.moyen) {
        return;
    }
    d.par_images = true;
    let etalon = &mut d.etalons[derniere.moyen.rang()];
    if avance == 0 {
        if derniere.partiel {
            etalon.refuser_partiel();
            log::info!("defiler {}: cran partiel sans effet", derniere.moyen.nom());
        }
        return;
    }
    etalon.constater(
        derniere.unites,
        f64::from(avance) * derniere.unites.signum(),
    );
}

pub fn descendre(d: &mut Defileur, pixels: i32) -> Result<Envoi, String> {
    if pixels == 0 {
        return Ok(Envoi::Saute);
    }
    while let Some(m) = d.assurer() {
        if d.disponible(m) == Dispo::PasEncore {
            return Ok(Envoi::Saute);
        }
        let essai = d.verifier(m).and_then(|()| {
            let constat = d.constat(m);
            if garde(m, d.zone, d.racine, &constat) == Envoi::Saute {
                if constat.touches_tenues && !d.touches_dites {
                    d.touches_dites = true;
                    log::info!("defiler pas saute: touche ou bouton vu enfonce (dit une fois)");
                }
                return Ok(None);
            }
            d.envoyer(m, pixels)
        });
        match essai {
            Ok(Some(derniere)) => {
                d.choix.envoye();
                // Un cran partiel compte pour une unité.
                let entieres = if derniere.partiel {
                    1
                } else {
                    derniere.unites.abs().round() as i32
                };
                d.etalons[m.rang()].envoye(entieres);
                d.derniere = Some(derniere);
                return Ok(Envoi::Envoye);
            }
            Ok(None) => return Ok(Envoi::Saute),
            Err(cause) => {
                let suivant = d.choix.exclure(m);
                d.derniere = None;
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
    // Le système attend ce fil à chaque mouvement de souris : il passe avant tout le reste.
    // SAFETY : pseudo-poignée de ce fil.
    if let Err(e) = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL) }
    {
        log::info!(
            "defiler molette: priorite du fil non changee ({})",
            code(&e)
        );
    }
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
    /// Vrai jusqu'au `Drop`, lu avant chaque appel du rappel.
    ouvert: Arc<AtomicBool>,
    /// Tenu par le fil du rappel pendant chaque appel.
    en_appel: Arc<Mutex<()>>,
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
    let ouvert = Arc::new(AtomicBool::new(true));
    let ouvert_fil = Arc::clone(&ouvert);
    let en_appel = Arc::new(Mutex::new(()));
    let en_appel_fil = Arc::clone(&en_appel);
    thread::Builder::new()
        .name("defilante-molette-rappel".into())
        .spawn(move || {
            for m in rx {
                if !ouvert_fil.load(Ordering::SeqCst) {
                    break;
                }
                // Tenu pendant l'appel : `Drop` attend (au plus `ARRET_MAX`) la fin d'un
                // appel commencé ; relu une fois tenu, `Drop` a pu passer entre-temps.
                let _appel = en_appel_fil.lock().unwrap_or_else(|e| e.into_inner());
                if !ouvert_fil.load(Ordering::SeqCst) {
                    break;
                }
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
            ouvert,
            en_appel,
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
            ouvert.store(false, Ordering::SeqCst);
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
        let fin = Instant::now() + ARRET_MAX;
        // Plus aucun rappel ne commence à partir d'ici.
        self.ouvert.store(false, Ordering::SeqCst);
        // SAFETY : message sans données, à la file du fil du crochet.
        let _ = unsafe { PostThreadMessageW(self.fil, WM_QUIT, WPARAM(0), LPARAM(0)) };
        // Un appel commencé est attendu, borné : un rappel qui bloque (ou ce `Drop` appelé
        // du rappel lui-même) ne retient pas la fin de la capture.
        loop {
            match self.en_appel.try_lock() {
                Ok(_) | Err(TryLockError::Poisoned(_)) => break,
                Err(TryLockError::WouldBlock) if Instant::now() >= fin => {
                    log::warn!("defiler molette: rappel encore en cours a l'arret");
                    break;
                }
                Err(TryLockError::WouldBlock) => thread::sleep(Duration::from_millis(5)),
            }
        }
        let reste = fin.saturating_duration_since(Instant::now());
        if self.fini.recv_timeout(reste).is_ok() {
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
    //! Tests Windows, un à la fois : ils posent des fenêtres, bougent le pointeur et envoient
    //! des molettes. Toutes les séances passent par une seule garde, `seance()`. Les
    //! fenêtres d'essai sont en bas à droite de l'écran principal, loin de celles du Gel (en
    //! haut à gauche).

    use super::*;
    use std::sync::atomic::{AtomicI32, AtomicIsize, AtomicUsize};
    use std::sync::{MutexGuard, Once};

    use ::windows::core::w;
    use ::windows::Win32::Foundation::{GetLastError, ERROR_CLASS_ALREADY_EXISTS};
    use ::windows::Win32::UI::HiDpi::{
        SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };
    use ::windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, LoadCursorW, RegisterClassW, SetCursorPos,
        SetWindowPos, HWND_TOP, HWND_TOPMOST, IDC_ARROW, SB_BOTTOM, SB_LINEDOWN, SB_LINEUP,
        SB_PAGEDOWN, SB_PAGEUP, SB_THUMBPOSITION, SB_THUMBTRACK, SB_TOP, SIF_POS, SWP_NOACTIVATE,
        SWP_NOMOVE, SWP_NOSIZE, WINDOW_EX_STYLE, WINDOW_STYLE, WM_VSCROLL, WNDCLASSW, WS_CHILD,
        WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
        WS_POPUP, WS_VISIBLE,
    };

    use crate::systeme::defiler as facade;
    use crate::systeme::defiler::BUDGET_PAGES;
    use crate::systeme::gel::{self, Coins};

    /// Verrou des séances. ASSEMBLAGE : le remplacer par `crate::systeme::gel::VERROU_ECRAN`
    /// (le verrou commun aux tests qui touchent l'écran ou le pointeur, Gel et A), ici
    /// seulement : `seance()` est le seul endroit qui le prend.
    static ECHELLE: Once = Once::new();

    /// La garde de chaque test Windows de ce module.
    fn seance() -> MutexGuard<'static, ()> {
        ECHELLE.call_once(|| {
            // Comme l'app (manifeste). Un échec veut dire que c'est déjà posé.
            // SAFETY : sans tampon.
            let _ = unsafe {
                SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
            };
        });
        // Partagé avec les tests de `gel` et de `vivant` : un seul test d'écran à la fois.
        crate::systeme::gel::VERROU_ECRAN
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    /// Sur GitHub (`BREEZYSHOT_AUTOMATION_OBLIGATOIRE=1`), UI Automation doit marcher et la
    /// molette doit partir : les issues de repli des tests sont interdites.
    fn obligatoire() -> bool {
        std::env::var("BREEZYSHOT_AUTOMATION_OBLIGATOIRE").as_deref() == Ok("1")
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
    /// Pixels par cran de molette (`WM_MOUSEWHEEL`) ; un cran partiel en fait d'autant moins.
    const PAR_CRAN: i32 = 40;
    /// Pixels par ligne (`WM_VSCROLL`, `SB_LINEDOWN` : ce qu'UI Automation envoie).
    const PAR_LIGNE: i32 = 20;
    /// Bande du conteneur au-dessus de la page fille, et hauteur du couvercle posé sur elle.
    const BANDE: i32 = 20;
    const COUVERCLE: i32 = 60;
    /// Une attente du défileur ne dépasse jamais ceci (280 ms promis, plus de quoi tenir
    /// sur une machine chargée).
    const ATTENTE_TENUE: Duration = Duration::from_millis(500);

    /// La fenêtre qui défile (son HWND) ; toutes les autres sont des sentinelles.
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

    /// Défile de `d` pixels, borné à la page. Une sentinelle compte tout mouvement.
    fn bouger(h: HWND, d: i32) {
        if brut(h) != PAGE.load(Ordering::SeqCst) {
            POSITION_SENTINELLE.fetch_add(d.abs().max(1), Ordering::SeqCst);
            return;
        }
        let nouvelle = (POSITION_PAGE.load(Ordering::SeqCst) + d).clamp(0, CONTENU - HAUTEUR);
        POSITION_PAGE.store(nouvelle, Ordering::SeqCst);
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
                bouger(h, -delta * PAR_CRAN / CRAN);
                LRESULT(0)
            }
            WM_VSCROLL => {
                let actuelle = POSITION_PAGE.load(Ordering::SeqCst);
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

    /// Ce que l'essai pose.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Forme {
        /// La page est une fenêtre de premier niveau à barre verticale.
        Simple,
        /// La page est une fenêtre fille à barre verticale, dans un conteneur (sous une
        /// bande de `BANDE` px), en partie couverte en bas par une sœur (`COUVERCLE` px).
        Fille,
    }

    /// La page d'essai (barre verticale, `WM_MOUSEWHEEL`, `WM_VSCROLL`) ; à sa gauche, une
    /// sentinelle de même sorte qui ne doit jamais bouger ; plus à gauche, une fenêtre nue
    /// (sans barre, rien que UI Automation y trouverait). Leur fil à elles tourne une boucle
    /// de messages (UI Automation leur parle depuis un autre fil).
    struct Fenetres {
        page: isize,
        sentinelle: isize,
        nue: isize,
        /// La fenêtre de premier niveau de la page (elle-même en `Simple`).
        racine: isize,
        fil: u32,
        poignee: Option<JoinHandle<()>>,
    }

    impl Fenetres {
        fn ouvrir(forme: Forme) -> Fenetres {
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
            let nue = Rect {
                x: page.x - 2 * LARGEUR,
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
                    let creer = |r: Rect, style: WINDOW_STYLE, parent: Option<HWND>| {
                        let etendu = if parent.is_some() {
                            WINDOW_EX_STYLE(0)
                        } else {
                            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE
                        };
                        CreateWindowExW(
                            etendu,
                            CLASSE,
                            w!(""),
                            style | WS_VISIBLE,
                            r.x,
                            r.y,
                            r.largeur,
                            r.hauteur,
                            parent,
                            None,
                            Some(instance),
                            None,
                        )
                        .unwrap()
                    };
                    let mut creees = Vec::new();
                    let (p, racine) = match forme {
                        Forme::Simple => {
                            let p = creer(page, WS_POPUP | WS_VSCROLL, None);
                            creees.push(p);
                            (p, p)
                        }
                        Forme::Fille => {
                            let c = creer(page, WS_POPUP | WS_CLIPCHILDREN, None);
                            let dedans = Rect {
                                x: 0,
                                y: BANDE,
                                largeur: LARGEUR,
                                hauteur: HAUTEUR - BANDE,
                            };
                            let style = WS_CHILD | WS_CLIPSIBLINGS;
                            let p = creer(dedans, style | WS_VSCROLL, Some(c));
                            let couvercle = Rect {
                                y: HAUTEUR - COUVERCLE,
                                hauteur: COUVERCLE,
                                ..dedans
                            };
                            let k = creer(couvercle, style, Some(c));
                            // Le couvercle au-dessus de la page, dans l'ordre des sœurs.
                            let _ = SetWindowPos(
                                k,
                                Some(HWND_TOP),
                                0,
                                0,
                                0,
                                0,
                                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                            );
                            creees.push(c);
                            (p, c)
                        }
                    };
                    PAGE.store(brut(p), Ordering::SeqCst);
                    regler_barre(p, 0);
                    let s = creer(sentinelle, WS_POPUP | WS_VSCROLL, None);
                    regler_barre(s, 0);
                    let n = creer(nue, WS_POPUP, None);
                    creees.extend([s, n]);
                    tx.send((
                        brut(p),
                        brut(s),
                        brut(n),
                        brut(racine),
                        GetCurrentThreadId(),
                    ))
                    .unwrap();
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                    for h in creees {
                        let _ = DestroyWindow(h);
                    }
                }
            });
            let (page, sentinelle, nue, racine, fil) = rx
                .recv_timeout(Duration::from_secs(5))
                .expect("fenetres d'essai creees");
            thread::sleep(Duration::from_millis(100));
            Fenetres {
                page,
                sentinelle,
                nue,
                racine,
                fil,
                poignee: Some(poignee),
            }
        }

        fn info(&self, h: isize) -> FenetreInfo {
            FenetreInfo {
                rect: rect_fenetre(hwnd(h)).expect("rectangle de la fenetre d'essai"),
                titre: String::new(),
                coins: Coins::Droits,
                fenetre: h,
            }
        }

        fn zone(&self) -> Rect {
            zone_cliente(hwnd(self.page)).expect("zone cliente de la page")
        }

        /// Pose la sentinelle en `(x, y)`, au-dessus de toutes les fenêtres d'essai.
        fn deplacer_sentinelle(&self, x: i32, y: i32) {
            // SAFETY : fenêtre d'essai ; son fil tourne sa boucle de messages.
            unsafe {
                SetWindowPos(
                    hwnd(self.sentinelle),
                    Some(HWND_TOPMOST),
                    x,
                    y,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOACTIVATE,
                )
            }
            .expect("sentinelle deplacee");
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

    fn position_page() -> i32 {
        POSITION_PAGE.load(Ordering::SeqCst)
    }

    fn sentinelles_immobiles() {
        thread::sleep(Duration::from_millis(100));
        assert_eq!(
            POSITION_SENTINELLE.load(Ordering::SeqCst),
            0,
            "une sentinelle a bougé"
        );
    }

    /// `descendre`, en vérifiant qu'il n'attend pas.
    fn pas(d: &mut facade::Defileur, pixels: i32) -> Envoi {
        let debut = Instant::now();
        let envoi = d.descendre(pixels).unwrap();
        let duree = debut.elapsed();
        assert!(duree < ATTENTE_TENUE, "descendre a attendu {duree:?}");
        envoi
    }

    /// Passe au moyen `m` (`abandonner` jusqu'à lui).
    fn aller_a(d: &mut facade::Defileur, m: Moyen) {
        for _ in 0..3 {
            if d.moyen() == Some(m) {
                break;
            }
            let debut = Instant::now();
            d.abandonner();
            assert!(debut.elapsed() < ATTENTE_TENUE, "abandonner a attendu");
        }
        assert_eq!(d.moyen(), Some(m));
    }

    #[test]
    fn pages_trouve_la_fenetre_qui_defile_sans_sa_barre() {
        let _s = seance();
        let f = Fenetres::ouvrir(Forme::Simple);
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
    fn types_de_controle_comme_ceux_du_systeme() {
        use ::windows::Win32::UI::Accessibility::{
            UIA_ButtonControlTypeId, UIA_DataItemControlTypeId, UIA_DocumentControlTypeId,
            UIA_GroupControlTypeId, UIA_HeaderItemControlTypeId, UIA_ImageControlTypeId,
            UIA_ListItemControlTypeId, UIA_PaneControlTypeId, UIA_SeparatorControlTypeId,
            UIA_TextControlTypeId, UIA_WindowControlTypeId,
        };
        for t in [
            UIA_ButtonControlTypeId,
            UIA_DataItemControlTypeId,
            UIA_HeaderItemControlTypeId,
            UIA_ImageControlTypeId,
            UIA_ListItemControlTypeId,
            UIA_ScrollBarControlTypeId,
            UIA_SeparatorControlTypeId,
            UIA_TextControlTypeId,
        ] {
            assert!(!a_fouiller(t.0), "{t:?}");
        }
        for t in [
            UIA_DocumentControlTypeId,
            UIA_GroupControlTypeId,
            UIA_PaneControlTypeId,
            UIA_WindowControlTypeId,
        ] {
            assert!(a_fouiller(t.0), "{t:?}");
        }
    }

    /// Une demande de fouille de `h`, et sa réception.
    fn demande_page(h: isize) -> (DemandePage, Receiver<Nouvelle>) {
        let (reponse, rx) = mpsc::channel();
        let d = DemandePage {
            i: 3,
            h,
            echeance: Instant::now() + Duration::from_secs(2),
            reponse,
        };
        (d, rx)
    }

    /// La fin de la fouille (la ligne `Debut` passée).
    fn fin_recue(rx: &Receiver<Nouvelle>) -> Option<(usize, Fouille)> {
        while let Ok(n) = rx.recv_timeout(Duration::from_secs(3)) {
            if let Nouvelle::Fin { i, fouille, .. } = n {
                return Some((i, fouille));
            }
        }
        None
    }

    #[test]
    fn un_client_rate_est_recree_a_la_demande_suivante() {
        // Passage 98 : un fil dont le client avait raté à sa création répondait `E_FAIL` en
        // 0 ms à toutes les fenêtres, 17 s plus tard. Le client se recrée à la demande.
        let _s = seance();
        let f = Fenetres::ouvrir(Forme::Simple);
        CLIENTS_RATES.with(|c| c.set(CREATIONS_CLIENT));
        let mut poste = Poste::ouvrir(None);
        assert!(poste.client.is_err(), "le client devait rater");
        let (d, rx) = demande_page(f.nue);
        assert!(matches!(servir(&mut poste, d), Suite::Servie));
        let (i, fouille) = fin_recue(&rx).expect("fin de la fouille");
        assert_eq!(i, 3);
        assert_eq!(fouille.erreur, None);
        assert!(fouille.elements >= 1, "{fouille:?}");
        assert!(poste.client.is_ok());
        // Un client sain n'est remplacé que par un client créé : raté, l'ancien reste.
        CLIENTS_RATES.with(|c| c.set(CREATIONS_CLIENT));
        assert!(poste.remplacer().is_none());
        assert!(poste.client.is_ok());
        CLIENTS_RATES.with(|c| c.set(0));
    }

    #[test]
    fn un_fil_sans_client_ne_repond_pas_lui_meme() {
        let _s = seance();
        let f = Fenetres::ouvrir(Forme::Simple);
        CLIENTS_RATES.with(|c| c.set(10 * CREATIONS_CLIENT));
        let mut poste = Poste::ouvrir(None);
        let (d, rx) = demande_page(f.nue);
        match servir(&mut poste, d) {
            Suite::SansClient(d, e) => {
                assert!(d.i == 3 && d.h == f.nue);
                assert_eq!(e, "client essai 0x80004005");
            }
            Suite::Servie => panic!("sans client, la demande revient sans réponse"),
        }
        assert!(rx.try_recv().is_err(), "rien ne devait être répondu");
        CLIENTS_RATES.with(|c| c.set(0));
    }

    #[test]
    fn le_bassin_rend_la_demande_d_un_fil_sans_client_a_un_fil_qui_en_a_un() {
        let _s = seance();
        let f = Fenetres::ouvrir(Forme::Simple);
        let b = Bassin::nouveau(1);
        *b.maudit.lock().unwrap() = Some(None);
        garnir(&b, &mut b.etat());
        assert!(
            attendre(Duration::from_secs(3), || b.etat().prepares == 1),
            "le premier fil ne s'est pas préparé"
        );
        {
            let e = b.etat();
            assert_eq!((e.vivants, e.avec_client), (1, 0));
            assert_eq!(e.rates, CREATIONS_CLIENT);
            assert!(matches!(
                e.automation(Instant::now()),
                EtatAutomation::Impossible(ref c) if c == "client essai 0x80004005"
            ));
        }
        // Aucun fil n'a de client : l'erreur est répondue tout de suite, le fil reste.
        let (d, rx) = demande_page(f.nue);
        let debut = Instant::now();
        b.envoi.send(d).unwrap();
        let (_, fouille) = fin_recue(&rx).expect("réponse");
        assert_eq!(fouille.erreur.as_deref(), Some("client essai 0x80004005"));
        assert!(
            debut.elapsed() < Duration::from_millis(500),
            "{:?}",
            debut.elapsed()
        );
        assert_eq!(b.etat().vivants, 1);
        // Un second fil, qui a un client.
        b.etat().fils_max = 2;
        garnir(&b, &mut b.etat());
        assert!(
            attendre(Duration::from_secs(3), || b.etat().avec_client == 1),
            "le second fil n'a pas de client"
        );
        assert!(matches!(
            b.etat().automation(Instant::now()),
            EtatAutomation::Prete { .. }
        ));
        // Des demandes jusqu'à ce que le fil sans client en prenne une : il la rend à la file
        // et s'en va ; chacune est fouillée par le fil qui a un client, aucune erreur.
        let mut servies = 0;
        for _ in 0..20 {
            let (d, rx) = demande_page(f.nue);
            b.envoi.send(d).unwrap();
            let (_, fouille) = fin_recue(&rx).expect("réponse");
            assert_eq!(fouille.erreur, None, "{fouille:?}");
            servies += 1;
            if b.etat().vivants == 1 {
                break;
            }
        }
        let e = b.etat();
        assert_eq!(
            (e.vivants, e.avec_client),
            (1, 1),
            "après {servies} demandes"
        );
    }

    #[test]
    fn pages_d_une_fenetre_sans_barre_dans_le_budget() {
        let _s = seance();
        let f = Fenetres::ouvrir(Forme::Simple);
        // Rien par les fenêtres : UI Automation est interrogé, et attendu au plus le budget.
        let debut = Instant::now();
        let liste = facade::pages(&[f.info(f.nue)], 8);
        let duree = debut.elapsed();
        assert!(
            duree < BUDGET_PAGES + Duration::from_millis(100),
            "pages en {duree:?}"
        );
        assert!(liste.iter().all(|p| p.fenetre != f.page), "{liste:?}");
    }

    #[test]
    fn pages_et_message_d_une_fenetre_fille() {
        let _s = seance();
        let f = Fenetres::ouvrir(Forme::Fille);
        let client = f.zone();
        assert_ne!(f.page, f.racine);
        let liste = facade::pages(&[f.info(f.racine), f.info(f.sentinelle)], 8);
        let page = liste
            .iter()
            .find(|p| p.fenetre == f.page)
            .copied()
            .unwrap_or_else(|| panic!("page fille trouvee : {liste:?}"));
        // Coupée par sa sœur (le couvercle) en bas.
        assert_eq!(page.rect.x, client.x);
        assert_eq!(page.rect.y, client.y);
        assert_eq!(page.rect.largeur, client.largeur);
        assert_eq!(
            page.rect.bas(),
            client.y as i64 + (HAUTEUR - BANDE - COUVERCLE) as i64,
            "{page:?} dans {client:?}"
        );
        assert!(!liste.iter().any(|p| p.fenetre == f.racine), "{liste:?}");
        // Zone tracée : la page fille sous le centre, depuis la seule racine.
        let point = centre(page.rect);
        assert_eq!(
            cible_sous(None, Some(hwnd(f.racine)), point),
            Some(hwnd(f.page))
        );
        // Page cliquée : le message va à la fille, ni au conteneur ni à la sentinelle.
        let mut d = facade::preparer(page.rect, Some(page.fenetre)).unwrap();
        assert_eq!(d.moyen(), Some(Moyen::Message));
        assert_eq!(pas(&mut d, 100), Envoi::Envoye);
        assert!(
            attendre(Duration::from_secs(2), || position_page() > 0),
            "la page fille n'a pas bougé"
        );
        // Zone de moins de 400 px : le premier pas est un demi-cran.
        assert_eq!(position_page(), PAR_CRAN / 2);
        sentinelles_immobiles();
    }

    #[test]
    fn message_poste_fait_defiler_la_page_pas_la_sentinelle() {
        let _s = seance();
        let f = Fenetres::ouvrir(Forme::Simple);
        let zone = f.zone();
        let mut d = facade::preparer(zone, Some(f.page)).unwrap();
        assert_eq!(d.moyen(), Some(Moyen::Message));
        assert_eq!(d.avance_par_cran(), None);
        assert_eq!(pas(&mut d, 100), Envoi::Envoye);
        assert!(
            attendre(Duration::from_secs(2), || position_page() > 0),
            "la page n'a pas bougé"
        );
        // Premier pas, zone de moins de 400 px : un demi-cran (delta 60, 20 px).
        assert_eq!(position_page(), PAR_CRAN / 2);
        assert_eq!(d.envoyes(), 1);
        // Les images ont vu 20 px pour un demi-cran : 40 px par cran, et le pas suivant de
        // 100 px fait deux crans.
        d.constater((PAR_CRAN / 2) as u32);
        assert_eq!(d.avance_par_cran(), Some(PAR_CRAN as u32));
        assert_eq!(pas(&mut d, 100), Envoi::Envoye);
        assert!(attendre(Duration::from_secs(2), || position_page()
            == 2 * PAR_CRAN + PAR_CRAN / 2));
        // Remonter.
        assert_eq!(pas(&mut d, -40), Envoi::Envoye);
        assert!(attendre(Duration::from_secs(2), || position_page()
            == PAR_CRAN + PAR_CRAN / 2));
        // Un pas plus petit qu'un cran : un cran partiel (delta 60, 20 px).
        assert_eq!(pas(&mut d, PAR_CRAN / 2), Envoi::Envoye);
        assert!(
            attendre(Duration::from_secs(2), || position_page() == 2 * PAR_CRAN),
            "cran partiel : {}",
            position_page()
        );
        d.constater((PAR_CRAN / 2) as u32);
        assert_eq!(d.avance_par_cran(), Some(PAR_CRAN as u32));
        assert_eq!(pas(&mut d, 0), Envoi::Saute);
        // Abandonner passe toujours au suivant, même après un effet.
        assert_ne!(d.abandonner(), Some(Moyen::Message));
        sentinelles_immobiles();
    }

    #[test]
    fn rien_ne_part_quand_une_fenetre_couvre_la_zone() {
        let _s = seance();
        let f = Fenetres::ouvrir(Forme::Simple);
        let zone = f.zone();
        let fenetre = f.info(f.page).rect;
        let ailleurs = f.info(f.sentinelle).rect;
        // La sentinelle posée sur la page, devant le centre de la zone.
        f.deplacer_sentinelle(fenetre.x, fenetre.y);
        let mut d = facade::preparer(zone, Some(f.page)).unwrap();
        assert_eq!(d.moyen(), Some(Moyen::Message));
        assert_eq!(pas(&mut d, 100), Envoi::Saute, "message sous une fenêtre");
        // La molette, le pointeur sur la sentinelle posée sur la zone.
        let mut m = facade::preparer(zone, Some(f.page)).unwrap();
        aller_a(&mut m, Moyen::Molette);
        poser_pointeur(centre(zone));
        assert_eq!(pas(&mut m, 100), Envoi::Saute, "molette sous une fenêtre");
        thread::sleep(Duration::from_millis(100));
        assert_eq!(position_page(), 0);
        assert_eq!(POSITION_SENTINELLE.load(Ordering::SeqCst), 0);
        // La sentinelle remise à sa place : le message part.
        f.deplacer_sentinelle(ailleurs.x, ailleurs.y);
        assert_eq!(pas(&mut d, 100), Envoi::Envoye);
        assert!(attendre(Duration::from_secs(2), || position_page() > 0));
        sentinelles_immobiles();
    }

    #[test]
    fn automation_fait_defiler_et_lit_la_position() {
        let _s = seance();
        let f = Fenetres::ouvrir(Forme::Simple);
        let mut d = facade::preparer(f.zone(), Some(f.page)).unwrap();
        // « Rien n'a bougé » : le moyen suivant, même si sa recherche court encore.
        aller_a(&mut d, Moyen::Automation);
        // Le pointeur hors de la zone : si UI Automation est laissé pendant un pas, la
        // molette qui suit ne part pas.
        poser_pointeur(centre(f.info(f.nue).rect));
        let mut parti = false;
        let fin = Instant::now() + Duration::from_secs(3);
        while Instant::now() < fin && d.moyen() == Some(Moyen::Automation) {
            if pas(&mut d, 50) == Envoi::Envoye {
                assert_eq!(d.moyen(), Some(Moyen::Automation));
                parti = true;
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        if parti {
            assert!(
                attendre(Duration::from_secs(2), || position_page() > 0),
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
            assert!(
                !obligatoire(),
                "UI Automation n'a rien fait défiler (moyen {:?})",
                d.moyen()
            );
            eprintln!("fenetre d'essai sans ScrollPattern : moyen {:?}", d.moyen());
            assert_eq!(d.moyen(), Some(Moyen::Molette));
            assert_eq!(d.position(), None);
        }
        sentinelles_immobiles();
    }

    #[test]
    fn molette_envoyee_seulement_le_pointeur_sur_la_zone() {
        let _s = seance();
        let f = Fenetres::ouvrir(Forme::Simple);
        let zone = f.zone();
        let mut d = facade::preparer(zone, Some(f.page)).unwrap();
        aller_a(&mut d, Moyen::Molette);
        // Le pointeur sur la sentinelle : rien ne part.
        poser_pointeur(centre(zone_cliente(hwnd(f.sentinelle)).unwrap()));
        assert_eq!(pas(&mut d, 100), Envoi::Saute);
        thread::sleep(Duration::from_millis(100));
        assert_eq!(POSITION_SENTINELLE.load(Ordering::SeqCst), 0);
        assert_eq!(position_page(), 0);
        // Le pointeur sur la zone : part si la garde le permet (« faire défiler les
        // fenêtres inactives » mis ; la fenêtre d'essai ne prend jamais le premier plan).
        let survol = survol_actif(routage());
        let mut parti = false;
        for _ in 0..30 {
            poser_pointeur(centre(zone));
            if pas(&mut d, 100) == Envoi::Envoye {
                parti = true;
                break;
            }
            // Une autre fenêtre est peut-être passée dessus : on retente.
            thread::sleep(Duration::from_millis(100));
        }
        if survol {
            assert!(parti, "molette jamais envoyee sur la zone");
            assert!(
                attendre(Duration::from_secs(2), || position_page() > 0),
                "la page n'a pas bougé par la molette"
            );
        } else {
            assert!(
                !obligatoire(),
                "faire defiler les fenetres inactives coupe sur la machine d'essai"
            );
            eprintln!("faire defiler les fenetres inactives coupe : molette retenue");
            assert!(!parti);
        }
        sentinelles_immobiles();
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
        let f = Fenetres::ouvrir(Forme::Simple);
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
        let f = Fenetres::ouvrir(Forme::Simple);
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
        // Des crans encore dans le canal au moment du `Drop` ne sont jamais rendus.
        envoyer_molette(-CRAN, 0);
        envoyer_molette(-CRAN, 0);
        drop(ecoute);
        let apres_drop = recus.load(Ordering::SeqCst);
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
        assert_eq!(
            recus.load(Ordering::SeqCst),
            apres_drop,
            "rappel appelé après le Drop"
        );
        assert!(apres_drop <= 3);
    }
}
