use gettextrs::LocaleCategory;

const DOMAIN: &str = "mimebind";

/// Where `just install` puts the message catalogues, fixed at build time so a
/// build for a different prefix finds its own translations.
const LOCALE_DIR: &str = match option_env!("MIMEBIND_LOCALEDIR") {
    Some(dir) => dir,
    None => "/usr/share/locale",
};

/// Point gettext at the catalogues. A failure here leaves the app in English
/// rather than stopping it, so it is reported to the GLib log and not fatal.
pub(crate) fn init() {
    // SAFETY: called once, before any thread or GLib main loop exists.
    unsafe {
        gettextrs::setlocale(LocaleCategory::LcAll, "");
    }
    if let Err(error) = gettextrs::bindtextdomain(DOMAIN, LOCALE_DIR) {
        gtk::glib::g_warning!(DOMAIN, "no translations from {LOCALE_DIR}: {error}");
        return;
    }
    if let Err(error) = gettextrs::bind_textdomain_codeset(DOMAIN, "UTF-8") {
        gtk::glib::g_warning!(DOMAIN, "cannot set the message encoding: {error}");
    }
    if let Err(error) = gettextrs::textdomain(DOMAIN) {
        gtk::glib::g_warning!(DOMAIN, "cannot select the message domain: {error}");
    }
}

/// Marks a literal for extraction without translating it here, for the tables
/// that have to stay `const`. Translate at the point of display with `gettext`.
#[allow(non_snake_case)]
pub(crate) const fn N_(text: &'static str) -> &'static str {
    text
}
