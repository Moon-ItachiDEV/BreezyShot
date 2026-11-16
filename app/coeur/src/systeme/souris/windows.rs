//! La souris sous Windows : `GetCursorPos` et `GetAsyncKeyState`, rien d'autre.

use ::windows::Win32::Foundation::POINT;
use ::windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON,
};
use ::windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

use super::Lecture;

pub fn lire() -> Option<Lecture> {
    let mut point = POINT::default();
    // SAFETY : `point` reçoit la position, en pixels réels du bureau virtuel.
    unsafe { GetCursorPos(&mut point) }.ok()?;
    let (mut enfonce, mut appuye) = (false, false);
    for touche in [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON] {
        // SAFETY : aucun tampon ; bit de poids fort : enfoncé maintenant ; bit de poids
        // faible : enfoncé depuis le dernier appel.
        let etat = unsafe { GetAsyncKeyState(i32::from(touche.0)) };
        enfonce |= etat < 0;
        appuye |= etat & 1 != 0;
    }
    Some(Lecture {
        position: (point.x, point.y),
        enfonce,
        appuye,
    })
}

#[cfg(test)]
mod tests {
    /// Sur une machine sans ouverture de session interactive (GitHub), `lire` rend une
    /// position ou rien, sans panique.
    #[test]
    fn lire_sans_panique() {
        let a = super::super::lire();
        let b = super::super::lire();
        eprintln!("souris: {a:?} puis {b:?}");
    }
}
