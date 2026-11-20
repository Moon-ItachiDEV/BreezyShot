//! Hors Windows : aucune action sur les icônes du bureau.

pub(super) fn masquer() -> Option<isize> {
    None
}

pub(super) fn restaurer(_fenetre: isize) {}
