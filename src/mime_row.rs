use crate::entry::MimeEntry;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::{Cell, RefCell};

mod imp {
    use super::*;

    /// A list row: type icon, description, MIME name, and the current default app.
    /// Plain widgets rather than `AdwActionRow` because a `GtkListBoxRow` outside a
    /// `GtkListBox` hits `gtk_list_box_row_grab_focus: assertion 'box != NULL'`.
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[template(resource = "/io/github/sachesi/mimebind/ui/mime_row.ui")]
    #[properties(wrapper_type = super::MimeRow)]
    pub struct MimeRow {
        #[template_child]
        pub type_icon: TemplateChild<gtk::Image>,
        #[template_child]
        pub description_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub mime_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub app_icon: TemplateChild<gtk::Image>,
        #[template_child]
        pub app_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub reset_slot: TemplateChild<gtk::Stack>,
        #[template_child]
        pub reset_button: TemplateChild<gtk::Button>,
        /// Hides the application name, for windows too narrow for two columns.
        #[property(get, set)]
        pub narrow: Cell<bool>,
        /// The entry shown, and the handler that follows its changes.
        pub bound: RefCell<Option<(MimeEntry, glib::SignalHandlerId)>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MimeRow {
        const NAME: &'static str = "MimebindMimeRow";
        type Type = super::MimeRow;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for MimeRow {}
    impl WidgetImpl for MimeRow {}
    impl BoxImpl for MimeRow {}
}

glib::wrapper! {
    pub struct MimeRow(ObjectSubclass<imp::MimeRow>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl MimeRow {
    pub(crate) fn new() -> Self {
        glib::Object::new()
    }

    /// Show `entry`, and follow its changes until `unbind`.
    pub(crate) fn bind(&self, entry: &MimeEntry) {
        self.unbind();
        self.show(entry);
        let handler = entry.connect_changed(glib::clone!(
            #[weak(rename_to = row)]
            self,
            move |entry| row.show(entry)
        ));
        self.imp().bound.replace(Some((entry.clone(), handler)));
    }

    pub(crate) fn unbind(&self) {
        if let Some((entry, handler)) = self.imp().bound.take() {
            entry.disconnect(handler);
        }
    }

    /// Rows are recycled, so everything a previous entry set is set again.
    fn show(&self, entry: &MimeEntry) {
        let imp = self.imp();
        let mime = entry.mime();

        imp.type_icon
            .set_from_gicon(&gio::functions::content_type_get_icon(&mime));
        imp.description_label.set_label(&entry.description());
        imp.mime_label.set_label(&mime);

        let modified = entry.modified();
        let reset = &imp.reset_button;
        // The target first: an action name without one is a type mismatch GTK warns about.
        reset.set_action_target_value(Some(&mime.to_variant()));
        reset.set_action_name(Some("win.reset-type"));
        imp.reset_slot
            .set_visible_child_name(if modified { "reset" } else { "unchanged" });

        let default_app = gio::AppInfo::default_for_type(&mime, false);
        match default_app.as_ref().and_then(|app| app.icon()) {
            Some(icon) => imp.app_icon.set_from_gicon(&icon),
            None => imp.app_icon.clear(),
        }
        let name = default_app.map(|app| app.display_name().to_string());
        imp.app_label
            .set_label(name.as_deref().unwrap_or(&gettext("Not set")));

        let label = match &name {
            // Translators: what a screen reader says for a row of the list, e.g.
            // "PNG image, image/png, opens with Image Viewer".
            Some(app) => gettext("{description}, {mime}, opens with {app}").replace("{app}", app),
            // Translators: what a screen reader says for a row of a type nothing opens.
            None => gettext("{description}, {mime}, no default application"),
        };
        self.update_property(&[gtk::accessible::Property::Label(
            &label
                .replace("{description}", &entry.description())
                .replace("{mime}", &mime),
        )]);
    }
}
