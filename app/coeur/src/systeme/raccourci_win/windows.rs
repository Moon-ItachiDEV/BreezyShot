//! Crochet clavier bas niveau (`WH_KEYBOARD_LL`) pour `Win + Maj + S` sous Windows.
//!
//! Même séparation de fils que `systeme/defiler` (`WH_MOUSE_LL`) :
//! - le fil `raccourci-win` (priorité « temps critique ») ne fait que tourner sa boucle de
//!   messages ; le rappel du crochet lit les touches et poste un jeton dans un canal non
//!   bloquant pour ne jamais approcher `LowLevelHooksTimeout` ;
//! - le fil `raccourci-win-rappel` masque l'ouverture du menu Démarrer au relâchement de la
//!   touche `Win` (touche virtuelle non attribuée `0xE8` via `SendInput`) puis appelle le
//!   rappel fourni par l'Application.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex, TryLockError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use ::windows::core::PCWSTR;
use ::windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use ::windows::Win32::System::LibraryLoader::GetModuleHandleW;
use ::windows::Win32::System::Threading::{
    GetCurrentThread, GetCurrentThreadId, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL,
};
use ::windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use ::windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PeekMessageW, PostThreadMessageW,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, HC_ACTION, KBDLLHOOKSTRUCT, MSG,
    PM_NOREMOVE, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
    WM_USER,
};

use super::{Decision, FiltreWinMajS, ModificateursWin, Rappel, VK_S};

/// Délai maximal attendu pour la pose du crochet.
const POSE_MAX: Duration = Duration::from_secs(2);
/// Délai maximal attendu à l'arrêt pour que le fil retire son crochet.
const ARRET_MAX: Duration = Duration::from_millis(400);

/// Touche virtuelle non attribuée (`0xE8`) envoyée pour éviter que le relâchement de `Win`
/// n'ouvre le menu Démarrer quand `S` a été avalée.
const VK_MASQUE: VIRTUAL_KEY = VIRTUAL_KEY(0xE8);
/// Marque portée par la touche de masquage pour la distinguer sans ambiguïté.
const MARQUE_MASQUE: usize = 0x4252_5A57;

thread_local! {
    /// Canal du crochet, uniquement dans le fil `raccourci-win`.
    static ENVOI: RefCell<Option<Sender<()>>> = const { RefCell::new(None) };
    /// État de répétition de `S`, uniquement dans le fil `raccourci-win`.
    static FILTRE: RefCell<FiltreWinMajS> = const { RefCell::new(FiltreWinMajS::nouveau()) };
}

fn code(e: &::windows::core::Error) -> String {
    format!("0x{:08x}", e.code().0 as u32)
}

fn touche_enfoncee(vk: VIRTUAL_KEY) -> bool {
    // SAFETY : lecture sans effet de bord de l'état asynchrone de la touche.
    (unsafe { GetAsyncKeyState(i32::from(vk.0)) } as u16 & 0x8000) != 0
}

fn lire_modificateurs() -> ModificateursWin {
    ModificateursWin {
        win: touche_enfoncee(VK_LWIN) || touche_enfoncee(VK_RWIN),
        maj: touche_enfoncee(VK_SHIFT),
        ctrl: touche_enfoncee(VK_CONTROL),
        alt: touche_enfoncee(VK_MENU),
    }
}

/// Empêche le menu Démarrer de s'ouvrir quand l'utilisateur relâche `Win` après que `S` a été
/// avalée (notamment si `Maj` a été pressée avant `Win`).
fn masquer_menu_demarrer() {
    let entree = |drapeaux: KEYBD_EVENT_FLAGS| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_MASQUE,
                wScan: 0,
                dwFlags: drapeaux,
                time: 0,
                dwExtraInfo: MARQUE_MASQUE,
            },
        },
    };
    let entrees = [entree(KEYBD_EVENT_FLAGS(0)), entree(KEYEVENTF_KEYUP)];
    // SAFETY : tableau contigu de deux `INPUT` valides ; taille exacte de la structure.
    let _ = unsafe { SendInput(&entrees, std::mem::size_of::<INPUT>() as i32) };
}

unsafe extern "system" fn rappel_crochet(code_ev: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code_ev == HC_ACTION as i32 && lparam.0 != 0 {
        // SAFETY : pour `WH_KEYBOARD_LL`, `lParam` pointe sur un `KBDLLHOOKSTRUCT` valide
        // pendant toute la durée de l'appel.
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if info.dwExtraInfo != MARQUE_MASQUE && info.vkCode == VK_S {
            let appui = match wparam.0 as u32 {
                WM_KEYDOWN | WM_SYSKEYDOWN => Some(true),
                WM_KEYUP | WM_SYSKEYUP => Some(false),
                _ => None,
            };
            if let Some(appui) = appui {
                let mods = lire_modificateurs();
                let decision = FILTRE
                    .try_with(|f| {
                        f.try_borrow_mut()
                            .map(|mut f| f.traiter(appui, info.vkCode, mods))
                            .unwrap_or(Decision::Passer)
                    })
                    .unwrap_or(Decision::Passer);
                match decision {
                    Decision::Passer => {}
                    Decision::Avaler => return LRESULT(1),
                    Decision::Declencher => {
                        let _ = ENVOI.try_with(|e| {
                            if let Ok(e) = e.try_borrow() {
                                if let Some(tx) = e.as_ref() {
                                    let _ = tx.send(());
                                }
                            }
                        });
                        return LRESULT(1);
                    }
                }
            }
        }
    }
    // SAFETY : passe l'événement au crochet suivant sans le modifier.
    unsafe { CallNextHookEx(None, code_ev, wparam, lparam) }
}

fn boucle_crochet(
    tx: Sender<()>,
    pret: SyncSender<Result<u32, String>>,
    abandonne: Arc<AtomicBool>,
    fini: SyncSender<()>,
) {
    // Le système attend ce fil à chaque frappe : priorité élevée pour ne jamais freiner le
    // clavier.
    // SAFETY : pseudo-poignée du fil courant.
    if let Err(e) = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL) }
    {
        log::info!("raccourci win: priorite du fil non changee ({})", code(&e));
    }
    let mut msg = MSG::default();
    // SAFETY : crée la file de messages de ce fil avant que `Drop` puisse lui poster `WM_QUIT`.
    unsafe {
        let _ = PeekMessageW(&mut msg, None, WM_USER, WM_USER, PM_NOREMOVE);
    }
    // SAFETY : sans paramètre.
    let fil = unsafe { GetCurrentThreadId() };
    ENVOI.with(|e| *e.borrow_mut() = Some(tx));
    FILTRE.with(|f| *f.borrow_mut() = FiltreWinMajS::nouveau());
    // SAFETY : module du programme courant.
    let module = unsafe { GetModuleHandleW(PCWSTR::null()) }
        .ok()
        .map(HINSTANCE::from);
    // SAFETY : procédure du module courant, vivante autant que le processus.
    let crochet =
        match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(rappel_crochet), module, 0) } {
            Ok(c) => c,
            Err(e) => {
                ENVOI.with(|envoi| envoi.borrow_mut().take());
                let _ = pret.send(Err(format!("SetWindowsHookExW {}", code(&e))));
                return;
            }
        };
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
    // SAFETY : crochet posé ci-dessus, retiré exactement une fois.
    if let Err(e) = unsafe { UnhookWindowsHookEx(crochet) } {
        log::warn!("raccourci win: crochet non retire ({})", code(&e));
    }
    ENVOI.with(|e| e.borrow_mut().take());
    FILTRE.with(|f| *f.borrow_mut() = FiltreWinMajS::nouveau());
    let _ = fini.send(());
}

#[derive(Debug)]
pub struct Ecoute {
    fil: u32,
    ouvert: Arc<AtomicBool>,
    en_appel: Arc<Mutex<()>>,
    fini: Receiver<()>,
    poignee: Option<JoinHandle<()>>,
}

pub fn disponible() -> bool {
    true
}

pub fn ecouter(rappel: Rappel) -> Result<Ecoute, String> {
    let (tx, rx) = mpsc::channel::<()>();
    let (pret_tx, pret_rx) = mpsc::sync_channel(1);
    let (fini_tx, fini_rx) = mpsc::sync_channel(1);
    let abandonne = Arc::new(AtomicBool::new(false));
    let abandonne_fil = Arc::clone(&abandonne);
    let ouvert = Arc::new(AtomicBool::new(true));
    let ouvert_fil = Arc::clone(&ouvert);
    let en_appel = Arc::new(Mutex::new(()));
    let en_appel_fil = Arc::clone(&en_appel);
    thread::Builder::new()
        .name("raccourci-win-rappel".into())
        .spawn(move || {
            for () in rx {
                if !ouvert_fil.load(Ordering::SeqCst) {
                    break;
                }
                let _appel = en_appel_fil.lock().unwrap_or_else(|e| e.into_inner());
                if !ouvert_fil.load(Ordering::SeqCst) {
                    break;
                }
                masquer_menu_demarrer();
                rappel();
            }
        })
        .map_err(|e| format!("fil du rappel: {e}"))?;
    let poignee = thread::Builder::new()
        .name("raccourci-win".into())
        .spawn(move || boucle_crochet(tx, pret_tx, abandonne_fil, fini_tx))
        .map_err(|e| format!("fil du crochet: {e}"))?;
    match pret_rx.recv_timeout(POSE_MAX) {
        Ok(Ok(fil)) => Ok(Ecoute {
            fil,
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
            abandonne.store(true, Ordering::SeqCst);
            ouvert.store(false, Ordering::SeqCst);
            if let Ok(Ok(fil)) = pret_rx.try_recv() {
                // SAFETY : message sans données à la file du fil du crochet.
                let _ = unsafe { PostThreadMessageW(fil, WM_QUIT, WPARAM(0), LPARAM(0)) };
            }
            Err("crochet pas pose a temps".into())
        }
    }
}

impl Drop for Ecoute {
    fn drop(&mut self) {
        let fin = Instant::now() + ARRET_MAX;
        self.ouvert.store(false, Ordering::SeqCst);
        // SAFETY : message sans données à la file du fil du crochet.
        let _ = unsafe { PostThreadMessageW(self.fil, WM_QUIT, WPARAM(0), LPARAM(0)) };
        loop {
            match self.en_appel.try_lock() {
                Ok(_) | Err(TryLockError::Poisoned(_)) => break,
                Err(TryLockError::WouldBlock) if Instant::now() >= fin => {
                    log::warn!("raccourci win: rappel encore en cours a l'arret");
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
            log::warn!("raccourci win: fil du crochet sans reponse a l'arret");
        }
    }
}
