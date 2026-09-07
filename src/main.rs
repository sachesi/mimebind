mod catalog;
mod category;
mod dialogs;
mod entry;
mod i18n;
mod rows;
mod window;

use gtk::glib;
use gtk::prelude::*;
use window::build_window;

const APP_ID: &str = "io.github.sachesi.mimebind";

fn main() -> glib::ExitCode {
    i18n::init();
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_window);
    app.set_accels_for_action("window.close", &["<Control>w", "<Control>q"]);
    app.run()
}
