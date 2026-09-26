use gettextrs::gettext;
use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::OnceLock;

mod imp {
    use super::*;
    use glib::subclass::prelude::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::MimeEntry)]
    pub struct MimeEntry {
        #[property(get, set)]
        pub mime: RefCell<String>,
        #[property(get, set)]
        pub description: RefCell<String>,
        #[property(get, set)]
        pub search_key: RefCell<String>,
        #[property(get, set)]
        pub type_group: RefCell<String>,
        /// True when this type has an entry in the user's own mimeapps.list.
        #[property(get, set)]
        pub modified: std::cell::Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MimeEntry {
        const NAME: &'static str = "MimebindMimeEntry";
        type Type = super::MimeEntry;
    }

    #[glib::derived_properties]
    impl ObjectImpl for MimeEntry {
        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: OnceLock<Vec<glib::subclass::Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| vec![glib::subclass::Signal::builder("default-changed").build()])
        }
    }
}

glib::wrapper! {
    pub struct MimeEntry(ObjectSubclass<imp::MimeEntry>);
}

impl MimeEntry {
    pub(crate) fn new(mime: &str, overrides: &HashSet<String>) -> Self {
        let description = gio::functions::content_type_get_description(mime);
        glib::Object::builder()
            .property("mime", mime)
            .property("description", description.as_str())
            .property("search-key", format!("{mime} {description}"))
            .property("type-group", media_group(mime))
            .property("modified", overrides.contains(mime))
            .build()
    }

    /// Tell the row showing this type to read its default again, for a change
    /// that reaches it without touching the entry, such as one to a supertype.
    pub(crate) fn default_changed(&self) {
        self.emit_by_name::<()>("default-changed", &[]);
    }

    pub(crate) fn connect_default_changed(
        &self,
        callback: impl Fn(&Self) + 'static,
    ) -> glib::SignalHandlerId {
        self.connect_closure(
            "default-changed",
            false,
            glib::closure_local!(move |entry: &Self| callback(entry)),
        )
    }
}

/// The section a type belongs to: its top-level media type.
pub(crate) fn media_group(mime: &str) -> String {
    let media = mime.split_once('/').map_or(mime, |(media, _)| media);
    match media {
        "application" => gettext("Applications and Documents"),
        "audio" => gettext("Audio"),
        "font" => gettext("Fonts"),
        "image" => gettext("Images"),
        "inode" => gettext("Folders and Devices"),
        "message" => gettext("Messages"),
        "model" => gettext("3D Models"),
        "multipart" => gettext("Multipart"),
        "text" => gettext("Text"),
        "video" => gettext("Video"),
        "x-scheme-handler" => gettext("Links and Protocols"),
        other => {
            let mut characters = other.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().to_string() + characters.as_str(),
                None => gettext("Other"),
            }
        }
    }
}
