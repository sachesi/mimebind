mod catalog;
mod category;
mod dialogs;
mod entry;
mod i18n;
mod mime_row;
mod rows;
mod window;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};
use window::MimebindWindow;

const APP_ID: &str = "io.github.sachesi.mimebind";

fn main() -> glib::ExitCode {
    i18n::init();
    gio::resources_register_include!("mimebind.gresource").expect("register resources");
    glib::set_application_name("Mimebind");

    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_startup(setup_actions);
    // One window per instance: launching again brings it back to the front.
    app.connect_activate(|app| {
        let window = app
            .active_window()
            .unwrap_or_else(|| MimebindWindow::new(app).upcast());
        window.present();
    });
    app.run()
}

fn setup_actions(app: &adw::Application) {
    let quit = gio::ActionEntry::builder("quit")
        .activate(|app: &adw::Application, _, _| app.quit())
        .build();
    let about = gio::ActionEntry::builder("about")
        .activate(|app: &adw::Application, _, _| show_about(app))
        .build();
    app.add_action_entries([quit, about]);
    app.set_accels_for_action("app.quit", &["<Control>q"]);
    app.set_accels_for_action("window.close", &["<Control>w"]);
}

fn show_about(app: &adw::Application) {
    adw::AboutDialog::builder()
        .application_name("Mimebind")
        .application_icon(APP_ID)
        .version(env!("CARGO_PKG_VERSION"))
        .developer_name("sachesi")
        .license_type(gtk::License::Gpl30)
        .comments(
            gettext("Choose which application opens which file type.")
                + "\n\n"
                + &gettext("Changes are saved in {path}.")
                    .replace("{path}", &window::associations_path()),
        )
        .translator_credits(gettext("translator-credits"))
        .website("https://github.com/sachesi/mimebind")
        .issue_url("https://github.com/sachesi/mimebind/issues")
        .build()
        .present(app.active_window().as_ref());
}
