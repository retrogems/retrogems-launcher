// Pas de fenêtre de console sous Windows en version finale.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    retrogems_launcher_lib::run()
}
