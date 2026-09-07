use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::RefCell;
use std::collections::HashSet;

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
    impl ObjectImpl for MimeEntry {}
}

glib::wrapper! {
    pub struct MimeEntry(ObjectSubclass<imp::MimeEntry>);
}

mod imp_state {
    use super::*;
    use glib::subclass::prelude::*;

    /// The one piece of layout state the breakpoint flips and rows bind to.
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::ViewState)]
    pub struct ViewState {
        #[property(get, set)]
        pub narrow: std::cell::Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ViewState {
        const NAME: &'static str = "MimebindViewState";
        type Type = super::ViewState;
    }

    #[glib::derived_properties]
    impl ObjectImpl for ViewState {}
}

glib::wrapper! {
    pub struct ViewState(ObjectSubclass<imp_state::ViewState>);
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
}

/// The section a type belongs to: its top-level media type.
pub(crate) fn media_group(mime: &str) -> String {
    let media = mime.split_once('/').map_or(mime, |(media, _)| media);
    match media {
        "application" => "Applications and Documents".into(),
        "audio" => "Audio".into(),
        "font" => "Fonts".into(),
        "image" => "Images".into(),
        "inode" => "Folders and Devices".into(),
        "message" => "Messages".into(),
        "model" => "3D Models".into(),
        "multipart" => "Multipart".into(),
        "text" => "Text".into(),
        "video" => "Video".into(),
        "x-scheme-handler" => "Links and Protocols".into(),
        other => {
            let mut characters = other.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().to_string() + characters.as_str(),
                None => "Other".into(),
            }
        }
    }
}
