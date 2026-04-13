use muda::accelerator::{Accelerator, Code, CMD_OR_CTRL};
use muda::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};

pub struct AppMenu {
    pub menu: Menu,
    pub new_tab_id: MenuId,
    pub close_tab_id: MenuId,
    pub toggle_debug_id: MenuId,
    pub toggle_settings_id: MenuId,
    pub copy_id: MenuId,
    pub paste_id: MenuId,
    pub cut_id: MenuId,
    pub select_all_id: MenuId,

    #[cfg(target_os = "macos")]
    pub window_submenu: Submenu,
}

pub fn build_menu() -> AppMenu {
    let app_submenu = Submenu::new("Awebo", true);
    app_submenu
        .append_items(&[
            &PredefinedMenuItem::about(None, None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::services(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::quit(None),
        ])
        .expect("Failed to build app submenu");

    let new_tab = MenuItem::new(
        "New Tab",
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyT)),
    );
    let close_tab = MenuItem::new(
        "Close Tab",
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyW)),
    );

    let file_submenu = Submenu::new("File", true);
    file_submenu
        .append_items(&[&new_tab, &PredefinedMenuItem::separator(), &close_tab])
        .expect("Failed to build file submenu");

    let cut_item = MenuItem::new(
        "Cut",
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyX)),
    );
    let copy_item = MenuItem::new(
        "Copy",
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyC)),
    );
    let paste_item = MenuItem::new(
        "Paste",
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyV)),
    );
    let select_all_item = MenuItem::new(
        "Select All",
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyA)),
    );

    let edit_submenu = Submenu::new("Edit", true);
    edit_submenu
        .append_items(&[
            &PredefinedMenuItem::undo(None),
            &PredefinedMenuItem::redo(None),
            &PredefinedMenuItem::separator(),
            &cut_item,
            &copy_item,
            &paste_item,
            &select_all_item,
        ])
        .expect("Failed to build edit submenu");

    let toggle_debug = MenuItem::new("Toggle Debug Panel", true, None);
    let toggle_settings = MenuItem::new(
        "Settings…",
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::Comma)),
    );

    let view_submenu = Submenu::new("View", true);
    view_submenu
        .append_items(&[
            &toggle_debug,
            &PredefinedMenuItem::separator(),
            &toggle_settings,
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::fullscreen(None),
        ])
        .expect("Failed to build view submenu");

    let window_submenu = Submenu::new("Window", true);
    window_submenu
        .append_items(&[
            &PredefinedMenuItem::minimize(None),
            &PredefinedMenuItem::maximize(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::bring_all_to_front(None),
        ])
        .expect("Failed to build window submenu");

    let help_submenu = Submenu::new("Help", true);

    let menu = Menu::new();
    menu.append_items(&[
        &app_submenu,
        &file_submenu,
        &edit_submenu,
        &view_submenu,
        &window_submenu,
        &help_submenu,
    ])
    .expect("Failed to build menu bar");

    AppMenu {
        menu,
        new_tab_id: new_tab.into_id(),
        close_tab_id: close_tab.into_id(),
        toggle_debug_id: toggle_debug.into_id(),
        toggle_settings_id: toggle_settings.into_id(),
        copy_id: copy_item.into_id(),
        paste_id: paste_item.into_id(),
        cut_id: cut_item.into_id(),
        select_all_id: select_all_item.into_id(),
        #[cfg(target_os = "macos")]
        window_submenu,
    }
}

pub fn setup_event_handler<T: 'static + Send>(
    proxy: winit::event_loop::EventLoopProxy<T>,
    convert: fn(MenuEvent) -> T,
) {
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let _ = proxy.send_event(convert(event));
    }));
}
