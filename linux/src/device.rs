//! The selected device: header, controls, details and its apps.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use tether_core::*;

use crate::dialogs;
use crate::model::Model;

pub struct DevicePage {
    model: Rc<Model>,
    search: gtk::SearchEntry,
    stack: gtk::Stack,
    empty: adw::StatusPage,
    /// Header and controls, rebuilt on each render.
    top: gtk::Box,
    tabs: adw::ViewStack,
    notifications_box: gtk::Box,
    files_header: gtk::Box,
    files_state: gtk::Stack,
    files_error: adw::StatusPage,
    files_list: gtk::ListBox,
    /// What the file list shows, so it's only rebuilt when that changes.
    shown_files: RefCell<Option<Vec<PhoneFile>>>,
    details_box: gtk::Box,
    apps_count: gtk::Label,
    apps_hint: gtk::Label,
    apps_state: gtk::Stack,
    apps_error: adw::StatusPage,
    grid: gtk::FlowBox,
    /// What the grid shows, so it's only rebuilt when that changes.
    shown: RefCell<(Option<Rc<Vec<DeviceApp>>>, String)>,
    shown_apps: Rc<RefCell<Vec<DeviceApp>>>,
}

impl DevicePage {
    pub fn new(model: Rc<Model>, search: gtk::SearchEntry) -> Rc<DevicePage> {
        let empty = adw::StatusPage::new();
        let top = gtk::Box::new(gtk::Orientation::Vertical, 16);

        let title = gtk::Label::builder().label("Apps").css_classes(["title-2"]).build();
        let apps_count = gtk::Label::builder().css_classes(["title-2", "dim-label"]).build();
        let size_button = dialogs::window_size_button(&model);
        let title_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        title_row.append(&title);
        title_row.append(&apps_count);
        title_row.append(&gtk::Box::builder().hexpand(true).build());
        title_row.append(&size_button);
        let apps_hint = gtk::Label::builder().xalign(0.0).css_classes(["dim-label"]).build();

        let grid = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .activate_on_single_click(true)
            .homogeneous(true)
            .min_children_per_line(3)
            .max_children_per_line(14)
            .column_spacing(4)
            .row_spacing(4)
            .build();
        let apps_error = adw::StatusPage::builder().icon_name("dialog-warning-symbolic").title("Couldn't List Apps").build();
        let retry = gtk::Button::builder().label("Try Again").halign(gtk::Align::Center).css_classes(["pill"]).action_name("app.refresh").build();
        apps_error.set_child(Some(&retry));
        let apps_state = gtk::Stack::new();
        apps_state.add_named(&gtk::Spinner::builder().spinning(true).width_request(32).height_request(32).margin_top(60).margin_bottom(60).build(), Some("loading"));
        apps_state.add_named(&apps_error, Some("error"));
        apps_state.add_named(&grid, Some("apps"));

        let apps = gtk::Box::new(gtk::Orientation::Vertical, 10);
        apps.append(&title_row);
        apps.append(&apps_hint);
        apps.append(&apps_state);
        let notifications_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
        let details_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
        // Overview: the phone's header, controls and details.
        let overview = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(16).valign(gtk::Align::Start).build();
        overview.append(&top);
        overview.append(&details_box);
        let tabs = adw::ViewStack::builder().vhomogeneous(false).build();
        tabs.add_named(&overview, Some("overview"));
        tabs.add_named(&apps, Some("apps"));
        tabs.add_named(&notifications_box, Some("notifications"));
        let files_header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let files_list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).css_classes(["boxed-list"]).valign(gtk::Align::Start).build();
        let files_error = adw::StatusPage::builder().icon_name("folder-symbolic").title("Can't Open This Folder").build();
        let files_state = gtk::Stack::builder().vhomogeneous(false).build();
        files_state.add_named(&files_list, Some("list"));
        files_state.add_named(&files_error, Some("error"));
        files_state.add_named(&adw::StatusPage::builder().icon_name("folder-symbolic").title("Empty Folder").description("Drop files here to copy them to the phone.").build(), Some("empty"));
        let files_page = gtk::Box::new(gtk::Orientation::Vertical, 12);
        files_page.append(&files_header);
        files_page.append(&files_state);
        tabs.add_named(&files_page, Some("files"));
        let clamp = adw::Clamp::builder().maximum_size(1400).tightening_threshold(1000).child(&tabs).build();
        clamp.set_margin_top(24);
        clamp.set_margin_bottom(24);
        clamp.set_margin_start(24);
        clamp.set_margin_end(24);
        let device_view = gtk::ScrolledWindow::builder().child(&clamp).hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).build();

        let stack = gtk::Stack::new();
        stack.add_named(&empty, Some("empty"));
        stack.add_named(&device_view, Some("device"));

        let page = Rc::new(DevicePage {
            model,
            search,
            stack,
            empty,
            top,
            tabs,
            notifications_box,
            files_header,
            files_state,
            files_error,
            files_list,
            shown_files: Default::default(),
            details_box,
            apps_count,
            apps_hint,
            apps_state,
            apps_error,
            grid,
            shown: Default::default(),
            shown_apps: Default::default(),
        });
        {
            let (model, apps) = (page.model.clone(), page.shown_apps.clone());
            page.grid.connect_child_activated(move |_, child| {
                let app = apps.borrow().get(child.index() as usize).cloned();
                if let (Some(app), Some(device)) = (app, model.selected_device()) {
                    model.open(&app, &device);
                }
            });
        }
        page
    }

    /// Shows a section: "overview", "apps", "notifications" or "files".
    pub fn show_section(&self, name: &str) {
        self.tabs.set_visible_child_name(name);
        if name == "files" {
            if let Some(device) = self.model.selected_device().filter(|d| !self.model.files.borrow().contains_key(&d.serial)) {
                self.model.load_files(&device);
            }
        }
    }

    pub fn section(&self) -> String {
        self.tabs.visible_child_name().map(|n| n.to_string()).unwrap_or_default()
    }

    pub fn widget(&self) -> &gtk::Stack {
        &self.stack
    }

    pub fn render(&self) {
        let model = &self.model;
        let Some(device) = model.selected_device() else {
            self.show_empty("phone-symbolic", "No Devices", "Plug in a phone with USB debugging on, or pair one over Wi-Fi.");
            return;
        };
        if device.state != "device" {
            let hint = match device.state.as_str() {
                "unauthorized" => "Unlock the phone and accept the “Allow USB debugging?” prompt.".into(),
                "offline" => "The device is offline. Try reconnecting it.".into(),
                state => format!("The device is {state}."),
            };
            self.show_empty("system-lock-screen-symbolic", &device.label, &hint);
            return;
        }
        self.stack.set_visible_child_name("device");

        let info = model.info.borrow().get(&device.serial).cloned();
        while let Some(child) = self.top.first_child() {
            self.top.remove(&child);
        }
        self.top.append(&self.header(&device, info.as_ref()));
        if let Some(info) = &info {
            self.top.append(&self.controls(&device, info));
            self.top.append(&self.clipboard(&device));
        }
        replace_children(&self.details_box, info.as_ref().map(details));
        replace_children(&self.notifications_box, Some(self.notifications(&device)));
        self.render_files(&device);

        let supported = info.as_ref().map_or(true, |i| i.sdk >= virtual_display_min_sdk());
        self.apps_hint.set_label(if supported { "Click an app to open it in its own window." } else { "App windows need Android 10 or newer." });
        self.grid.set_sensitive(supported);
        let apps = model.apps.borrow().get(&device.serial).cloned();
        let error = model.app_errors.borrow().get(&device.serial).cloned();
        self.apps_count.set_label(&apps.as_ref().map_or(String::new(), |a| a.len().to_string()));
        if model.loading_apps.borrow().contains(&device.serial) && apps.is_none() {
            self.apps_state.set_visible_child_name("loading");
        } else if let Some(error) = error {
            self.apps_error.set_description(Some(&error));
            self.apps_state.set_visible_child_name("error");
        } else {
            self.apps_state.set_visible_child_name("apps");
        }
        self.render_apps(apps);
    }

    fn show_empty(&self, icon: &str, title: &str, description: &str) {
        self.empty.set_icon_name(Some(icon));
        self.empty.set_title(title);
        self.empty.set_description(Some(description));
        self.stack.set_visible_child_name("empty");
    }

    fn header(&self, device: &Device, info: Option<&DeviceInfo>) -> gtk::Widget {
        let model = &self.model;
        let card = card();
        let name = gtk::Label::builder().label(model.name(device)).xalign(0.0).css_classes(["title-1"]).build();
        card.append(&name);
        if let Some(info) = info {
            let subtitle = gtk::Label::builder().label(format!("{} · Android {}", info.model, info.android)).xalign(0.0).css_classes(["title-4", "dim-label"]).build();
            card.append(&subtitle);
        }
        let pills = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        if let Some(info) = info {
            let charging = info.power.starts_with("charging") || info.power.starts_with("full");
            let icon = if charging { "battery-full-charging-symbolic" } else { "battery-good-symbolic" };
            pills.append(&pill(icon, &format!("{}% · {}", info.battery, capitalized(&info.power))));
        }
        pills.append(&pill(if device.wireless { "network-wireless-symbolic" } else { "media-removable-symbolic" }, if device.wireless { "Wi-Fi" } else { "USB" }));
        pills.append(&pill("tag-symbolic", &device.short_serial));
        card.append(&pills);

        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        buttons.set_margin_top(8);
        let mirror = labeled_button("view-mirror-symbolic", "Mirror Screen");
        mirror.add_css_class("suggested-action");
        mirror.add_css_class("pill");
        mirror.set_tooltip_text(Some("Mirror the phone's screen (Ctrl+Shift+M)"));
        mirror.set_action_name(Some("app.mirror"));
        buttons.append(&mirror);
        if !device.wireless {
            let wireless = labeled_button("network-wireless-symbolic", "Go Wireless");
            wireless.add_css_class("pill");
            let (model, d) = (model.clone(), device.clone());
            wireless.connect_clicked(move |_| model.go_wireless(&d));
            buttons.append(&wireless);
        }
        let capture = gtk::ToggleButton::builder()
            .child(&adw::ButtonContent::builder().icon_name("input-mouse-symbolic").label("Capture Mouse").build())
            .active(model.config.borrow().capture_mouse)
            .css_classes(["pill"])
            .tooltip_text("Use the mouse like a real one on the phone (for games and apps that capture the pointer). Press Left Alt or Super to release it.")
            .build();
        {
            let model = model.clone();
            capture.connect_toggled(move |b| model.update_config(|c| c.capture_mouse = b.is_active()));
        }
        buttons.append(&capture);
        card.append(&buttons);
        card.upcast()
    }

    /// Buttons and switches that act on the phone without opening a window.
    fn controls(&self, device: &Device, info: &DeviceInfo) -> gtk::Widget {
        let model = &self.model;
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);

        let screenshot = labeled_button("camera-photo-symbolic", "Screenshot");
        screenshot.set_tooltip_text(Some("Save a screenshot to Pictures/Screenshots"));
        {
            let (model, d) = (model.clone(), device.clone());
            screenshot.connect_clicked(move |_| model.screenshot(&d));
        }
        row.append(&screenshot);
        let installing = model.installing.borrow().contains(&device.serial);
        let install = labeled_button("system-software-install-symbolic", if installing { "Installing…" } else { "Install APK…" });
        install.set_sensitive(!installing);
        install.set_tooltip_text(Some("Install an app from an .apk file. You can also drop .apk files on this window."));
        {
            let (model, d) = (model.clone(), device.clone());
            install.connect_clicked(move |button| {
                let (model, d) = (model.clone(), d.clone());
                let window = button.root().and_downcast::<gtk::Window>();
                glib::spawn_future_local(async move {
                    let filter = gtk::FileFilter::new();
                    filter.set_name(Some("Android apps"));
                    filter.add_suffix("apk");
                    let filters = gio::ListStore::new::<gtk::FileFilter>();
                    filters.append(&filter);
                    let dialog = gtk::FileDialog::builder().title("Install APK").filters(&filters).build();
                    if let Ok(files) = dialog.open_multiple_future(window.as_ref()).await {
                        let apks: Vec<_> = (0..files.n_items()).filter_map(|i| files.item(i).and_downcast::<gio::File>()?.path()).collect();
                        if !apks.is_empty() {
                            model.install(apks, &d);
                        }
                    }
                });
            });
        }
        row.append(&install);
        row.append(&gtk::Separator::new(gtk::Orientation::Vertical));

        let keys = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        keys.add_css_class("linked");
        for (key, icon, tip) in [
            (DeviceKey::Back, "go-previous-symbolic", "Back"),
            (DeviceKey::Home, "go-home-symbolic", "Home"),
            (DeviceKey::RecentApps, "view-app-grid-symbolic", "Recent apps"),
            (DeviceKey::VolumeDown, "audio-volume-low-symbolic", "Volume down"),
            (DeviceKey::VolumeUp, "audio-volume-high-symbolic", "Volume up"),
            (DeviceKey::Power, if info.screen_on { "system-lock-screen-symbolic" } else { "display-brightness-symbolic" }, if info.screen_on { "Lock screen" } else { "Wake screen" }),
        ] {
            let button = gtk::Button::builder().icon_name(icon).tooltip_text(tip).build();
            let (model, d) = (model.clone(), device.clone());
            button.connect_clicked(move |_| model.press(key, &d));
            keys.append(&button);
        }
        row.append(&keys);
        row.append(&gtk::Separator::new(gtk::Orientation::Vertical));

        let dark = toggle("weather-clear-night-symbolic", "Dark Mode", info.dark_mode);
        {
            let (model, d) = (model.clone(), device.clone());
            dark.connect_toggled(move |b| model.set_dark_mode(b.is_active(), &d));
        }
        row.append(&dark);
        let awake = toggle("preferences-desktop-screensaver-symbolic", "Stay Awake", info.stay_awake);
        awake.set_tooltip_text(Some("Keep the screen on while charging"));
        {
            let (model, d) = (model.clone(), device.clone());
            awake.connect_toggled(move |b| model.set_stay_awake(b.is_active(), &d));
        }
        row.append(&awake);
        row.append(&gtk::Box::builder().hexpand(true).build());

        let restart_menu = gio::Menu::new();
        restart_menu.append(Some("Restart"), Some("device.restart::normal"));
        restart_menu.append(Some("Restart to Recovery"), Some("device.restart::recovery"));
        restart_menu.append(Some("Restart to Bootloader"), Some("device.restart::bootloader"));
        let restart = gtk::MenuButton::builder()
            .child(&adw::ButtonContent::builder().icon_name("system-reboot-symbolic").label("Restart").build())
            .menu_model(&restart_menu)
            .build();
        let actions = gio::SimpleActionGroup::new();
        let action = gio::SimpleAction::new("restart", Some(glib::VariantTy::STRING));
        {
            let (model, d, name) = (model.clone(), device.clone(), info.name.clone());
            action.connect_activate(move |_, mode| {
                let mode = match mode.and_then(|m| m.str()) {
                    Some("recovery") => RebootMode::Recovery,
                    Some("bootloader") => RebootMode::Bootloader,
                    _ => RebootMode::Normal,
                };
                confirm_reboot(&model, &d, &name, mode);
            });
        }
        actions.add_action(&action);
        row.insert_action_group("device", Some(&actions));
        row.append(&restart);

        let card = card();
        card.append(&row);
        card.upcast()
    }

    /// The phone's current notifications, as separate cards like the phone shows them.
    fn notifications(&self, device: &Device) -> gtk::Widget {
        let all = self.model.notifications.borrow().get(&device.serial).cloned().unwrap_or_default();
        if all.is_empty() {
            return adw::StatusPage::builder()
                .icon_name("preferences-system-notifications-symbolic")
                .title("No Notifications")
                .description("Notifications on the phone show up here.")
                .build()
                .upcast();
        }
        let list = gtk::Box::new(gtk::Orientation::Vertical, 10);
        let buddy = self.model.buddy.borrow().get(&device.serial).copied();
        if let Some(state) = buddy.filter(|s| *s != BuddyState::Ready) {
            list.append(&self.buddy_banner(state, device));
        }
        let hint = if buddy == Some(BuddyState::Ready) {
            "Click a notification to open it on the phone's screen."
        } else {
            "Click a notification to show it on the phone's screen and tap it there."
        };
        list.append(&gtk::Label::builder().label(hint).xalign(0.0).css_classes(["dim-label"]).build());
        for notification in all {
            list.append(&self.notification_card(notification, device));
        }
        adw::Clamp::builder().maximum_size(720).child(&list).build().upcast()
    }

    /// Offers Tether Buddy, which lets a click run the notification's own action.
    fn buddy_banner(&self, state: BuddyState, device: &Device) -> gtk::Widget {
        let installing = self.model.installing_buddy.borrow().contains(&device.serial);
        let row = adw::ActionRow::builder()
            .title(if state == BuddyState::Outdated { "Update Tether Buddy" } else { "Open notifications directly with Tether Buddy" })
            .subtitle("A tiny app with no screens that Tether puts on the phone. With it, clicking a notification does what tapping it does, like opening that chat.")
            .build();
        row.add_prefix(&gtk::Image::builder().icon_name("starred-symbolic").pixel_size(24).build());
        let label = match (installing, state) {
            (true, _) => "Installing…",
            (_, BuddyState::Outdated) => "Update",
            (_, BuddyState::Disabled) => "Turn On",
            _ => "Install",
        };
        let button = gtk::Button::builder().label(label).valign(gtk::Align::Center).css_classes(["suggested-action"]).sensitive(!installing).build();
        let (model, device) = (self.model.clone(), device.clone());
        button.connect_clicked(move |_| model.install_buddy(&device));
        row.add_suffix(&button);
        let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).css_classes(["boxed-list"]).build();
        list.append(&row);
        list.upcast()
    }

    fn notification_card(&self, notification: PhoneNotification, device: &Device) -> gtk::Widget {
        let app_name = if notification.app_name.is_empty() { notification.package.clone() } else { notification.app_name.clone() };
        let when = glib::DateTime::from_unix_local((notification.time / 1000) as i64)
            .ok()
            .filter(|_| notification.time > 0)
            .and_then(|t| t.format(" · %H:%M").ok())
            .map(|t| t.to_string())
            .unwrap_or_default();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(14);
        content.set_margin_end(14);
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        header.append(&app_tile_icon(&DeviceApp { name: app_name.clone(), package: notification.package.clone(), system: false, icon_path: notification.icon_path.clone() }, 20));
        header.append(&gtk::Label::builder().label(format!("{app_name}{when}")).xalign(0.0).css_classes(["caption", "dim-label"]).build());
        content.append(&header);
        if !notification.title.is_empty() {
            content.append(&gtk::Label::builder().label(&notification.title).xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).css_classes(["heading"]).build());
        }
        if !notification.text.is_empty() {
            content.append(&gtk::Label::builder().label(&notification.text).xalign(0.0).wrap(true).wrap_mode(gtk::pango::WrapMode::WordChar).lines(4).ellipsize(gtk::pango::EllipsizeMode::End).build());
        }
        let card = gtk::Button::builder().child(&content).css_classes(["card"]).tooltip_text("Open on the phone's screen").build();
        {
            let (model, device, notification) = (self.model.clone(), device.clone(), notification.clone());
            card.connect_clicked(move |_| model.show_notification(&notification, &device));
        }
        let open = gtk::Button::builder()
            .label("Open App")
            .tooltip_text(format!("Open {app_name}"))
            .css_classes(["flat", "caption"])
            .halign(gtk::Align::End)
            .valign(gtk::Align::Start)
            .margin_top(6)
            .margin_end(6)
            .build();
        {
            let (model, device) = (self.model.clone(), device.clone());
            open.connect_clicked(move |_| model.open_notification_app(&notification, &device));
        }
        let overlay = gtk::Overlay::builder().child(&card).build();
        overlay.add_overlay(&open);
        overlay.upcast()
    }

    /// Files: the toolbar, then the folder's contents (rebuilt only when they change).
    fn render_files(&self, device: &Device) {
        let model = &self.model;
        let folder = model.current_folder(&device.serial);
        while let Some(child) = self.files_header.first_child() {
            self.files_header.remove(&child);
        }
        let up = gtk::Button::builder().icon_name("go-up-symbolic").tooltip_text("Up one folder").sensitive(folder != "/").build();
        {
            let (model, d, parent) = (model.clone(), device.clone(), folder.rsplit_once('/').map_or("/", |(p, _)| if p.is_empty() { "/" } else { p }).to_string());
            up.connect_clicked(move |_| model.open_folder(parent.clone(), &d));
        }
        self.files_header.append(&up);
        self.files_header.append(&breadcrumbs(model, device, &folder));
        self.files_header.append(&gtk::Box::builder().hexpand(true).build());
        if model.transfers.borrow().get(&device.serial).copied().unwrap_or(0) > 0 {
            self.files_header.append(&gtk::Spinner::builder().spinning(true).build());
        }
        let hidden = gtk::ToggleButton::builder().icon_name("view-reveal-symbolic").tooltip_text("Show hidden files").active(model.show_hidden_files.get()).build();
        {
            let (model, d) = (model.clone(), device.clone());
            hidden.connect_toggled(move |b| {
                model.show_hidden_files.set(b.is_active());
                model.load_files(&d);
            });
        }
        let new_folder = gtk::Button::builder().icon_name("folder-new-symbolic").tooltip_text("New folder").build();
        {
            let (model, d) = (model.clone(), device.clone());
            new_folder.connect_clicked(move |b| ask_name(b, "New Folder", "Create", "", {
                let (model, d) = (model.clone(), d.clone());
                move |name| model.make_folder(&name, &d)
            }));
        }
        let upload = labeled_button("document-send-symbolic", "Upload…");
        upload.set_tooltip_text(Some("Copy files from this computer to this folder. You can also drop files here."));
        {
            let (model, d) = (model.clone(), device.clone());
            upload.connect_clicked(move |button| {
                let (model, d, window) = (model.clone(), d.clone(), button.root().and_downcast::<gtk::Window>());
                glib::spawn_future_local(async move {
                    if let Ok(files) = gtk::FileDialog::builder().title("Upload to the Phone").build().open_multiple_future(window.as_ref()).await {
                        let paths: Vec<_> = (0..files.n_items()).filter_map(|i| files.item(i).and_downcast::<gio::File>()?.path()).collect();
                        model.upload_files(paths, &d);
                    }
                });
            });
        }
        let refresh = gtk::Button::builder().icon_name("view-refresh-symbolic").tooltip_text("Reload this folder").build();
        {
            let (model, d) = (model.clone(), device.clone());
            refresh.connect_clicked(move |_| model.load_files(&d));
        }
        for widget in [hidden.upcast_ref::<gtk::Widget>(), new_folder.upcast_ref(), upload.upcast_ref(), refresh.upcast_ref()] {
            self.files_header.append(widget);
        }

        if let Some(error) = model.files_error.borrow().get(&device.serial) {
            self.files_error.set_description(Some(error));
            self.files_state.set_visible_child_name("error");
            return;
        }
        let files = model.files.borrow().get(&device.serial).cloned();
        if *self.shown_files.borrow() == files {
            return;
        }
        self.files_list.remove_all();
        for item in files.iter().flatten() {
            self.files_list.append(&self.file_row(item, device));
        }
        self.files_state.set_visible_child_name(if files.as_ref().is_some_and(|f| f.is_empty()) { "empty" } else { "list" });
        *self.shown_files.borrow_mut() = files;
    }

    fn file_row(&self, item: &PhoneFile, device: &Device) -> adw::ActionRow {
        let date = glib::DateTime::from_unix_local(item.modified).ok().and_then(|t| t.format("%e %b %Y, %H:%M").ok()).map(|t| t.trim().to_string()).unwrap_or_default();
        let subtitle = if item.folder { format!("Folder · {date}") } else { format!("{} · {date}", bytes(item.size)) };
        let row = adw::ActionRow::builder().title(glib::markup_escape_text(&item.name)).subtitle(subtitle).activatable(true).build();
        row.add_prefix(&gtk::Image::from_icon_name(file_icon(item)));
        {
            let (model, d, item) = (self.model.clone(), device.clone(), item.clone());
            row.connect_activated(move |_| model.open_file(&item, &d));
        }
        // ⋮ menu: Download, Rename, Delete.
        let actions = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let popover = gtk::Popover::builder().child(&actions).build();
        let entry = |label: &str, action: Box<dyn Fn(&gtk::Button)>| {
            let button = gtk::Button::builder().label(label).css_classes(["flat"]).build();
            let popover = popover.clone();
            button.connect_clicked(move |b| {
                popover.popdown();
                action(b);
            });
            actions.append(&button);
        };
        {
            let (model, d, item) = (self.model.clone(), device.clone(), item.clone());
            entry("Download to Downloads", Box::new(move |_| model.download_files(vec![item.clone()], &d)));
        }
        {
            let (model, d, item) = (self.model.clone(), device.clone(), item.clone());
            entry("Rename…", Box::new(move |b| {
                let (model, d, item2) = (model.clone(), d.clone(), item.clone());
                ask_name(b, "Rename", "Rename", &item.name, move |name| model.rename_file(&item2, &name, &d));
            }));
        }
        {
            let (model, d, item) = (self.model.clone(), device.clone(), item.clone());
            entry("Delete…", Box::new(move |b| confirm_delete(b, &model, &item, &d)));
        }
        let menu = gtk::MenuButton::builder().icon_name("view-more-symbolic").popover(&popover).valign(gtk::Align::Center).css_classes(["flat"]).build();
        row.add_suffix(&menu);
        row
    }

    /// Moves text between this computer's and the phone's clipboards.
    fn clipboard(&self, device: &Device) -> gtk::Widget {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let label = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        label.add_css_class("dim-label");
        label.append(&gtk::Image::from_icon_name("edit-paste-symbolic"));
        label.append(&gtk::Label::new(Some("Clipboard")));
        row.append(&label);
        let send = labeled_button("send-to-symbolic", "Send Clipboard to Phone");
        send.set_tooltip_text(Some("Put this computer's clipboard text on the phone's clipboard"));
        {
            let (model, d) = (self.model.clone(), device.clone());
            send.connect_clicked(move |_| model.send_clipboard(&d));
        }
        let copy = labeled_button("edit-copy-symbolic", "Copy Phone Clipboard");
        copy.set_tooltip_text(Some("Copy the phone's clipboard text to this computer"));
        {
            let (model, d) = (self.model.clone(), device.clone());
            copy.connect_clicked(move |_| model.copy_clipboard(&d));
        }
        row.append(&send);
        row.append(&copy);
        let card = card();
        card.append(&row);
        card.upcast()
    }

    fn render_apps(&self, apps: Option<Rc<Vec<DeviceApp>>>) {
        let search = self.search.text().trim().to_lowercase();
        {
            let shown = self.shown.borrow();
            let same = match (&shown.0, &apps) {
                (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            };
            if same && shown.1 == search {
                return;
            }
        }
        *self.shown.borrow_mut() = (apps.clone(), search.clone());
        self.grid.remove_all();
        let visible: Vec<DeviceApp> = apps
            .iter()
            .flat_map(|a| a.iter())
            .filter(|app| search.is_empty() || app.name.to_lowercase().contains(&search) || app.package.to_lowercase().contains(&search))
            .cloned()
            .collect();
        for app in &visible {
            self.grid.append(&app_tile(app));
        }
        *self.shown_apps.borrow_mut() = visible;
    }
}

fn confirm_reboot(model: &Rc<Model>, device: &Device, name: &str, mode: RebootMode) {
    let body = match mode {
        RebootMode::Normal => "The phone will disconnect while it restarts.",
        RebootMode::Recovery => "The phone will start in recovery mode. Only do this if you know how to use it.",
        RebootMode::Bootloader => "The phone will start in its bootloader. Only do this if you know how to use it.",
    };
    let dialog = adw::AlertDialog::new(Some(&format!("Restart {name}?")), Some(body));
    dialog.add_responses(&[("cancel", "Cancel"), ("restart", "Restart")]);
    dialog.set_response_appearance("restart", adw::ResponseAppearance::Destructive);
    let (model, device) = (model.clone(), device.clone());
    dialog.connect_response(None, move |_, response| {
        if response == "restart" {
            model.reboot(&device, mode);
        }
    });
    let window = gio::Application::default().and_downcast::<gtk::Application>().and_then(|a| a.active_window());
    dialog.present(window.as_ref());
}

/// Storage, memory, battery and hardware details.
fn details(info: &DeviceInfo) -> gtk::Widget {
    let left = gtk::Grid::builder().column_spacing(16).row_spacing(12).build();
    let right = gtk::Grid::builder().column_spacing(16).row_spacing(12).build();
    if info.storage_total > 0 {
        let free = info.storage_total - info.storage_used;
        add_meter(&left, "drive-harddisk-symbolic", "Storage", info.storage_used, info.storage_total, &format!("{} free of {}", bytes(free), bytes(info.storage_total)));
    }
    if info.memory_total > 0 {
        let used = info.memory_total - info.memory_available;
        add_meter(&left, "memory-symbolic", "Memory", used, info.memory_total, &format!("{} available of {}", bytes(info.memory_available), bytes(info.memory_total)));
    }
    let mut battery = vec![format!("{}%", info.battery), capitalized(&info.power)];
    if !info.battery_health.is_empty() {
        battery.push(format!("{} health", capitalized(&info.battery_health)));
    }
    if info.battery_temperature > 0.0 {
        battery.push(format!("{:.1} °C", info.battery_temperature));
    }
    add_row(&left, "battery-good-symbolic", "Battery", &battery.join(" · "));
    if !info.chipset.is_empty() {
        add_row(&right, "processor-symbolic", "Chipset", &info.chipset);
    }
    if !info.screen.is_empty() {
        add_row(&right, "phone-symbolic", "Display", &info.screen);
    }
    let android = match glib::DateTime::from_iso8601(&format!("{}T00:00:00Z", info.security_patch), None) {
        Ok(patch) => format!("Android {} · Security patch {}", info.android, patch.format("%B %Y").map(|s| s.to_string()).unwrap_or_default()),
        Err(_) => format!("Android {}", info.android),
    };
    add_row(&right, "security-high-symbolic", "Android", &android);
    if info.uptime_seconds > 0 {
        let (days, hours, minutes) = (info.uptime_seconds / 86400, info.uptime_seconds / 3600 % 24, info.uptime_seconds / 60 % 60);
        let uptime = if days > 0 { format!("{days} days, {hours} hours") } else { format!("{hours} hours, {minutes} minutes") };
        add_row(&right, "preferences-system-time-symbolic", "Up for", &uptime);
    }
    if let Some(address) = &info.wifi_address {
        add_row(&right, "network-wireless-symbolic", "Wi-Fi address", address);
    }
    let columns = gtk::Box::new(gtk::Orientation::Horizontal, 40);
    columns.append(&left);
    columns.append(&right);
    let card = card();
    card.append(&columns);
    card.upcast()
}

fn add_row(grid: &gtk::Grid, icon: &str, title: &str, value: &str) {
    // Not wrapped: a wrapping label reserves the height it would need at its narrowest.
    let value = gtk::Label::builder().label(value).xalign(0.0).selectable(true).build();
    add(grid, icon, title, &value);
}

fn add_meter(grid: &gtk::Grid, icon: &str, title: &str, used: u64, total: u64, text: &str) {
    let meter = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let bar = gtk::LevelBar::builder().max_value(total as f64).value(used as f64).width_request(240).build();
    // The default offsets color high values as "good"; here high is the warning.
    bar.remove_offset_value(Some(gtk::LEVEL_BAR_OFFSET_LOW));
    bar.remove_offset_value(Some(gtk::LEVEL_BAR_OFFSET_HIGH));
    bar.remove_offset_value(Some(gtk::LEVEL_BAR_OFFSET_FULL));
    if used as f64 / total as f64 > 0.9 {
        bar.add_css_class("warning");
    }
    meter.append(&bar);
    meter.append(&gtk::Label::builder().label(text).xalign(0.0).css_classes(["caption", "dim-label"]).build());
    add(grid, icon, title, &meter);
}

fn add(grid: &gtk::Grid, icon: &str, title: &str, value: &impl IsA<gtk::Widget>) {
    let mut row = 0;
    while grid.child_at(0, row).is_some() {
        row += 1;
    }
    let label = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    label.add_css_class("dim-label");
    label.set_valign(gtk::Align::Start);
    label.append(&gtk::Image::from_icon_name(icon));
    label.append(&gtk::Label::new(Some(title)));
    grid.attach(&label, 0, row, 1, 1);
    grid.attach(value, 1, row, 1, 1);
}

fn app_tile(app: &DeviceApp) -> gtk::Widget {
    let tile = gtk::Box::new(gtk::Orientation::Vertical, 6);
    tile.set_margin_top(8);
    tile.set_margin_bottom(4);
    tile.set_width_request(96);
    tile.set_tooltip_text(Some(&app.package));
    tile.append(&app_tile_icon(app, 56));
    tile.append(
        &gtk::Label::builder()
            .label(&app.name)
            .justify(gtk::Justification::Center)
            .wrap(true)
            .lines(2)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(12)
            .css_classes(["caption"])
            .build(),
    );
    tile.upcast()
}

fn card() -> gtk::Box {
    gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(10).css_classes(["card", "tether-card"]).build()
}

fn pill(icon: &str, text: &str) -> gtk::Widget {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    content.append(&gtk::Image::from_icon_name(icon));
    content.append(&gtk::Label::builder().label(text).selectable(true).build());
    let pill = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    pill.add_css_class("tether-pill");
    pill.append(&content);
    pill.upcast()
}

fn labeled_button(icon: &str, label: &str) -> gtk::Button {
    gtk::Button::builder().child(&adw::ButtonContent::builder().icon_name(icon).label(label).build()).build()
}

fn toggle(icon: &str, label: &str, active: bool) -> gtk::ToggleButton {
    gtk::ToggleButton::builder().child(&adw::ButtonContent::builder().icon_name(icon).label(label).build()).active(active).build()
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or(String::new(), |c| c.to_uppercase().chain(chars).collect())
}

fn bytes(count: u64) -> String {
    glib::format_size(count).to_string()
}

/// The app's cached icon, or its initials on a color.
fn app_tile_icon(app: &DeviceApp, size: i32) -> gtk::Widget {
    match &app.icon_path {
        Some(path) => gtk::Picture::builder()
            .paintable(&gdk::Texture::from_filename(path).map(|t| t.upcast::<gdk::Paintable>()).unwrap_or_else(|_| gdk::Paintable::new_empty(size, size)))
            .width_request(size)
            .height_request(size)
            .content_fit(gtk::ContentFit::Contain)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build()
            .upcast(),
        None => {
            let letter = adw::Avatar::new(size, Some(&app.name), true);
            letter.set_halign(gtk::Align::Center);
            letter.set_valign(gtk::Align::Center);
            letter.upcast()
        }
    }
}

fn replace_children(container: &gtk::Box, child: Option<gtk::Widget>) {
    while let Some(old) = container.first_child() {
        container.remove(&old);
    }
    if let Some(child) = child {
        container.append(&child);
    }
}

/// "Phone › DCIM › Camera": click a part to go there.
fn breadcrumbs(model: &Rc<Model>, device: &Device, path: &str) -> gtk::Widget {
    let root = phone_storage();
    let in_storage = path == root || path.starts_with(&format!("{root}/"));
    let rest = if in_storage { &path[root.len()..] } else { path };
    let parts: Vec<&str> = rest.split('/').filter(|p| !p.is_empty()).collect();
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    let crumb = |label: &str, target: String, bold: bool| {
        let button = gtk::Button::builder().label(label).css_classes(["flat"]).build();
        if bold {
            button.add_css_class("heading");
        }
        let (model, d) = (model.clone(), device.clone());
        button.connect_clicked(move |_| model.open_folder(target.clone(), &d));
        button
    };
    if in_storage {
        row.append(&crumb("Phone", root.clone(), parts.is_empty()));
    }
    for (i, part) in parts.iter().enumerate() {
        if in_storage || i > 0 {
            row.append(&gtk::Image::from_icon_name("go-next-symbolic"));
        }
        let target = format!("{}/{}", if in_storage { root.as_str() } else { "" }, parts[..=i].join("/"));
        row.append(&crumb(part, target, i == parts.len() - 1));
    }
    row.upcast()
}

fn file_icon(item: &PhoneFile) -> &'static str {
    if item.folder {
        return "folder-symbolic";
    }
    let ext = item.name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" => "image-x-generic-symbolic",
        "mp4" | "mkv" | "mov" | "webm" | "3gp" => "video-x-generic-symbolic",
        "mp3" | "m4a" | "aac" | "ogg" | "opus" | "wav" | "flac" => "audio-x-generic-symbolic",
        "zip" | "rar" | "7z" | "tar" | "gz" => "package-x-generic-symbolic",
        "apk" => "application-x-executable-symbolic",
        _ => "text-x-generic-symbolic",
    }
}

/// Asks for a name (new folder, rename); calls `done` with a non-empty one.
fn ask_name(anchor: &impl IsA<gtk::Widget>, heading: &str, action: &str, initial: &str, done: impl Fn(String) + 'static) {
    let entry = gtk::Entry::builder().text(initial).activates_default(true).build();
    let dialog = adw::AlertDialog::builder().heading(heading).extra_child(&entry).build();
    dialog.add_responses(&[("cancel", "Cancel"), ("ok", action)]);
    dialog.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("ok"));
    dialog.connect_response(Some("ok"), move |_, _| {
        let name = entry.text().trim().to_string();
        if !name.is_empty() && !name.contains('/') {
            done(name);
        }
    });
    dialog.present(Some(anchor));
}

fn confirm_delete(anchor: &impl IsA<gtk::Widget>, model: &Rc<Model>, item: &PhoneFile, device: &Device) {
    let dialog = adw::AlertDialog::new(Some(&format!("Delete “{}”?", item.name)), Some("It's deleted from the phone right away; this can't be undone."));
    dialog.add_responses(&[("cancel", "Cancel"), ("delete", "Delete")]);
    dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
    let (model, item, device) = (model.clone(), item.clone(), device.clone());
    dialog.connect_response(Some("delete"), move |_, _| model.delete_files(vec![item.clone()], &device));
    dialog.present(Some(anchor));
}
