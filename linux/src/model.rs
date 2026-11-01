//! App state. Core calls block (adb, scrcpy, downloads), so they run on GLib's thread pool via
//! `off_main`; everything else happens on the main thread. The window redraws on `changed`.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gio, glib};
use tether_core::*;

/// Runs blocking work off the main thread.
pub async fn off_main<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    gio::spawn_blocking(work).await.expect("background work panicked")
}

pub struct Model {
    pub core: Arc<Core>,
    pub config: RefCell<Config>,
    pub needs_setup: Cell<bool>,
    pub devices: RefCell<Vec<Device>>,
    pub selection: RefCell<Option<String>>,
    pub info: RefCell<HashMap<String, DeviceInfo>>,
    pub apps: RefCell<HashMap<String, Rc<Vec<DeviceApp>>>>,
    pub loading_apps: RefCell<HashSet<String>>,
    pub app_errors: RefCell<HashMap<String, String>>,
    pub notifications: RefCell<HashMap<String, Vec<PhoneNotification>>>,
    /// Tether Buddy's state per phone.
    pub buddy: RefCell<HashMap<String, BuddyState>>,
    pub installing_buddy: RefCell<HashSet<String>>,
    pub installing: RefCell<HashSet<String>>,
    pub update: RefCell<Option<Release>>,
    pub installing_update: Cell<bool>,
    /// The usable screen area app windows open on.
    pub screen: Cell<Screen>,
    /// The selected serial and state last loaded: a device is (re)loaded when selected or authorized.
    loaded: RefCell<Option<String>>,
    on_change: RefCell<Vec<Box<dyn Fn()>>>,
    on_alert: RefCell<Option<Box<dyn Fn(&str)>>>,
    on_toast: RefCell<Option<Box<dyn Fn(&str, Option<PathBuf>)>>>,
}

impl Model {
    pub fn new() -> Rc<Model> {
        let dir = glib::user_data_dir().join("tether");
        let core = Core::new(dir.to_string_lossy().into());
        Rc::new(Model {
            config: RefCell::new(core.config()),
            needs_setup: Cell::new(core.needs_setup()),
            core,
            devices: Default::default(),
            selection: Default::default(),
            info: Default::default(),
            apps: Default::default(),
            loading_apps: Default::default(),
            app_errors: Default::default(),
            notifications: Default::default(),
            buddy: Default::default(),
            installing_buddy: Default::default(),
            installing: Default::default(),
            update: Default::default(),
            installing_update: Cell::new(false),
            screen: Cell::new(Screen { width: 1280, height: 800, scale: 1.0 }),
            loaded: Default::default(),
            on_change: Default::default(),
            on_alert: Default::default(),
            on_toast: Default::default(),
        })
    }

    pub fn connect_changed(&self, f: impl Fn() + 'static) {
        self.on_change.borrow_mut().push(Box::new(f));
    }

    pub fn connect_alert(&self, f: impl Fn(&str) + 'static) {
        *self.on_alert.borrow_mut() = Some(Box::new(f));
    }

    /// A short message, optionally with a file to open.
    pub fn connect_toast(&self, f: impl Fn(&str, Option<PathBuf>) + 'static) {
        *self.on_toast.borrow_mut() = Some(Box::new(f));
    }

    pub fn changed(&self) {
        for f in self.on_change.borrow().iter() {
            f();
        }
    }

    pub fn alert(&self, message: &str) {
        if let Some(f) = &*self.on_alert.borrow() {
            f(message);
        }
    }

    fn toast(&self, message: &str, file: Option<PathBuf>) {
        if let Some(f) = &*self.on_toast.borrow() {
            f(message, file);
        }
    }

    pub fn selected_device(&self) -> Option<Device> {
        let selection = self.selection.borrow();
        self.devices.borrow().iter().find(|d| Some(&d.serial) == selection.as_ref()).cloned()
    }

    pub fn name(&self, device: &Device) -> String {
        self.info.borrow().get(&device.serial).map_or(device.label.clone(), |info| info.name.clone())
    }

    /// Starts watching for devices and checks for a scrcpy update.
    pub fn start(self: &Rc<Self>) {
        let model = self.clone();
        glib::spawn_future_local(async move {
            model.check_for_update();
            for tick in 1u64.. {
                model.refresh_devices().await;
                // Keep notifications, battery, memory etc. current for the device on screen.
                if tick % 3 == 0 {
                    if let Some(serial) = model.selection.borrow().clone() {
                        model.load_notifications(&serial).await;
                    }
                }
                if tick % 15 == 0 {
                    if let Some(serial) = model.selection.borrow().clone() {
                        model.load_info(&serial).await;
                    }
                }
                glib::timeout_future(Duration::from_secs(2)).await;
            }
        });
    }

    pub async fn refresh_devices(self: &Rc<Self>) {
        let core = self.core.clone();
        let Ok(devices) = off_main(move || core.devices()).await else { return };
        let mut changed = devices.len() != self.devices.borrow().len()
            || devices.iter().zip(self.devices.borrow().iter()).any(|(a, b)| !same_device(a, b));
        *self.devices.borrow_mut() = devices;
        let selection_gone = match &*self.selection.borrow() {
            Some(serial) => !self.devices.borrow().iter().any(|d| &d.serial == serial),
            None => true,
        };
        if selection_gone {
            *self.selection.borrow_mut() = self.devices.borrow().first().map(|d| d.serial.clone());
            changed = true;
        }
        if changed {
            self.changed();
        }
        self.load_selection();
    }

    pub fn select(self: &Rc<Self>, serial: Option<String>) {
        if *self.selection.borrow() == serial {
            return;
        }
        *self.selection.borrow_mut() = serial;
        self.changed();
        self.load_selection();
    }

    fn load_selection(self: &Rc<Self>) {
        let key = self.selected_device().map(|d| format!("{}{}", d.serial, d.state));
        if *self.loaded.borrow() == key {
            return;
        }
        *self.loaded.borrow_mut() = key;
        if let Some(device) = self.selected_device().filter(|d| d.state == "device") {
            let model = self.clone();
            glib::spawn_future_local(async move {
                model.load_info(&device.serial).await;
                model.load_apps(&device.serial, false).await;
                model.load_notifications(&device.serial).await; // after the apps, which give them names and icons
                model.check_buddy(&device.serial).await;
            });
        }
    }

    /// Reloads info, and the app list from the device rather than the cache.
    pub fn refresh(self: &Rc<Self>) {
        let Some(serial) = self.selection.borrow().clone() else { return };
        let model = self.clone();
        glib::spawn_future_local(async move {
            model.load_info(&serial).await;
            model.load_apps(&serial, true).await;
        });
    }

    pub async fn load_info(&self, serial: &str) {
        let (core, serial) = (self.core.clone(), serial.to_string());
        if let Ok(info) = off_main({
            let serial = serial.clone();
            move || core.device_info(serial)
        })
        .await
        {
            self.info.borrow_mut().insert(serial, info);
            self.changed();
        }
    }

    async fn load_notifications(&self, serial: &str) {
        if !self.devices.borrow().iter().any(|d| d.serial == serial && d.state == "device") {
            return;
        }
        let (core, owned) = (self.core.clone(), serial.to_string());
        if let Ok(list) = off_main(move || core.notifications(owned)).await {
            if self.notifications.borrow().get(serial) != Some(&list) {
                self.notifications.borrow_mut().insert(serial.into(), list);
                self.changed();
            }
        }
    }

    /// Opens a notification on the phone's screen, mirrored: with Tether Buddy its action runs (e.g.
    /// that chat opens); without, the notification shade is pulled down to tap it.
    pub fn show_notification(self: &Rc<Self>, notification: &PhoneNotification, device: &Device) {
        let (core, serial, key, model, device) = (self.core.clone(), device.serial.clone(), notification.key.clone(), self.clone(), device.clone());
        glib::spawn_future_local(async move {
            let _ = off_main(move || core.open_notification(serial, key)).await;
            model.mirror(&device);
        });
    }

    async fn check_buddy(&self, serial: &str) {
        let (core, owned) = (self.core.clone(), serial.to_string());
        if let Ok(state) = off_main(move || core.buddy_state(owned)).await {
            if self.buddy.borrow().get(serial) != Some(&state) {
                self.buddy.borrow_mut().insert(serial.into(), state);
                self.changed();
            }
        }
    }

    pub fn install_buddy(self: &Rc<Self>, device: &Device) {
        let (core, serial, model) = (self.core.clone(), device.serial.clone(), self.clone());
        self.installing_buddy.borrow_mut().insert(serial.clone());
        self.changed();
        glib::spawn_future_local(async move {
            let s = serial.clone();
            if let Err(e) = off_main(move || core.install_buddy(s)).await {
                model.alert(&format!("Couldn't install Tether Buddy: {e}\n\nOn Xiaomi phones, turn on Developer options → Install via USB first."));
            }
            model.installing_buddy.borrow_mut().remove(&serial);
            model.check_buddy(&serial).await;
            model.changed();
        });
    }

    /// Opens the notification's app: in its own window if the phone supports that, else mirrored.
    pub fn open_notification_app(self: &Rc<Self>, notification: &PhoneNotification, device: &Device) {
        let supported = self.info.borrow().get(&device.serial).is_some_and(|i| i.sdk >= virtual_display_min_sdk());
        if !supported {
            return self.mirror(device);
        }
        let app = self.apps.borrow().get(&device.serial).and_then(|apps| apps.iter().find(|a| a.package == notification.package).cloned());
        let app = app.unwrap_or(DeviceApp { name: notification.app_name.clone(), package: notification.package.clone(), system: false, icon_path: None });
        self.open(&app, device);
    }

    async fn load_apps(&self, serial: &str, force: bool) {
        if self.loading_apps.borrow().contains(serial) || (!force && self.apps.borrow().contains_key(serial)) {
            return;
        }
        self.loading_apps.borrow_mut().insert(serial.into());
        self.app_errors.borrow_mut().remove(serial);
        self.changed();
        let (core, owned) = (self.core.clone(), serial.to_string());
        match off_main(move || core.apps(owned, force)).await {
            Ok(apps) => {
                self.apps.borrow_mut().insert(serial.into(), Rc::new(apps));
            }
            Err(e) => {
                self.app_errors.borrow_mut().insert(serial.into(), e.to_string());
            }
        }
        self.loading_apps.borrow_mut().remove(serial);
        self.changed();
    }

    // --- Actions -------------------------------------------------------------

    pub fn mirror(self: &Rc<Self>, device: &Device) {
        let (serial, title) = (device.serial.clone(), self.name(device));
        self.launch(move |core| core.mirror(serial, title));
    }

    pub fn open(self: &Rc<Self>, app: &DeviceApp, device: &Device) {
        let (serial, package) = (device.serial.clone(), app.package.clone());
        let title = format!("{} · {}", app.name, self.name(device));
        let screen = self.screen.get();
        self.launch(move |core| core.open_app(serial, package, title, screen));
    }

    /// Puts this computer's clipboard text on the phone's clipboard.
    pub fn send_clipboard(self: &Rc<Self>, device: &Device) {
        let (model, device) = (self.clone(), device.clone());
        glib::spawn_future_local(async move {
            let clipboard = gtk::gdk::Display::default().unwrap().clipboard();
            let text = clipboard.read_text_future().await.ok().flatten().map(|t| t.to_string()).unwrap_or_default();
            if text.is_empty() {
                return model.toast("The clipboard has no text.", None);
            }
            let (core, serial, count) = (model.core.clone(), device.serial.clone(), text.chars().count());
            match off_main(move || core.set_phone_clipboard(serial, text)).await {
                Ok(()) => model.toast(&format!("Sent {count} characters to the phone"), None),
                Err(e) => model.alert(&e.to_string()),
            }
        });
    }

    /// Copies the phone's clipboard text to this computer's clipboard.
    pub fn copy_clipboard(self: &Rc<Self>, device: &Device) {
        let (core, serial, model) = (self.core.clone(), device.serial.clone(), self.clone());
        glib::spawn_future_local(async move {
            match off_main(move || core.phone_clipboard(serial)).await {
                Ok(Some(text)) => {
                    gtk::gdk::Display::default().unwrap().clipboard().set_text(&text);
                    model.toast(&format!("Copied {} characters from the phone", text.chars().count()), None);
                }
                Ok(None) => model.alert("Nothing to copy: the phone's clipboard is empty, or the phone doesn't let it be read (Xiaomi phones don't)."),
                Err(e) => model.alert(&e.to_string()),
            }
        });
    }

    pub fn go_wireless(self: &Rc<Self>, device: &Device) {
        let serial = device.serial.clone();
        self.run(move |core| core.go_wireless(serial).map(|_| ()));
    }

    /// Pairs (if a code is given) and/or connects over Wi-Fi.
    pub async fn pair(self: &Rc<Self>, address: String, code: String, connect: String) -> Result<(), String> {
        let core = self.core.clone();
        off_main(move || {
            if !address.is_empty() && !code.is_empty() {
                core.pair(address, code)?;
            }
            if !connect.is_empty() {
                core.connect(connect)?;
            }
            Ok::<_, CoreError>(())
        })
        .await
        .map_err(|e| e.to_string())?;
        self.refresh_devices().await;
        Ok(())
    }

    /// Saves a screenshot to Pictures/Screenshots.
    pub fn screenshot(self: &Rc<Self>, device: &Device) {
        let dir = glib::user_special_dir(glib::UserDirectory::Pictures).unwrap_or_else(glib::home_dir).join("Screenshots");
        let time = glib::DateTime::now_local().and_then(|t| t.format("%Y-%m-%d %H-%M-%S")).map(|t| t.to_string()).unwrap_or_default();
        let path = dir.join(format!("Screenshot {} {time}.png", self.name(device).replace('/', "-")));
        let (core, serial, model) = (self.core.clone(), device.serial.clone(), self.clone());
        glib::spawn_future_local(async move {
            let target = path.clone();
            let result = off_main(move || {
                std::fs::create_dir_all(target.parent().unwrap())?;
                core.screenshot(serial, target.to_string_lossy().into())
            })
            .await;
            match result {
                Ok(()) => model.toast("Screenshot saved to Pictures/Screenshots", Some(path)),
                Err(e) => model.alert(&e.to_string()),
            }
        });
    }

    pub fn install(self: &Rc<Self>, apks: Vec<PathBuf>, device: &Device) {
        let (serial, model) = (device.serial.clone(), self.clone());
        self.installing.borrow_mut().insert(serial.clone());
        self.changed();
        glib::spawn_future_local(async move {
            for apk in apks {
                let (core, s, path) = (model.core.clone(), serial.clone(), apk.to_string_lossy().to_string());
                match off_main(move || core.install_apk(s, path)).await {
                    Ok(()) => model.toast(&format!("Installed {}", apk.file_name().unwrap_or_default().to_string_lossy()), None),
                    Err(e) => model.alert(&format!("Couldn't install {}: {e}", apk.file_name().unwrap_or_default().to_string_lossy())),
                }
            }
            model.installing.borrow_mut().remove(&serial);
            model.changed();
            model.load_apps(&serial, true).await;
        });
    }

    pub fn press(self: &Rc<Self>, key: DeviceKey, device: &Device) {
        let (core, serial, model) = (self.core.clone(), device.serial.clone(), self.clone());
        glib::spawn_future_local(async move {
            let s = serial.clone();
            if let Err(e) = off_main(move || core.press_key(s, key)).await {
                model.alert(&e.to_string());
            }
            if matches!(key, DeviceKey::Power) {
                glib::timeout_future(Duration::from_millis(600)).await;
                model.load_info(&serial).await;
            }
        });
    }

    pub fn set_dark_mode(self: &Rc<Self>, on: bool, device: &Device) {
        let serial = device.serial.clone();
        self.update_device(device, move |core| core.set_dark_mode(serial, on));
    }

    pub fn set_stay_awake(self: &Rc<Self>, on: bool, device: &Device) {
        let serial = device.serial.clone();
        self.update_device(device, move |core| core.set_stay_awake(serial, on));
    }

    pub fn reboot(self: &Rc<Self>, device: &Device, mode: RebootMode) {
        let serial = device.serial.clone();
        self.run(move |core| core.reboot(serial, mode));
    }

    /// Changes a device setting, then reloads its info to show the result.
    fn update_device(self: &Rc<Self>, device: &Device, action: impl FnOnce(&Core) -> Result<(), CoreError> + Send + 'static) {
        let (core, serial, model) = (self.core.clone(), device.serial.clone(), self.clone());
        glib::spawn_future_local(async move {
            if let Err(e) = off_main(move || action(&core)).await {
                model.alert(&e.to_string());
            }
            model.load_info(&serial).await;
        });
    }

    // --- Settings ------------------------------------------------------------

    pub fn window_size(&self) -> WindowSize {
        parse_window_size(self.config.borrow().window_size.clone()).unwrap_or(WindowSize { width: 0, height: 0, dpi: 0 })
    }

    pub fn set_window_size(&self, size: WindowSize) {
        self.update_config(|c| c.window_size = format_window_size(size));
    }

    pub fn update_config(&self, change: impl FnOnce(&mut Config)) {
        let mut config = self.config.borrow().clone();
        change(&mut config);
        self.save(config);
    }

    pub fn save(&self, config: Config) {
        match self.core.save_config(config.clone()) {
            Ok(()) => {
                *self.config.borrow_mut() = config;
                self.changed();
            }
            Err(e) => self.alert(&e.to_string()),
        }
    }

    pub fn finish_setup(self: &Rc<Self>) {
        let mut config = self.core.config();
        config.setup_done = true;
        self.save(config);
        self.needs_setup.set(self.core.needs_setup());
        self.apps.borrow_mut().clear(); // adb or scrcpy may have changed
        *self.loaded.borrow_mut() = None;
        self.changed();
        self.check_for_update();
    }

    pub fn run_setup_again(&self) {
        self.needs_setup.set(true);
        self.changed();
    }

    // --- scrcpy updates ------------------------------------------------------

    pub fn check_for_update(self: &Rc<Self>) {
        let (core, model) = (self.core.clone(), self.clone());
        glib::spawn_future_local(async move {
            if let Ok(update) = off_main(move || core.available_update()).await {
                *model.update.borrow_mut() = update;
                model.changed();
            }
        });
    }

    pub fn install_update(self: &Rc<Self>) {
        let Some(release) = self.update.borrow().clone() else { return };
        self.installing_update.set(true);
        self.changed();
        let (core, model) = (self.core.clone(), self.clone());
        glib::spawn_future_local(async move {
            match off_main(move || core.install_scrcpy(release)).await {
                Ok(()) => *model.update.borrow_mut() = None,
                Err(e) => model.alert(&format!("Couldn't update scrcpy: {e}")),
            }
            model.installing_update.set(false);
            model.changed();
        });
    }

    /// Starts a scrcpy window and reports how it ended.
    fn launch(self: &Rc<Self>, start: impl FnOnce(&Core) -> Result<Arc<Session>, CoreError> + Send + 'static) {
        let (core, model) = (self.core.clone(), self.clone());
        glib::spawn_future_local(async move {
            if let Err(e) = off_main(move || start(&core)?.wait()).await {
                model.alert(&e.to_string());
            }
        });
    }

    /// Runs a core action in the background and shows its error, if any.
    fn run(self: &Rc<Self>, action: impl FnOnce(&Core) -> Result<(), CoreError> + Send + 'static) {
        let (core, model) = (self.core.clone(), self.clone());
        glib::spawn_future_local(async move {
            if let Err(e) = off_main(move || action(&core)).await {
                model.alert(&e.to_string());
            }
        });
    }
}

fn same_device(a: &Device, b: &Device) -> bool {
    a.serial == b.serial && a.state == b.state && a.model == b.model
}
