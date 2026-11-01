//! Tether for Linux: GTK4 + libadwaita on top of the shared Rust core.

mod device;
mod dialogs;
mod model;
mod setup;

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use model::Model;
use tether_core::Screen;

const APP_ID: &str = "io.github.msaudtahir.Tether";

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build);
    app.run()
}

const CSS: &str = "
.tether-card { padding: 20px; }
.tether-pill { padding: 4px 10px; border-radius: 999px; background: alpha(currentColor, 0.08); }
";

fn build(app: &adw::Application) {
    if let Some(window) = app.active_window() {
        window.present();
        return;
    }
    let css = gtk::CssProvider::new();
    css.load_from_string(CSS);
    gtk::style_context_add_provider_for_display(&gdk::Display::default().unwrap(), &css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    let model = Model::new();

    // Sidebar, like GNOME Settings: the phone up top (a menu switches phones), its sections below.
    let picker_title = gtk::Label::builder().xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).css_classes(["heading"]).build();
    let picker_subtitle = gtk::Label::builder().xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).css_classes(["caption", "dim-label"]).build();
    let picker_text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    picker_text.set_hexpand(true);
    picker_text.append(&picker_title);
    picker_text.append(&picker_subtitle);
    let picker_content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    picker_content.append(&gtk::Image::builder().icon_name("phone-symbolic").pixel_size(24).build());
    picker_content.append(&picker_text);
    picker_content.append(&gtk::Image::from_icon_name("pan-down-symbolic"));
    let devices = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).build();
    let pair_from_menu = gtk::Button::builder().label("Pair over Wi-Fi…").css_classes(["flat"]).build();
    let picker_menu = gtk::Box::new(gtk::Orientation::Vertical, 6);
    picker_menu.append(&devices);
    picker_menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    picker_menu.append(&pair_from_menu);
    let picker_popover = gtk::Popover::builder().child(&picker_menu).build();
    let picker = gtk::MenuButton::builder().child(&picker_content).popover(&picker_popover).tooltip_text("Switch phones").css_classes(["flat"]).build();
    picker.set_margin_start(6);
    picker.set_margin_end(6);

    let sections = gtk::ListBox::new();
    sections.add_css_class("navigation-sidebar");
    let notification_count = gtk::Label::builder().css_classes(["dim-label", "caption"]).build();
    for (name, title, icon) in [
        ("overview", "Overview", "phone-symbolic"),
        ("apps", "Apps", "view-app-grid-symbolic"),
        ("notifications", "Notifications", "preferences-system-notifications-symbolic"),
        ("photos", "Photos", "image-x-generic-symbolic"),
        ("files", "Files", "folder-symbolic"),
    ] {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.set_margin_top(6);
        row.set_margin_bottom(6);
        row.append(&gtk::Image::from_icon_name(icon));
        row.append(&gtk::Label::builder().label(title).xalign(0.0).hexpand(true).build());
        if name == "notifications" {
            row.append(&notification_count);
        }
        let item = gtk::ListBoxRow::builder().child(&row).name(name).build();
        sections.append(&item);
    }
    sections.select_row(sections.row_at_index(0).as_ref());
    let sidebar_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    sidebar_box.append(&picker);
    sidebar_box.append(&sections);
    let sidebar = adw::ToolbarView::new();
    sidebar.add_top_bar(&adw::HeaderBar::new());
    sidebar.set_content(Some(&gtk::ScrolledWindow::builder().child(&sidebar_box).vexpand(true).build()));

    // Content: the selected device.
    let header = adw::HeaderBar::new();
    let update = gtk::Button::builder().css_classes(["suggested-action"]).visible(false).build();
    let refresh = gtk::Button::builder().icon_name("view-refresh-symbolic").tooltip_text("Reload device info and the app list (Ctrl+R)").build();
    let pair = gtk::Button::builder().icon_name("network-wireless-symbolic").tooltip_text("Pair over Wi-Fi").build();
    let search = gtk::SearchEntry::builder().placeholder_text("Filter apps").width_chars(22).build();
    let menu = gio::Menu::new();
    menu.append(Some("Settings"), Some("app.settings"));
    menu.append(Some("About Tether"), Some("app.about"));
    let menu_button = gtk::MenuButton::builder().icon_name("open-menu-symbolic").menu_model(&menu).primary(true).build();
    let mirror = gtk::Button::builder().icon_name("view-mirror-symbolic").tooltip_text("Mirror the phone's screen (Ctrl+Shift+M)").action_name("app.mirror").build();
    header.pack_start(&mirror);
    header.pack_start(&pair);
    header.pack_start(&refresh);
    header.pack_end(&menu_button);
    header.pack_end(&search);
    header.pack_end(&update);

    let page = device::DevicePage::new(model.clone(), search.clone());
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(page.widget()));
    let content = adw::ToolbarView::new();
    content.add_top_bar(&header);
    content.set_content(Some(&toasts));

    let split = adw::NavigationSplitView::new();
    split.set_sidebar(Some(&adw::NavigationPage::new(&sidebar, "Tether")));
    split.set_content(Some(&adw::NavigationPage::new(&content, "Device")));

    let setup = setup::SetupView::new(model.clone());
    let stack = gtk::Stack::new();
    stack.add_named(&split, Some("main"));
    stack.add_named(setup.widget(), Some("setup"));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Tether")
        .default_width(1180)
        .default_height(820)
        .content(&stack)
        .build();
    let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        640.0,
        adw::LengthUnit::Sp,
    ));
    breakpoint.add_setter(&split, "collapsed", Some(&true.to_value()));
    window.add_breakpoint(breakpoint);

    // Actions and shortcuts.
    let action = |name: &str, accels: &[&str], f: Box<dyn Fn()>| {
        let a = gio::SimpleAction::new(name, None);
        a.connect_activate(move |_, _| f());
        app.add_action(&a);
        app.set_accels_for_action(&format!("app.{name}"), accels);
    };
    {
        let model = model.clone();
        action("refresh", &["<Control>r"], Box::new(move || model.refresh()));
    }
    {
        let model = model.clone();
        action("mirror", &["<Control><Shift>m"], Box::new(move || {
            if let Some(d) = model.selected_device().filter(|d| d.state == "device") {
                model.mirror(&d);
            }
        }));
    }
    {
        let (model, window) = (model.clone(), window.clone());
        action("settings", &["<Control>comma"], Box::new(move || dialogs::show_settings(&model, &window)));
    }
    {
        let window = window.clone();
        action("about", &[], Box::new(move || {
            adw::AboutDialog::builder()
                .application_name("Tether")
                .application_icon(APP_ID)
                .version(env!("CARGO_PKG_VERSION"))
                .website("https://github.com/MSaudTahir/tether")
                .comments("Use your Android phone from your computer.\n\nBuilt on scrcpy by Genymobile.")
                .build()
                .present(Some(&window));
        }));
    }
    refresh.set_action_name(Some("app.refresh"));
    {
        let (model, window) = (model.clone(), window.clone());
        pair.connect_clicked(move |_| dialogs::show_pair(&model, &window));
    }
    {
        let model = model.clone();
        update.connect_clicked(move |_| model.install_update());
    }
    {
        let (model, popover) = (model.clone(), picker_popover.clone());
        devices.connect_row_activated(move |_, row| {
            popover.popdown();
            model.select(Some(row.widget_name().to_string()));
        });
    }
    {
        let (model, window, popover) = (model.clone(), window.clone(), picker_popover.clone());
        pair_from_menu.connect_clicked(move |_| {
            popover.popdown();
            dialogs::show_pair(&model, &window);
        });
    }
    {
        let page = page.clone();
        sections.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                page.show_section(&row.widget_name());
            }
        });
    }

    // Errors and toasts.
    {
        let window = window.clone();
        model.connect_alert(move |message| {
            let dialog = adw::AlertDialog::new(Some("Something Went Wrong"), Some(message));
            dialog.add_response("ok", "OK");
            dialog.present(Some(&window));
        });
    }
    {
        let toasts = toasts.clone();
        model.connect_toast(move |message, file| {
            let toast = adw::Toast::new(message);
            if let Some(file) = file {
                toast.set_button_label(Some("Open"));
                toast.connect_button_clicked(move |_| {
                    let _ = gio::AppInfo::launch_default_for_uri(&gio::File::for_path(&file).uri(), None::<&gio::AppLaunchContext>);
                });
            }
            toasts.add_toast(toast);
        });
    }

    // Redraw on every change; the device list only when it changed.
    let shown_devices: Rc<RefCell<Vec<(String, String)>>> = Default::default();
    let render = {
        let (model, window, page) = (model.clone(), window.clone(), page.clone());
        move || {
            stack.set_visible_child_name(if model.needs_setup.get() { "setup" } else { "main" });
            if let Some(release) = &*model.update.borrow() {
                update.set_label(&if model.installing_update.get() { "Updating scrcpy…".into() } else { format!("Update scrcpy to {}", release.version) });
            }
            update.set_visible(model.update.borrow().is_some());
            update.set_sensitive(!model.installing_update.get());
            let ready = model.selected_device().is_some_and(|d| d.state == "device");
            refresh.set_sensitive(ready);
            mirror.set_sensitive(ready);
            model.screen.set(screen_of(&window));

            let rows: Vec<(String, String)> = model
                .devices
                .borrow()
                .iter()
                .map(|d| (d.serial.clone(), format!("{}\n{}", model.name(d), device_subtitle(&model, d))))
                .collect();
            if *shown_devices.borrow() != rows {
                devices.remove_all();
                for device in model.devices.borrow().iter() {
                    let row = adw::ActionRow::builder().title(model.name(device)).subtitle(device_subtitle(&model, device)).activatable(true).build();
                    row.add_prefix(&gtk::Image::from_icon_name("phone-symbolic"));
                    row.set_widget_name(&device.serial);
                    devices.append(&row);
                }
                *shown_devices.borrow_mut() = rows;
            }
            match model.selected_device() {
                Some(device) => {
                    picker_title.set_label(&model.name(&device));
                    picker_subtitle.set_label(&device_subtitle(&model, &device));
                }
                None => {
                    picker_title.set_label("No Phone");
                    picker_subtitle.set_label("Connect or pair a phone");
                }
            }
            sections.set_visible(ready);
            let count = model.selection.borrow().as_ref().and_then(|s| model.notifications.borrow().get(s).map(Vec::len)).unwrap_or(0);
            notification_count.set_label(&if count > 0 { count.to_string() } else { String::new() });
            page.render();
        }
    };
    let render = Rc::new(render);
    {
        let render = render.clone();
        model.connect_changed(move || render());
    }
    {
        let render = render.clone();
        search.connect_search_changed(move |_| render());
    }

    // Dropped files upload to the folder shown in Files; elsewhere, dropped .apk files are installed.
    let drop = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
    {
        let (model, page) = (model.clone(), page.clone());
        drop.connect_drop(move |_, value, _, _| {
            let Ok(files) = value.get::<gdk::FileList>() else { return false };
            let paths: Vec<_> = files.files().iter().filter_map(|f| f.path()).collect();
            let Some(device) = model.selected_device().filter(|_| !paths.is_empty()) else { return false };
            if page.section() == "files" {
                model.upload_files(paths, &device);
                return true;
            }
            let apks: Vec<_> = paths.into_iter().filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("apk"))).collect();
            if apks.is_empty() {
                return false;
            }
            model.install(apks, &device);
            true
        });
    }
    toasts.add_controller(drop);

    stop_sessions_on_quit(app, &model);
    render();
    window.present();
    model.start();
    #[cfg(debug_assertions)]
    snapshot_for_debugging(&window);
}

/// Debug builds: TETHER_SNAPSHOT=file.png saves the window as an image after a few seconds, to check
/// the layout from another machine.
#[cfg(debug_assertions)]
fn snapshot_for_debugging(window: &adw::ApplicationWindow) {
    let Ok(path) = std::env::var("TETHER_SNAPSHOT") else { return };
    // TETHER_SNAPSHOT_SECTION=apps|notifications picks the section to show.
    if let Ok(section) = std::env::var("TETHER_SNAPSHOT_SECTION") {
        let window = window.clone();
        glib::timeout_add_seconds_local_once(5, move || {
            if let Some(list) = find_sections(window.upcast_ref()) {
                let index = ["overview", "apps", "notifications", "photos", "files"].iter().position(|s| *s == section).unwrap_or(0);
                list.select_row(list.row_at_index(index as i32).as_ref());
            }
        });
    }
    let window = window.clone();
    glib::timeout_add_seconds_local_once(10, move || {
        let paintable = gtk::WidgetPaintable::new(Some(&window));
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(&snapshot, window.width() as f64, window.height() as f64);
        let renderer = window.native().and_then(|n| n.renderer());
        if let (Some(node), Some(renderer)) = (snapshot.to_node(), renderer) {
            let _ = renderer.render_texture(node, None).save_to_png(&path);
        }
    });
}

fn device_subtitle(model: &Model, device: &tether_core::Device) -> String {
    let transport = if device.wireless { "Wi-Fi" } else { "USB" };
    if device.state != "device" {
        return format!("{transport} · {}", device.state);
    }
    match model.info.borrow().get(&device.serial) {
        Some(info) => format!("{transport} · {}%", info.battery),
        None => transport.into(),
    }
}

/// The monitor the window is on (or the first one), in logical pixels.
fn screen_of(window: &adw::ApplicationWindow) -> Screen {
    let display = gtk::prelude::WidgetExt::display(window);
    let monitor = window
        .surface()
        .and_then(|surface| display.monitor_at_surface(&surface))
        .or_else(|| display.monitors().item(0).and_downcast::<gdk::Monitor>());
    match monitor {
        // Leave room for the top bar and dock.
        Some(m) => Screen { width: m.geometry().width() as u32, height: m.geometry().height() as u32 - 80, scale: m.scale() as f32, pixel_windows: false },
        None => Screen { width: 1280, height: 800, scale: 1.0, pixel_windows: false },
    }
}

fn stop_sessions_on_quit(app: &adw::Application, model: &Rc<Model>) {
    let core = model.core.clone();
    app.connect_shutdown(move |_| core.stop_sessions());
}

/// The sidebar's section list (debug snapshots only).
#[cfg(debug_assertions)]
fn find_sections(widget: &gtk::Widget) -> Option<gtk::ListBox> {
    if let Some(list) = widget.downcast_ref::<gtk::ListBox>().filter(|l| l.has_css_class("navigation-sidebar")) {
        return Some(list.clone());
    }
    let mut child = widget.first_child();
    while let Some(c) = child {
        if let Some(found) = find_sections(&c) {
            return Some(found);
        }
        child = c.next_sibling();
    }
    None
}
