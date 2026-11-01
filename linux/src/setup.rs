//! First-run setup: get scrcpy, pick an adb, connect a phone.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use crate::dialogs;
use crate::model::{off_main, Model};

pub struct SetupView {
    carousel: adw::Carousel,
    root: gtk::Box,
}

impl SetupView {
    pub fn new(model: Rc<Model>) -> Rc<SetupView> {
        let carousel = adw::Carousel::builder().interactive(false).vexpand(true).build();
        let dots = adw::CarouselIndicatorLines::builder().carousel(&carousel).build();
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let header = adw::HeaderBar::builder().show_title(false).build();
        root.append(&header);
        root.append(&carousel);
        root.append(&dots);
        root.set_margin_bottom(24);
        let view = Rc::new(SetupView { carousel, root });

        // 1. scrcpy
        let status = gtk::Label::builder().wrap(true).justify(gtk::Justification::Center).build();
        let spinner = gtk::Spinner::new();
        let retry = gtk::Button::builder().label("Try Again").halign(gtk::Align::Center).visible(false).build();
        let own = gtk::Button::builder().label("Use My Own scrcpy…").build();
        let next = suggested("Continue");
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        body.append(&spinner);
        body.append(&status);
        body.append(&retry);
        let scrcpy_page = step(
            "video-display-symbolic",
            "Get scrcpy",
            "Tether uses scrcpy to show your phone's screen. It's free and open source.",
            &body,
            &own,
            &next,
        );
        let ready = {
            let (model, status, spinner, retry, next) = (model.clone(), status.clone(), spinner.clone(), retry.clone(), next.clone());
            move || {
                spinner.stop();
                retry.set_visible(false);
                let custom = model.core.config().scrcpy;
                status.set_label(&if custom.is_empty() {
                    format!("✓ scrcpy {} is ready.", model.core.bundled_scrcpy_version().unwrap_or_default())
                } else {
                    format!("✓ Using {custom}")
                });
                status.remove_css_class("error");
                status.add_css_class("success");
                next.set_sensitive(true);
            }
        };
        let download = {
            let (model, status, spinner, retry, next, ready) = (model.clone(), status.clone(), spinner.clone(), retry.clone(), next.clone(), ready.clone());
            move || {
                let (model, status, spinner, retry, next, ready) = (model.clone(), status.clone(), spinner.clone(), retry.clone(), next.clone(), ready.clone());
                glib::spawn_future_local(async move {
                    next.set_sensitive(false);
                    retry.set_visible(false);
                    spinner.start();
                    status.set_label("Downloading the latest scrcpy…");
                    status.remove_css_class("error");
                    let core = model.core.clone();
                    let result = off_main(move || {
                        let release = core.latest_release()?.ok_or_else(|| {
                            tether_core::CoreError::Failed("There's no prebuilt scrcpy for this computer. Install it (e.g. from your package manager) and choose it below.".into())
                        })?;
                        core.install_scrcpy(release)
                    })
                    .await;
                    match result {
                        Ok(()) => {
                            model.update_config(|c| c.scrcpy.clear());
                            ready();
                        }
                        Err(e) => {
                            spinner.stop();
                            status.set_label(&e.to_string());
                            status.add_css_class("error");
                            retry.set_visible(true);
                        }
                    }
                });
            }
        };
        {
            let download = download.clone();
            retry.connect_clicked(move |_| download());
        }
        {
            let (model, ready) = (model.clone(), ready.clone());
            own.connect_clicked(move |button| {
                let (model, ready, window) = (model.clone(), ready.clone(), button.root().and_downcast::<gtk::Window>());
                glib::spawn_future_local(async move {
                    let dialog = gtk::FileDialog::builder().title("Choose scrcpy").build();
                    if let Some(path) = dialog.open_future(window.as_ref()).await.ok().and_then(|f| f.path()) {
                        model.update_config(|c| c.scrcpy = path.to_string_lossy().into());
                        ready();
                    }
                });
            });
        }

        // 2. adb
        let back = gtk::Button::with_label("Back");
        let next2 = suggested("Continue");
        let adb_page = step(
            "drive-removable-media-symbolic",
            "Choose adb",
            "adb is how your computer talks to the phone. If you use Android Studio, sharing its adb avoids conflicts.",
            &dialogs::adb_picker(&model),
            &back,
            &next2,
        );

        // 3. connect
        let steps = gtk::Label::builder()
            .label("1. On the phone, open Settings → About phone and tap Build number 7 times.\n2. Go to Settings → System → Developer options and turn on USB debugging.\n3. Plug the phone in and tap Allow on the “Allow USB debugging?” prompt.")
            .wrap(true)
            .xalign(0.0)
            .build();
        let connect_status = gtk::Label::builder().css_classes(["heading"]).build();
        let body3 = gtk::Box::new(gtk::Orientation::Vertical, 16);
        body3.append(&steps);
        body3.append(&connect_status);
        let back3 = gtk::Button::with_label("Back");
        let finish = suggested("Finish");
        let connect_page = step("phone-symbolic", "Connect Your Phone", "You can skip this and connect, or pair over Wi-Fi, later.", &body3, &back3, &finish);
        {
            let (m, connect_status) = (model.clone(), connect_status.clone());
            let render = move || {
                let devices = m.devices.borrow();
                connect_status.set_label(&match (devices.iter().find(|d| d.state == "device"), devices.first()) {
                    (Some(ready), _) => format!("✓ {} is connected", ready.label),
                    (None, Some(found)) => format!("Found {}. Tap Allow on the phone.", found.label),
                    _ => "Waiting for a device…".into(),
                });
            };
            render();
            model.connect_changed(render);
        }

        for page in [&scrcpy_page, &adb_page, &connect_page] {
            view.carousel.append(page);
        }
        let go = |view: &Rc<SetupView>, index: u32| {
            let view = view.clone();
            move |_: &gtk::Button| view.carousel.scroll_to(&view.carousel.nth_page(index), true)
        };
        next.connect_clicked(go(&view, 1));
        back.connect_clicked(go(&view, 0));
        next2.connect_clicked(go(&view, 2));
        back3.connect_clicked(go(&view, 1));
        {
            let model = model.clone();
            finish.connect_clicked(move |_| model.finish_setup());
        }

        // Starts over each time setup is shown.
        {
            let (view2, model) = (view.clone(), model.clone());
            view.root.connect_map(move |_| {
                view2.carousel.scroll_to(&view2.carousel.nth_page(0), false);
                if model.core.bundled_scrcpy_version().is_some() || !model.core.config().scrcpy.is_empty() {
                    ready();
                } else {
                    download();
                }
            });
        }
        view
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }
}

fn suggested(label: &str) -> gtk::Button {
    gtk::Button::builder().label(label).css_classes(["suggested-action"]).build()
}

fn step(icon: &str, title: &str, subtitle: &str, body: &impl IsA<gtk::Widget>, left: &gtk::Button, right: &gtk::Button) -> gtk::Widget {
    let page = adw::StatusPage::builder().icon_name(icon).title(title).description(subtitle).hexpand(true).build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 24);
    content.append(body);
    let buttons = gtk::CenterBox::new();
    buttons.set_start_widget(Some(left));
    buttons.set_end_widget(Some(right));
    content.append(&buttons);
    page.set_child(Some(&adw::Clamp::builder().maximum_size(520).child(&content).build()));
    page.upcast()
}
