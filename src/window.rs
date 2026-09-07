use crate::catalog::{AppEntry, build_catalog, installed_mime_types, user_overrides};
use crate::category::{DEFAULT_CATEGORIES, DefaultCategory, category_default};
use crate::dialogs;
use crate::entry::MimeEntry;
use crate::mime_row::MimeRow;
use crate::rows::{fill_sidebar, header_factory};
use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};
use std::cell::{Cell, OnceCell, Ref, RefCell};
use std::collections::HashSet;

/// What the sidebar selection narrows the list to.
#[derive(Clone, Default, PartialEq)]
pub(crate) enum Selection {
    #[default]
    Defaults,
    All,
    Modified,
    /// Every type the application with this id can open.
    App(String),
    /// Every type in this media group.
    Media(String),
}

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[template(resource = "/io/github/sachesi/mimebind/ui/window.ui")]
    #[properties(wrapper_type = super::MimebindWindow)]
    pub struct MimebindWindow {
        #[template_child]
        pub toasts: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub split_view: TemplateChild<adw::NavigationSplitView>,
        #[template_child]
        pub sidebar_mode: TemplateChild<gtk::DropDown>,
        #[template_child]
        pub groups: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub content_page: TemplateChild<adw::NavigationPage>,
        #[template_child]
        pub use_all_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub reset_all_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub search_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub search_bar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub defaults_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub scroller: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub list_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub filter_model: TemplateChild<gtk::FilterListModel>,
        #[template_child]
        pub filter: TemplateChild<gtk::EveryFilter>,
        #[template_child]
        pub search_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub store: TemplateChild<gio::ListStore>,
        #[template_child]
        pub empty_page: TemplateChild<adw::StatusPage>,

        /// Set by the breakpoint; rows bind to it to drop their second column.
        #[property(get, set)]
        pub narrow: Cell<bool>,

        pub(super) catalog: OnceCell<Vec<AppEntry>>,
        /// Types with an entry in the user's own mimeapps.list.
        pub(super) overrides: RefCell<HashSet<String>>,
        pub(super) selection: RefCell<Selection>,
        /// What each sidebar row selects, by row index.
        pub(super) sidebar: RefCell<Vec<Selection>>,
        pub(super) group_filter: OnceCell<gtk::CustomFilter>,
        pub(super) headers: OnceCell<gtk::SignalListItemFactory>,
        /// Whether section headers are on, and for which application.
        pub(super) header_state: RefCell<(bool, Option<String>)>,
        pub(super) default_rows: RefCell<Vec<(adw::ActionRow, DefaultCategory)>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MimebindWindow {
        const NAME: &'static str = "MimebindWindow";
        type Type = super::MimebindWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            MimeEntry::ensure_type();
            klass.bind_template();
            klass.bind_template_callbacks();

            klass.install_action("win.assign-all", None, |win, _, _| win.assign(None));
            klass.install_action(
                "win.assign-group",
                Some(glib::VariantTy::STRING),
                |win, _, parameter| {
                    if let Some(group) = parameter.and_then(|value| value.get::<String>()) {
                        win.assign(Some(group));
                    }
                },
            );
            klass.install_action("win.reset-all", None, |win, _, _| {
                dialogs::confirm_reset_all(win);
            });
            klass.install_action(
                "win.reset-type",
                Some(glib::VariantTy::STRING),
                |win, _, parameter| {
                    if let Some(mime) = parameter.and_then(|value| value.get::<String>()) {
                        win.reset_type(&mime);
                    }
                },
            );
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for MimebindWindow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            self.catalog.set(build_catalog()).ok();

            let group_filter = gtk::CustomFilter::new(glib::clone!(
                #[weak]
                obj,
                #[upgrade_or]
                false,
                move |item| obj.shows(item)
            ));
            self.filter.append(group_filter.clone());
            self.group_filter.set(group_filter).ok();

            let factory = gtk::SignalListItemFactory::new();
            factory.connect_setup(glib::clone!(
                #[weak]
                obj,
                move |_, item| {
                    let item = item.downcast_ref::<gtk::ListItem>().expect("a list item");
                    let row = MimeRow::new();
                    obj.bind_property("narrow", &row, "narrow")
                        .sync_create()
                        .build();
                    item.set_child(Some(&row));
                }
            ));
            factory.connect_bind(|_, item| {
                let item = item.downcast_ref::<gtk::ListItem>().expect("a list item");
                if let Some(row) = item.child().and_downcast::<MimeRow>()
                    && let Some(entry) = item.item().and_downcast::<MimeEntry>()
                {
                    row.bind(&entry);
                }
            });
            self.list_view.set_factory(Some(&factory));
            self.headers.set(header_factory(&obj)).ok();

            self.groups.set_header_func(|row, before| {
                let starts_groups = before.is_some() && row.index() == 3;
                row.set_header(
                    starts_groups
                        .then(|| gtk::Separator::new(gtk::Orientation::Horizontal))
                        .as_ref(),
                );
            });
            obj.build_default_rows();

            obj.reload();
            // Changing a default flips its "modified" flag and moves the counts.
            self.store.connect_items_changed(glib::clone!(
                #[weak]
                obj,
                move |_, _, _, _| obj.rebuild_sidebar()
            ));
            obj.rebuild_sidebar();
            obj.select(Selection::Defaults);

            let groups = self.groups.get();
            glib::idle_add_local_once(glib::clone!(
                #[weak]
                groups,
                move || {
                    groups.grab_focus();
                }
            ));
        }
    }

    impl WidgetImpl for MimebindWindow {}
    impl WindowImpl for MimebindWindow {}
    impl ApplicationWindowImpl for MimebindWindow {}
    impl AdwApplicationWindowImpl for MimebindWindow {}

    #[gtk::template_callbacks]
    impl MimebindWindow {
        #[template_callback]
        fn on_sidebar_mode_changed(&self, _pspec: &glib::ParamSpec, _dropdown: &gtk::DropDown) {
            self.obj().rebuild_sidebar();
        }

        #[template_callback]
        fn on_group_selected(&self, row: Option<gtk::ListBoxRow>, _groups: &gtk::ListBox) {
            let Some(row) = row else {
                return;
            };
            let chosen = self.sidebar.borrow().get(row.index() as usize).cloned();
            // The sidebar is rebuilt whenever a default changes, and selects its row
            // again; that must not reset the list the user is working in.
            if let Some(chosen) = chosen
                && chosen != *self.selection.borrow()
            {
                self.obj().select(chosen);
            }
        }

        #[template_callback]
        fn on_group_activated(&self, _row: &gtk::ListBoxRow, _groups: &gtk::ListBox) {
            // Selecting the row that is already selected emits nothing, so going
            // back into it on a narrow window happens here.
            if self.split_view.is_collapsed() {
                self.split_view.set_show_content(true);
            }
        }

        #[template_callback]
        fn on_search_changed(&self, entry: &gtk::SearchEntry) {
            self.search_filter.set_search(Some(entry.text().as_str()));
        }

        #[template_callback]
        fn on_filtered_changed(
            &self,
            _position: u32,
            _removed: u32,
            _added: u32,
            _model: &gtk::FilterListModel,
        ) {
            self.obj().update_view();
        }

        #[template_callback]
        fn on_list_activate(&self, position: u32, list: &gtk::ListView) {
            let Some(entry) = list
                .model()
                .and_then(|model| model.item(position))
                .and_downcast::<MimeEntry>()
            else {
                return;
            };
            dialogs::open_chooser(&self.obj(), &entry);
        }
    }
}

glib::wrapper! {
    pub struct MimebindWindow(ObjectSubclass<imp::MimebindWindow>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl MimebindWindow {
    pub(crate) fn new(app: &adw::Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    /// Installed applications with every type they can open, read once at startup.
    pub(crate) fn catalog(&self) -> &[AppEntry] {
        self.imp().catalog.get().map_or(&[], Vec::as_slice)
    }

    pub(crate) fn overrides(&self) -> Ref<'_, HashSet<String>> {
        self.imp().overrides.borrow()
    }

    pub(crate) fn selection(&self) -> Selection {
        self.imp().selection.borrow().clone()
    }

    pub(crate) fn toast(&self, message: &str) {
        self.imp().toasts.add_toast(adw::Toast::new(message));
    }

    /// Refill the whole store in one splice, so the sidebar rebuilds once.
    pub(crate) fn reload(&self) {
        let imp = self.imp();
        imp.overrides.replace(user_overrides());
        // Types the user assigned belong in the list even when nothing installed
        // declares them any more, or "Modified" would hide part of what they changed.
        let mut types = installed_mime_types(self.catalog());
        types.extend(self.overrides().iter().cloned());
        let entries: Vec<MimeEntry> = {
            let overrides = self.overrides();
            types
                .iter()
                .map(|mime| MimeEntry::new(mime, &overrides))
                .collect()
        };
        imp.store.splice(0, imp.store.n_items(), &entries);
    }

    /// Rebuild one row so it shows the new default. A fresh object is required:
    /// `items_changed` over an identical item lets GtkListView keep the old widget.
    pub(crate) fn refresh(&self, entry: &MimeEntry) {
        let imp = self.imp();
        imp.overrides.replace(user_overrides());
        if let Some(position) = imp.store.find(entry) {
            let fresh = MimeEntry::new(&entry.mime(), &self.overrides());
            imp.store.splice(position, 1, &[fresh]);
        }
    }

    pub(crate) fn refresh_default_rows(&self) {
        let overrides = self.overrides();
        for (row, category) in self.imp().default_rows.borrow().iter() {
            let current = category_default(self.catalog(), &overrides, *category)
                .map(|app| app.display_name().to_string())
                .unwrap_or_else(|| gettext("Not set"));
            row.set_subtitle(&current);
        }
    }

    fn build_default_rows(&self) {
        let imp = self.imp();
        let rows = DEFAULT_CATEGORIES
            .iter()
            .map(|category| {
                let row = adw::ActionRow::builder()
                    .title(gettext(category.title))
                    .activatable(true)
                    .build();
                row.add_prefix(&gtk::Image::from_icon_name(category.icon));
                row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
                row.connect_activated(glib::clone!(
                    #[weak(rename_to = window)]
                    self,
                    move |_| dialogs::open_default_chooser(&window, *category)
                ));
                imp.defaults_list.append(&row);
                (row, *category)
            })
            .collect();
        imp.default_rows.replace(rows);
    }

    /// The group filter: whether the current selection includes this entry.
    fn shows(&self, item: &glib::Object) -> bool {
        let Some(entry) = item.downcast_ref::<MimeEntry>() else {
            return true;
        };
        match &*self.imp().selection.borrow() {
            Selection::Defaults => false,
            Selection::All => true,
            Selection::Modified => entry.modified(),
            Selection::App(id) => self
                .catalog()
                .iter()
                .find(|app| app.id == *id)
                .is_some_and(|app| app.types.contains(&entry.mime())),
            Selection::Media(group) => entry.type_group() == *group,
        }
    }

    fn select(&self, chosen: Selection) {
        let imp = self.imp();
        imp.content_page.set_title(&match &chosen {
            Selection::Defaults => gettext("Default Apps"),
            Selection::All => gettext("All File Types"),
            Selection::Modified => gettext("Modified"),
            Selection::Media(group) => group.clone(),
            Selection::App(id) => self
                .catalog()
                .iter()
                .find(|app| app.id == *id)
                .map_or_else(|| id.clone(), |app| app.name.clone()),
        });
        imp.use_all_button
            .set_visible(matches!(chosen, Selection::App(_)));
        imp.search_button.set_visible(chosen != Selection::Defaults);
        if chosen == Selection::Defaults {
            imp.search_bar.set_search_mode(false);
            // Otherwise typing anywhere still pops a search bar over a page that has
            // nothing to search.
            imp.search_bar.set_key_capture_widget(gtk::Widget::NONE);
            self.refresh_default_rows();
        } else {
            imp.search_bar.set_key_capture_widget(Some(self));
        }
        imp.selection.replace(chosen);
        if let Some(filter) = imp.group_filter.get() {
            filter.changed(gtk::FilterChange::Different);
        }
        self.update_view();
        self.scroll_to_top();
        if imp.split_view.is_collapsed() {
            imp.split_view.set_show_content(true);
        }
    }

    fn update_view(&self) {
        let imp = self.imp();
        let current = self.selection();

        // Headers read the selection when they are bound, and a section whose members
        // did not move is not rebound on its own. Re-set the factory when what a
        // header would say changes: whether it is shown at all, and for which app.
        // A single media group needs no section headers; its name is the title.
        let wanted = (
            !matches!(current, Selection::Defaults | Selection::Media(_)),
            match &current {
                Selection::App(id) => Some(id.clone()),
                _ => None,
            },
        );
        if imp.header_state.replace(wanted.clone()) != wanted {
            imp.list_view.set_header_factory(gtk::ListItemFactory::NONE);
            if wanted.0 {
                imp.list_view.set_header_factory(imp.headers.get());
            }
        }

        let count = imp.filter_model.n_items();
        imp.reset_all_button
            .set_visible(current == Selection::Modified && count > 0);

        if current == Selection::Defaults {
            imp.stack.set_visible_child_name("defaults");
            return;
        }
        if count > 0 {
            imp.stack.set_visible_child_name("list");
            return;
        }
        let empty = &imp.empty_page;
        if current == Selection::Modified {
            empty.set_icon_name(Some("document-edit-symbolic"));
            empty.set_title(&gettext("Nothing changed yet"));
            empty.set_description(Some(
                &gettext("File types you assign to an application appear here, saved in {path}.")
                    .replace("{path}", &associations_path()),
            ));
        } else {
            empty.set_icon_name(Some("system-search-symbolic"));
            empty.set_title(&gettext("No matches"));
            empty.set_description(Some(&gettext("No file type matches this search.")));
        }
        imp.stack.set_visible_child_name("empty");
    }

    fn rebuild_sidebar(&self) {
        let imp = self.imp();
        let keep = self.selection();
        let by_app = imp.sidebar_mode.selected() == 0;
        let selections = fill_sidebar(&imp.groups, &imp.store, self.catalog(), by_app);
        let position = selections
            .iter()
            .position(|candidate| *candidate == keep)
            .unwrap_or(0);
        imp.sidebar.replace(selections);
        imp.groups
            .select_row(imp.groups.row_at_index(position as i32).as_ref());
    }

    /// Offer the selected application for everything it supports, or for one group.
    fn assign(&self, group: Option<String>) {
        let Selection::App(id) = self.selection() else {
            return;
        };
        let Some(position) = self.catalog().iter().position(|app| app.id == id) else {
            return;
        };
        dialogs::confirm_assign(self, position, group);
    }

    pub(crate) fn reset_type(&self, mime: &str) {
        let imp = self.imp();
        let Some(entry) = (0..imp.store.n_items())
            .filter_map(|position| imp.store.item(position).and_downcast::<MimeEntry>())
            .find(|entry| entry.mime() == mime)
        else {
            return;
        };
        gio::AppInfo::reset_type_associations(mime);
        self.refresh(&entry);
        self.toast(&gettext("Reset {mime} to the system default").replace("{mime}", mime));
    }

    /// GtkListView scrolls its first item into view once the list settles, which
    /// parks the view one section header below the top. Put it back afterwards.
    fn scroll_to_top(&self) {
        let scroller = self.imp().scroller.get();
        glib::idle_add_local_once(glib::clone!(
            #[weak]
            scroller,
            move || scroller.vadjustment().set_value(0.0)
        ));
    }
}

/// The file GIO writes every association to, with the home directory shortened.
pub(crate) fn associations_path() -> String {
    let path = glib::user_config_dir().join("mimeapps.list");
    match path.strip_prefix(glib::home_dir()) {
        Ok(relative) => format!("~/{}", relative.display()),
        Err(_) => path.display().to_string(),
    }
}
