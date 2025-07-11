use std::{cmp::Ordering, collections::HashMap};

use config::Config;
use gtk::{
    gdk::Display,
    gio::ApplicationFlags,
    glib,
    prelude::{ApplicationExt, ApplicationExtManual, BoxExt, FixedExt, GtkWindowExt, WidgetExt},
    Application, CssProvider, EventControllerKey,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use hyprland::{
    data::{Client, Clients, FullscreenMode},
    dispatch::{CycleDirection, Dispatch, DispatchType, FullscreenType, WindowIdentifier},
    keyword::Keyword,
    shared::{Address, HyprData, HyprDataActiveOptional},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
struct Window {
    pub workspace: i32,
    pub title: String,
    pub grouped: Vec<Box<Address>>,
    pub address: Address,
    pub position: (i32, i32),
    pub size: (i32, i32),
    pub is_current: bool,
}

impl From<Client> for Window {
    fn from(client: Client) -> Self {
        Self {
            workspace: client.workspace.id,
            title: client.title,
            grouped: client.grouped.clone(),
            address: client.address,
            position: ((client.at.0) as i32, (client.at.1) as i32),
            size: (client.size.0 as i32, client.size.1 as i32),
            is_current: client.focus_history_id == 0,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct AppConfig {
    labels: String,
    cycle_before: usize,
    label_position: Position,
    box_size: i32,
    ignore_current: bool,
    dim_inactive: bool,
    workspace_label_width: i32,
    ignore_workspace: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum Position {
    TopCenter,
    BottomCenter,
    TopLeft,
    BottomLeft,
    TopRight,
    BottomRight,
    Center,
}

fn main() {
    let app = Application::builder()
        .application_id("dev.benz.hyprland-easyfocus")
        .build();

    let has_setup = std::cell::Cell::new(false);
    let config = std::cell::RefCell::new(None);
    let window = std::cell::RefCell::new(None);
    let hold_guard = std::cell::RefCell::new(None);

    app.connect_activate(move |app| {
        if !has_setup.get() {
            setup_css();
            *config.borrow_mut() = Some(setup_config());
            *window.borrow_mut() = Some(setup_window(app));
            has_setup.set(true);
        }

        setup_ui(
            window.borrow().as_ref().unwrap(),
            config.borrow().as_ref().unwrap(),
        );
    });

    app.connect_startup(move |app| {
        if app.flags().contains(ApplicationFlags::IS_SERVICE) {
            *hold_guard.borrow_mut() = Some(app.hold());
        }
    });

    app.run();
}

fn get_windows(ignore_workspace: bool) -> (Vec<Window>, i32) {
    let workspace = match Client::get_active() {
        Ok(option) => match option {
            Some(active) => active.workspace.id,
            None => -1,
        },
        Err(_) => -1,
    };

    let clients = Clients::get().unwrap();
    let iter = clients.into_iter();

    if ignore_workspace {
        let mut windows = iter.map(Window::from).collect::<Vec<Window>>();
        windows.sort_by(
            |a, b| match (a.workspace == workspace, b.workspace == workspace) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                (false, false) => a.workspace.cmp(&b.workspace),
            },
        );

        return (windows, workspace);
    }

    (
        iter.filter(|w| w.workspace.id == workspace)
            .map(Window::from)
            .collect(),
        workspace,
    )
}

fn focus_window(win_add: &Address) {
    Dispatch::call(DispatchType::FocusWindow(WindowIdentifier::Address(
        win_add.clone(),
    )))
    .expect("failed to focus window");
}

fn handle_keypress(key_to_window_id: &HashMap<char, Address>, key: &str, labels: String) -> bool {
    if labels.contains(key) {
        let c = key.chars().next().unwrap();

        if key_to_window_id.contains_key(&c) {
            focus_window(&key_to_window_id[&c]);
        } else {
            return false;
        }

        return true;
    } else {
        if key == "Escape" {
            return true;
        }
    }

    return false;
}

fn setup_config() -> AppConfig {
    let mut config_file = dirs::config_dir().unwrap();
    config_file.push("hyprland-easyfocus");
    config_file.push("config.json");

    let mut config_builder = Config::builder().add_source(config::File::from_str(
        include_str!("config.json"),
        config::FileFormat::Json,
    ));

    if config_file.exists() {
        config_builder =
            config_builder.add_source(config::File::with_name(config_file.to_str().unwrap()));
    };

    let config: AppConfig = config_builder
        .build()
        .unwrap()
        .try_deserialize::<AppConfig>()
        .unwrap();

    config
}

fn setup_ui(win: &gtk::ApplicationWindow, config: &AppConfig) {
    let dim_inactive_initial = Keyword::get("decoration:dim_inactive").unwrap().value;

    if config.dim_inactive {
        Keyword::set("decoration:dim_inactive", 1).unwrap();
    }

    let (windows, active_workspace) = get_windows(config.ignore_workspace);

    if windows.is_empty() {
        eprintln!("No windows found");
        Keyword::set("decoration:dim_inactive", dim_inactive_initial.clone()).unwrap();
        return;
    }

    let has_on_other_workspace = windows.iter().any(|w| w.workspace != active_workspace);

    if windows.len() < config.cycle_before && !has_on_other_workspace {
        Dispatch::call(DispatchType::CycleWindow(CycleDirection::Next))
            .expect("failed to focus window");

        Keyword::set("decoration:dim_inactive", dim_inactive_initial.clone()).unwrap();

        return;
    }

    let fullscreen_mode = match Client::get_active() {
        Ok(option) => match option {
            Some(active) => {
                if active.fullscreen != FullscreenMode::None {
                    Dispatch::call(DispatchType::ToggleFullscreen(FullscreenType::NoParam))
                        .expect("failed to toggle fullscreen");

                    Some(active.fullscreen)
                } else {
                    None
                }
            }
            None => None,
        },
        Err(_) => None,
    };

    let mut chars = config.labels.chars();

    let fixed = gtk::Fixed::new();
    let workspace_wrapper = gtk::Box::new(gtk::Orientation::Vertical, 0);

    if config.ignore_workspace {
        workspace_wrapper.add_css_class("workspaces");
        fixed.put(&workspace_wrapper, 0.0, 0.0);
    }

    let mut assignments = HashMap::new();

    windows.iter().for_each(|win| {
        if config.ignore_current && win.is_current {
            return;
        }

        let mut char = chars.next().unwrap();

        assignments.insert(char, win.address.clone());

        if char.is_alphabetic() {
            char = char.to_uppercase().next().unwrap();
        }

        if win.workspace != active_workspace {
            let label_label = gtk::Label::new(Some(char.to_string().as_str()));
            label_label.add_css_class("label");
            label_label.set_xalign(0.5);
            label_label.set_size_request(config.workspace_label_width, -1);

            let title_label = gtk::Label::new(Some(win.title.as_str()));
            title_label.add_css_class("title");
            title_label.set_hexpand(true);
            title_label.set_hexpand_set(true);
            title_label.set_xalign(0.0);

            let wrapper = gtk::Box::new(gtk::Orientation::Horizontal, 0);

            wrapper.append(&label_label);
            wrapper.append(&title_label);

            workspace_wrapper.append(&wrapper);
        } else {
            let label = gtk::Label::new(Some(char.to_string().as_str()));
            label.set_hexpand(true);
            label.set_vexpand(true);
            label.set_hexpand_set(true);
            label.set_vexpand_set(true);
            label.set_halign(gtk::Align::Center);
            label.set_valign(gtk::Align::Center);
            label.set_single_line_mode(true);

            let wrapper = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            wrapper.set_size_request(config.box_size, config.box_size);
            wrapper.append(&label);

            let box_offset_half = f64::from(config.box_size / 2);

            let mut position = match config.label_position {
                Position::TopCenter => (
                    f64::from(win.position.0 + win.size.0 / 2) - box_offset_half,
                    f64::from(win.position.1),
                ),
                Position::BottomCenter => (
                    f64::from(win.position.0 + win.size.0 / 2) - box_offset_half,
                    f64::from(win.position.1 - config.box_size + win.size.1),
                ),
                Position::TopLeft => (f64::from(win.position.0), f64::from(win.position.1)),
                Position::BottomLeft => (
                    f64::from(win.position.0),
                    f64::from(win.position.1 + win.size.1 - config.box_size),
                ),
                Position::TopRight => (
                    f64::from(win.position.0 + win.size.0 - config.box_size),
                    f64::from(win.position.1),
                ),
                Position::BottomRight => (
                    f64::from(win.position.0 + win.size.0 - config.box_size),
                    f64::from(win.position.1 + win.size.1 - config.box_size),
                ),
                Position::Center => (
                    f64::from(win.position.0 + win.size.0 / 2) - box_offset_half,
                    f64::from(win.position.1 + win.size.1 / 2) - box_offset_half,
                ),
            };

            if win.grouped.len() > 1 {
                let index = win
                    .grouped
                    .iter()
                    .position(|x| x.as_ref() == &win.address)
                    .unwrap();

                let window_base = win.size.0 / win.grouped.len() as i32;

                position.0 = match config.label_position {
                    Position::TopCenter | Position::Center | Position::BottomCenter => {
                        f64::from(win.position.0 + (index as i32) * window_base + window_base / 2)
                    }
                    Position::TopLeft | Position::BottomLeft => {
                        f64::from(win.position.0 + (index as i32) * window_base)
                    }
                    Position::TopRight | Position::BottomRight => f64::from(
                        win.position.0 + window_base - config.box_size
                            + (index as i32) * window_base,
                    ),
                };
            }

            fixed.put(&wrapper, position.0, position.1);

            if win.is_current {
                wrapper.add_css_class("current");
            }
        }
    });

    win.focus();

    let key_controller: EventControllerKey = EventControllerKey::new();
    let key_controller_copy = key_controller.clone();
    let win_copy = win.clone();

    let config_clone = config.clone();

    key_controller.connect_key_pressed(move |_, key, _, _| {
        let success = handle_keypress(
            &assignments,
            &key.name().unwrap(),
            config_clone.labels.clone(),
        );

        if success {
            Keyword::set("decoration:dim_inactive", dim_inactive_initial.clone()).unwrap();

            if fullscreen_mode.is_some() {
                let fullscreen_type = match fullscreen_mode {
                    Some(FullscreenMode::Fullscreen) => FullscreenType::Real,
                    Some(FullscreenMode::Maximized) => FullscreenType::Maximize,
                    Some(FullscreenMode::MaximizedFullscreen) => FullscreenType::NoParam,
                    _ => FullscreenType::NoParam,
                };

                Dispatch::call(DispatchType::ToggleFullscreen(fullscreen_type))
                    .expect("failed to toggle fullscreen");
            }

            if !win_copy
                .application()
                .expect("should have app")
                .flags()
                .contains(ApplicationFlags::IS_SERVICE)
            {
                win_copy.close();
            } else {
                win_copy.remove_controller(&key_controller_copy);
                win_copy.set_visible(false);
            }
        }

        return glib::Propagation::Proceed;
    });

    win.add_controller(key_controller);

    win.set_child(Some(&fixed));
    win.present();
}

fn setup_css() {
    let css_provider = CssProvider::new();

    let mut style_file = dirs::config_dir().unwrap();
    style_file.push("hyprland-easyfocus");
    style_file.push("style.css");

    if style_file.exists() {
        css_provider.load_from_path(style_file.to_str().unwrap());
    } else {
        css_provider.load_from_string(include_str!("style.css"));
    }

    gtk::style_context_add_provider_for_display(
        &Display::default().expect("Could not connect to a display."),
        &css_provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn setup_window(app: &Application) -> gtk::ApplicationWindow {
    let win = gtk::ApplicationWindow::new(app);

    win.init_layer_shell();
    win.set_namespace("hyprland-easyfocus");
    win.set_exclusive_zone(-1);
    win.set_layer(Layer::Overlay);
    win.set_keyboard_mode(KeyboardMode::OnDemand);

    win.set_anchor(Edge::Left, true);
    win.set_anchor(Edge::Right, true);
    win.set_anchor(Edge::Top, true);
    win.set_anchor(Edge::Bottom, true);

    return win;
}
