// Pas de console en version de release sous Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    breezyshot::lancer();
}
