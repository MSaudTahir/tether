//! Window size popover, pairing, settings and the adb picker.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};
use tether_core::*;

use crate::model::Model;

/// Shows the app window size; opens a popover to change it.
pub fn window_size_button(model: &Rc<Model>) -> gtk::MenuButton {
    let label = adw::ButtonContent::new();
    let button = gtk::MenuButton::builder().child(&label).tooltip_text("Size of the window apps open in").build();
    let presets = window_presets();

    let sizes = gtk::StringList::new(&[]);
    for preset in &presets {
        sizes.append(&if preset.width == 0 { preset.name.clone() } else { format!("{} ({} × {})", preset.name, preset.width, preset.height) });
    }
    sizes.append("Custom");
    let size = adw::ComboRow::builder().title("Size").model(&sizes).build();
    let portrait = adw::SwitchRow::builder().title("Portrait").build();
    let text_size = adw::ComboRow::builder().title("Text size").model(&gtk::StringList::new(&["Smaller", "Default", "Larger"])).build();
    let resizable = adw::SwitchRow::builder()
        .title("Resizable")
        .subtitle("The app relayouts when you resize its window (less sharp on HiDPI screens)")
        .build();
    let custom = adw::EntryRow::builder().title("Custom size (WIDTHxHEIGHT/DPI)").show_apply_button(true).build();
    let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).css_classes(["boxed-list"]).width_request(380).build();
    for row in [size.upcast_ref::<gtk::Widget>(), portrait.upcast_ref(), text_size.upcast_ref(), resizable.upcast_ref(), custom.upcast_ref()] {
        list.append(row);
    }
    let footer = gtk::Label::builder()
        .label("Sizes are in points; big ones shrink to fit the screen. Portrait suits most phone apps.")
        .wrap(true)
        .max_width_chars(44)
        .xalign(0.0)
        .css_classes(["caption", "dim-label"])
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
    content.set_margin_top(6);
    content.set_margin_bottom(6);
    content.set_margin_start(6);
    content.set_margin_end(6);
    content.append(&list);
    content.append(&footer);
    button.set_popover(Some(&gtk::Popover::builder().child(&content).build()));

    // Rendering sets the rows, which fires their handlers: ignore those.
    let rendering = Rc::new(Cell::new(false));
    let render = {
        let (model, presets, rendering) = (model.clone(), presets.clone(), rendering.clone());
        let (size, portrait, text_size, resizable, custom) = (size.clone(), portrait.clone(), text_size.clone(), resizable.clone(), custom.clone());
        move || {
            rendering.set(true);
            let current = model.window_size();
            let choice = window_choice(current);
            let same_as_phone = choice.preset.as_ref().is_some_and(|p| p.width == 0);
            label.set_label(&describe_window_size(current));
            label.set_icon_name(if choice.portrait { "phone-symbolic" } else { "computer-symbolic" });
            let index = choice.preset.as_ref().and_then(|p| presets.iter().position(|q| q.name == p.name)).unwrap_or(presets.len());
            size.set_selected(index as u32);
            portrait.set_active(choice.portrait);
            portrait.set_sensitive(!same_as_phone && current.width != 0);
            text_size.set_selected(choice.text_size as u32);
            text_size.set_sensitive(choice.preset.is_some() && !same_as_phone);
            resizable.set_active(model.config.borrow().resizable_windows);
            if !custom.has_focus() {
                custom.set_text(&model.config.borrow().window_size);
            }
            rendering.set(false);
        }
    };
    render();
    model.connect_changed(render);

    {
        let (model, rendering) = (model.clone(), rendering.clone());
        size.connect_selected_notify(move |row| {
            let Some(preset) = presets.get(row.selected() as usize).cloned() else { return };
            if rendering.get() {
                return;
            }
            let choice = window_choice(model.window_size());
            model.set_window_size(preset_window_size(preset, choice.portrait, choice.text_size));
        });
    }
    {
        let (model, rendering) = (model.clone(), rendering.clone());
        portrait.connect_active_notify(move |row| {
            if rendering.get() {
                return;
            }
            let current = model.window_size();
            let choice = window_choice(current);
            match choice.preset {
                Some(preset) => model.set_window_size(preset_window_size(preset, row.is_active(), choice.text_size)),
                None if row.is_active() != (current.height > current.width) => {
                    model.set_window_size(WindowSize { width: current.height, height: current.width, dpi: current.dpi })
                }
                None => {}
            }
        });
    }
    {
        let (model, rendering) = (model.clone(), rendering.clone());
        text_size.connect_selected_notify(move |row| {
            if rendering.get() {
                return;
            }
            let choice = window_choice(model.window_size());
            let size = [TextSize::Smaller, TextSize::Default, TextSize::Larger][row.selected().min(2) as usize];
            if let Some(preset) = choice.preset {
                model.set_window_size(preset_window_size(preset, choice.portrait, size));
            }
        });
    }
    {
        let (model, rendering) = (model.clone(), rendering.clone());
        resizable.connect_active_notify(move |row| {
            if !rendering.get() {
                model.update_config(|c| c.resizable_windows = row.is_active());
            }
        });
    }
    {
        let model = model.clone();
        custom.connect_apply(move |row| match parse_window_size(row.text().to_string()) {
            Some(size) => {
                row.remove_css_class("error");
                model.set_window_size(size);
            }
            None => row.add_css_class("error"),
        });
    }
    button
}

pub fn show_pair(model: &Rc<Model>, window: &impl IsA<gtk::Widget>) {
    let pair_address = adw::EntryRow::builder().title("Pairing IP:port (e.g. 192.168.1.20:37099)").build();
    let code = adw::EntryRow::builder().title("Pairing code").build();
    let connect = adw::EntryRow::builder().title("IP:port (e.g. 192.168.1.20:41235)").build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let pair_group = adw::PreferencesGroup::builder()
        .title("Pair")
        .description("On the phone: Developer options → Wireless debugging → Pair device with pairing code. The phone and this computer must be on the same network.")
        .build();
    pair_group.add(&pair_address);
    pair_group.add(&code);
    let connect_group = adw::PreferencesGroup::builder()
        .title("Connect")
        .description("The IP address & port shown on the Wireless debugging screen. Already paired? Just fill this in.")
        .build();
    connect_group.add(&connect);
    content.append(&pair_group);
    content.append(&connect_group);

    let dialog = adw::AlertDialog::builder().heading("Pair over Wi-Fi").extra_child(&content).build();
    dialog.add_responses(&[("cancel", "Cancel"), ("connect", "Connect")]);
    dialog.set_response_appearance("connect", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("connect"));
    let model = model.clone();
    dialog.connect_response(Some("connect"), move |_, _| {
        let (model, address, code, connect) = (model.clone(), pair_address.text().trim().to_string(), code.text().trim().to_string(), connect.text().trim().to_string());
        glib::spawn_future_local(async move {
            if let Err(e) = model.pair(address, code, connect).await {
                model.alert(&e);
            }
        });
    });
    dialog.present(Some(window));
}

pub fn show_settings(model: &Rc<Model>, window: &impl IsA<gtk::Widget>) {
    let page = adw::PreferencesPage::new();

    let adb = adw::PreferencesGroup::builder().title("adb").build();
    adb.add(&adb_picker(model));
    page.add(&adb);

    let mouse = adw::PreferencesGroup::builder().title("Mouse").build();
    let capture = adw::SwitchRow::builder()
        .title("Capture the mouse")
        .subtitle("Sends the mouse as a real mouse, for games and apps that capture the pointer (Android 8+). Press Left Alt or Super to release it. App windows turn on the phone's “Force desktop mode” developer option so the mouse appears in them.")
        .active(model.config.borrow().capture_mouse)
        .build();
    {
        let model = model.clone();
        capture.connect_active_notify(move |row| model.update_config(|c| c.capture_mouse = row.is_active()));
    }
    mouse.add(&capture);
    page.add(&mouse);

    let scrcpy = adw::PreferencesGroup::builder().title("scrcpy").build();
    let in_use = adw::ActionRow::builder().title("In use").subtitle(model.core.scrcpy_path().unwrap_or("Not found".into())).subtitle_selectable(true).build();
    scrcpy.add(&in_use);
    let choose = button_row("Choose scrcpy…");
    let downloaded = button_row("Use Downloaded scrcpy");
    let render = {
        let (model, in_use, downloaded) = (model.clone(), in_use.clone(), downloaded.clone());
        move || {
            in_use.set_subtitle(&model.core.scrcpy_path().unwrap_or("Not found".into()));
            downloaded.set_visible(!model.config.borrow().scrcpy.is_empty() && model.core.bundled_scrcpy_version().is_some());
        }
    };
    render();
    {
        let (model, render) = (model.clone(), render.clone());
        downloaded.connect_activated(move |_| {
            model.update_config(|c| c.scrcpy.clear());
            render();
        });
    }
    {
        let model = model.clone();
        choose.connect_activated(move |row| {
            let (model, render, window) = (model.clone(), render.clone(), row.root().and_downcast::<gtk::Window>());
            glib::spawn_future_local(async move {
                if let Some(path) = pick_file("Choose scrcpy", window.as_ref()).await {
                    model.update_config(|c| c.scrcpy = path);
                    render();
                }
            });
        });
    }
    scrcpy.add(&downloaded);
    scrcpy.add(&choose);
    page.add(&scrcpy);

    let dialog = adw::PreferencesDialog::new();
    let setup_group = adw::PreferencesGroup::new();
    let setup = button_row("Run Setup Again");
    {
        let (model, dialog) = (model.clone(), dialog.clone());
        setup.connect_activated(move |_| {
            dialog.close();
            model.run_setup_again();
        });
    }
    setup_group.add(&setup);
    page.add(&setup_group);

    dialog.add(&page);
    dialog.present(Some(window));
}

/// Lets the user pick which adb to use; saves immediately.
pub fn adb_picker(model: &Rc<Model>) -> gtk::ListBox {
    let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).css_classes(["boxed-list"]).build();
    let render = {
        let (model, list) = (model.clone(), list.clone());
        move || {
            list.remove_all();
            let current = model.config.borrow().adb.clone();
            let mut choices = vec![("auto".to_string(), "Automatic (recommended)".to_string(), model.core.resolve_adb("auto".into()))];
            for candidate in model.core.adb_candidates() {
                let title = match candidate.source.as_str() {
                    "studio" => "Android Studio",
                    "system" => "System",
                    "bundled" => "Bundled with scrcpy",
                    other => other,
                };
                choices.push((candidate.source.clone(), title.into(), Some(candidate.path)));
            }
            if !choices.iter().any(|(value, _, _)| *value == current) {
                choices.push((current.clone(), "Custom".into(), Some(current.clone())));
            }
            let mut group: Option<gtk::CheckButton> = None;
            for (value, title, path) in choices {
                let check = gtk::CheckButton::builder().active(value == current).valign(gtk::Align::Center).build();
                check.set_group(group.as_ref());
                group.get_or_insert(check.clone());
                let row = adw::ActionRow::builder().title(title).subtitle(path.unwrap_or("Not found".into())).activatable_widget(&check).build();
                row.add_prefix(&check);
                let model = model.clone();
                check.connect_toggled(move |check| {
                    if check.is_active() && model.config.borrow().adb != value {
                        model.update_config(|c| c.adb = value.clone());
                    }
                });
                list.append(&row);
            }
            let other = button_row("Choose Another adb…");
            list.append(&other);
            let model = model.clone();
            let list = list.clone();
            other.connect_activated(move |_| {
                let (model, list) = (model.clone(), list.clone());
                glib::spawn_future_local(async move {
                    if let Some(path) = pick_file("Choose adb", list.root().and_downcast::<gtk::Window>().as_ref()).await {
                        model.update_config(|c| c.adb = path);
                        // Rebuilding the list is simplest: the new choice shows up as "Custom".
                        list.activate_action("adb.refresh", None).ok();
                    }
                });
            });
        }
    };
    let refresh = gio::SimpleAction::new("refresh", None);
    {
        let render = render.clone();
        refresh.connect_activate(move |_, _| render());
    }
    let actions = gio::SimpleActionGroup::new();
    actions.add_action(&refresh);
    list.insert_action_group("adb", Some(&actions));
    render();
    list
}

async fn pick_file(title: &str, window: Option<&gtk::Window>) -> Option<String> {
    let dialog = gtk::FileDialog::builder().title(title).build();
    let file = dialog.open_future(window).await.ok()?;
    Some(file.path()?.to_string_lossy().into())
}

/// A row that acts as a button (adw::ButtonRow needs libadwaita 1.6).
fn button_row(title: &str) -> adw::ActionRow {
    adw::ActionRow::builder().title(title).activatable(true).build()
}
