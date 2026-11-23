//! Ikona v oznamovací oblasti (systray): levý klik nebo „Otevřít“ otevře okno, „Zavřít“ ukončí měření.

use crate::i18n;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

const ID_OPEN: &str = "open";
const ID_QUIT: &str = "quit";

/// Vytvoří ikonu. Zpětná volání se spouštějí ve vlákně smyčky událostí, takže smí sahat na UI.
/// Vrácený objekt je potřeba držet naživu, jinak ikona zmizí.
pub fn create(
    on_open: impl Fn() + Send + Sync + 'static,
    on_quit: impl Fn() + Send + Sync + 'static,
) -> Result<TrayIcon, String> {
    let png = image::load_from_memory(include_bytes!("../assets/icon/timetrek-32.png"))
        .map_err(|e| e.to_string())?
        .into_rgba8();
    let icon = Icon::from_rgba(png.to_vec(), png.width(), png.height()).map_err(|e| e.to_string())?;

    let menu = Menu::new();
    menu.append(&MenuItem::with_id(ID_OPEN, i18n::TRAY_OPEN, true, None)).map_err(|e| e.to_string())?;
    menu.append(&MenuItem::with_id(ID_QUIT, i18n::TRAY_QUIT, true, None)).map_err(|e| e.to_string())?;

    let on_open = std::sync::Arc::new(on_open);
    let open2 = on_open.clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| match e.id.as_ref() {
        ID_OPEN => open2(),
        ID_QUIT => on_quit(),
        _ => {}
    }));
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
            on_open();
        }
    }));

    TrayIconBuilder::new()
        .with_tooltip(i18n::TRAY_TOOLTIP)
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .map_err(|e| e.to_string())
}
