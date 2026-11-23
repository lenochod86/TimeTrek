// Bez zbytečného okna konzole u vydané verze na Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    timetrek_lib::run()
}
