// Fără fereastră de consolă în versiunea de producție pe Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    scadentar_lib::run()
}
