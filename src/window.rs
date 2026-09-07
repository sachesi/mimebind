use crate::APP_ID;
use crate::catalog::{AppEntry, build_catalog, installed_mime_types, user_overrides};
use crate::category::{DEFAULT_CATEGORIES, DefaultCategory};
use crate::dialogs::{
    confirm_assign, confirm_reset_all, open_chooser, open_default_chooser, refresh_default_rows,
};
use crate::entry::{MimeEntry, ViewState};
use crate::rows::{build_row, fill_sidebar, header_factory, property_expression, string_sorter};
use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

/// What the sidebar selection narrows the list to.
#[derive(Clone, PartialEq)]
pub(crate) enum Selection {
    Defaults,
    All,
    Modified,
    /// Every type the application with this id can open.
    App(String),
    /// Every type in this media group.
    Media(String),
}

pub(crate) fn build_window(app: &adw::Application) {
    let catalog = Rc::new(build_catalog());
    let overrides = Rc::new(RefCell::new(user_overrides()));
    let store = gio::ListStore::new::<MimeEntry>();
    reload(&store, &catalog, &overrides);

    let state: ViewState = glib::Object::new();
    let selection = Rc::new(RefCell::new(Selection::Defaults));

    // ── models ──────────────────────────────────────────────────────────────
    let search_filter = gtk::StringFilter::builder()
        .expression(property_expression("search-key"))
        .match_mode(gtk::StringFilterMatchMode::Substring)
        .ignore_case(true)
        .build();

    let group_filter = gtk::CustomFilter::new(glib::clone!(
        #[strong]
        selection,
        #[strong]
        catalog,
        move |object| {
            let Some(entry) = object.downcast_ref::<MimeEntry>() else {
                return true;
            };
            match &*selection.borrow() {
                Selection::Defaults => false,
                Selection::All => true,
                Selection::Modified => entry.modified(),
                Selection::App(id) => catalog
                    .iter()
                    .find(|app| app.id == *id)
                    .is_some_and(|app| app.types.contains(&entry.mime())),
                Selection::Media(group) => entry.type_group() == *group,
            }
        }
    ));

    let filter = gtk::EveryFilter::new();
    filter.append(search_filter.clone());
    filter.append(group_filter.clone());
    let filter_model = gtk::FilterListModel::new(Some(store.clone()), Some(filter));

    let sorted = gtk::SortListModel::new(Some(filter_model.clone()), Some(string_sorter("mime")));
    sorted.set_section_sorter(Some(&string_sorter("type-group")));

    // ── content ─────────────────────────────────────────────────────────────
    let toasts = adw::ToastOverlay::new();

    let reset: Rc<dyn Fn(&MimeEntry)> = Rc::new(glib::clone!(
        #[strong]
        store,
        #[strong]
        overrides,
        #[weak]
        toasts,
        move |entry: &MimeEntry| {
            let mime = entry.mime();
            gio::AppInfo::reset_type_associations(&mime);
            refresh(&store, entry, &overrides);
            toast(
                &toasts,
                &gettext("Reset {mime} to the system default").replace("{mime}", &mime),
            );
        }
    ));

    let factory = gtk::SignalListItemFactory::new();
    factory.connect_bind(glib::clone!(
        #[strong]
        state,
        #[strong]
        reset,
        move |_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let Some(entry) = item.item().and_downcast::<MimeEntry>() else {
                return;
            };
            // Rebuilds the row on every bind instead of recycling widgets. Split this
            // into setup and bind if scrolling ever stutters.
            item.set_child(Some(&build_row(&entry, &state, &reset)));
        }
    ));

    let headers = header_factory(selection.clone(), catalog.clone());
    let list = gtk::ListView::builder()
        .model(&gtk::NoSelection::new(Some(sorted.clone())))
        .factory(&factory)
        .header_factory(&headers)
        .single_click_activate(true)
        .css_classes(["navigation-sidebar"])
        .build();

    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&list)
        .build();

    let empty = adw::StatusPage::builder()
        .icon_name("system-search-symbolic")
        .title(gettext("No matches"))
        .build();

    let defaults = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    let defaults_page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(12)
        .margin_end(12)
        .build();
    defaults_page.append(
        &gtk::Label::builder()
            .label(gettext(
                "Choose the applications used for common file types and links.",
            ))
            .wrap(true)
            .xalign(0.0)
            .css_classes(["dim-label"])
            .build(),
    );
    defaults_page.append(&defaults);
    let defaults_clamp = adw::Clamp::builder()
        .maximum_size(600)
        .child(&defaults_page)
        .build();
    let defaults_scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&defaults_clamp)
        .build();

    let stack = gtk::Stack::new();
    stack.add_named(&defaults_scroller, Some("defaults"));
    stack.add_named(&scroller, Some("list"));
    stack.add_named(&empty, Some("empty"));

    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text(gettext("Search file types and descriptions"))
        .hexpand(true)
        .build();
    search_entry.connect_search_changed(glib::clone!(
        #[weak]
        search_filter,
        move |entry| search_filter.set_search(Some(entry.text().as_str()))
    ));

    let search_bar = gtk::SearchBar::builder()
        .child(&search_entry)
        .show_close_button(false)
        .build();
    search_bar.connect_entry(&search_entry);

    let search_button = gtk::ToggleButton::builder()
        .icon_name("system-search-symbolic")
        .tooltip_text(gettext("Search"))
        .build();
    search_button.update_property(&[gtk::accessible::Property::Label(&gettext("Search"))]);
    search_button
        .bind_property("active", &search_bar, "search-mode-enabled")
        .bidirectional()
        .sync_create()
        .build();

    let menu = gio::Menu::new();
    menu.append(Some(&gettext("_About Mimebind")), Some("app.about"));
    let menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text(gettext("Main Menu"))
        .primary(true)
        .menu_model(&menu)
        .build();
    menu_button.update_property(&[gtk::accessible::Property::Label(&gettext("Main Menu"))]);

    let use_all_button = gtk::Button::builder()
        .label(gettext("Use for All"))
        .css_classes(["suggested-action"])
        .visible(false)
        .build();
    let reset_all_button = gtk::Button::builder()
        .label(gettext("Reset All"))
        .css_classes(["destructive-action"])
        .visible(false)
        .build();

    let content_header = adw::HeaderBar::new();
    content_header.pack_start(&use_all_button);
    content_header.pack_start(&reset_all_button);
    content_header.pack_end(&menu_button);
    content_header.pack_end(&search_button);

    let content_toolbar = adw::ToolbarView::builder().content(&stack).build();
    content_toolbar.add_top_bar(&content_header);
    content_toolbar.add_top_bar(&search_bar);

    let content_page = adw::NavigationPage::builder()
        .title(gettext("All File Types"))
        .tag("content")
        .child(&content_toolbar)
        .build();

    // ── sidebar ─────────────────────────────────────────────────────────────
    let groups = gtk::ListBox::builder()
        .css_classes(["navigation-sidebar"])
        .build();
    groups.set_header_func(|row, before| {
        let starts_groups = before.is_some() && row.index() == 3;
        row.set_header(
            starts_groups
                .then(|| gtk::Separator::new(gtk::Orientation::Horizontal))
                .as_ref(),
        );
    });
    let selections: Rc<RefCell<Vec<Selection>>> = Rc::new(RefCell::new(Vec::new()));

    let sidebar_mode =
        gtk::DropDown::from_strings(&[&gettext("Applications"), &gettext("Media Types")]);
    sidebar_mode.set_tooltip_text(Some(&gettext("List the sidebar by")));

    let sidebar_header = adw::HeaderBar::new();
    sidebar_header.set_title_widget(Some(&sidebar_mode));

    let sidebar_toolbar = adw::ToolbarView::builder()
        .content(
            &gtk::ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Never)
                .vexpand(true)
                .child(&groups)
                .build(),
        )
        .build();
    sidebar_toolbar.add_top_bar(&sidebar_header);

    let sidebar_page = adw::NavigationPage::builder()
        .title(gettext("Groups"))
        .tag("sidebar")
        .child(&sidebar_toolbar)
        .build();

    let split_view = adw::NavigationSplitView::builder()
        .sidebar(&sidebar_page)
        .content(&content_page)
        .min_sidebar_width(230.0)
        .max_sidebar_width(310.0)
        .build();

    toasts.set_child(Some(&split_view));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Mimebind")
        .default_width(900)
        .default_height(660)
        .width_request(360)
        .height_request(294)
        .content(&toasts)
        .build();

    search_bar.set_key_capture_widget(Some(&window));

    let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        620.0,
        adw::LengthUnit::Sp,
    ));
    breakpoint.add_setter(&state, "narrow", Some(&true.to_value()));
    breakpoint.add_setter(&split_view, "collapsed", Some(&true.to_value()));
    window.add_breakpoint(breakpoint);

    let default_rows: Rc<Vec<(adw::ActionRow, DefaultCategory)>> = Rc::new(
        DEFAULT_CATEGORIES
            .iter()
            .map(|category| {
                let row = adw::ActionRow::builder()
                    .title(gettext(category.title))
                    .activatable(true)
                    .build();
                row.add_prefix(&gtk::Image::from_icon_name(category.icon));
                row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
                row.connect_activated(glib::clone!(
                    #[weak]
                    window,
                    #[weak]
                    toasts,
                    #[strong]
                    catalog,
                    #[strong]
                    store,
                    #[strong]
                    overrides,
                    move |row| open_default_chooser(
                        &window, &toasts, &catalog, &store, &overrides, row, *category,
                    )
                ));
                defaults.append(&row);
                (row, *category)
            })
            .collect(),
    );
    refresh_default_rows(&catalog, &overrides.borrow(), &default_rows);

    // ── wiring ──────────────────────────────────────────────────────────────
    // Headers read the selection when they are bound, and a section whose members
    // did not move is not rebound on its own. Re-set the factory when what a
    // header would say changes: whether it is shown at all, and for which app.
    let header_state: Rc<RefCell<(bool, Option<String>)>> = Rc::new(RefCell::new((true, None)));
    let update_view = glib::clone!(
        #[strong]
        header_state,
        #[weak]
        stack,
        #[weak]
        empty,
        #[weak]
        filter_model,
        #[weak]
        list,
        #[weak]
        reset_all_button,
        #[strong]
        headers,
        #[strong]
        selection,
        move || {
            let current = selection.borrow().clone();
            // A single media group needs no section headers; its name is the title.
            let wanted = (
                !matches!(current, Selection::Defaults | Selection::Media(_)),
                match &current {
                    Selection::App(id) => Some(id.clone()),
                    _ => None,
                },
            );
            if header_state.replace(wanted.clone()) != wanted {
                list.set_header_factory(gtk::ListItemFactory::NONE);
                if wanted.0 {
                    list.set_header_factory(Some(&headers));
                }
            }

            let count = filter_model.n_items();
            reset_all_button.set_visible(current == Selection::Modified && count > 0);

            if current == Selection::Defaults {
                stack.set_visible_child_name("defaults");
                return;
            }

            if count > 0 {
                stack.set_visible_child_name("list");
                return;
            }
            if current == Selection::Modified {
                empty.set_icon_name(Some("document-edit-symbolic"));
                empty.set_title(&gettext("Nothing changed yet"));
                empty.set_description(Some(
                    &gettext(
                        "File types you assign to an application appear here, saved in {path}.",
                    )
                    .replace("{path}", &associations_path()),
                ));
            } else {
                empty.set_icon_name(Some("system-search-symbolic"));
                empty.set_title(&gettext("No matches"));
                empty.set_description(Some(&gettext("No file type matches this search.")));
            }
            stack.set_visible_child_name("empty");
        }
    );
    filter_model.connect_items_changed(glib::clone!(
        #[strong]
        update_view,
        move |_, _, _, _| update_view()
    ));

    let select = glib::clone!(
        #[strong]
        selection,
        #[strong]
        catalog,
        #[weak]
        group_filter,
        #[weak]
        content_page,
        #[weak]
        split_view,
        #[weak]
        use_all_button,
        #[weak]
        search_button,
        #[weak]
        search_bar,
        #[weak]
        window,
        #[weak]
        scroller,
        #[strong]
        overrides,
        #[strong]
        default_rows,
        #[strong]
        update_view,
        move |chosen: Selection| {
            content_page.set_title(&match &chosen {
                Selection::Defaults => gettext("Default Apps"),
                Selection::All => gettext("All File Types"),
                Selection::Modified => gettext("Modified"),
                Selection::Media(group) => group.clone(),
                Selection::App(id) => catalog
                    .iter()
                    .find(|app| app.id == *id)
                    .map_or_else(|| id.clone(), |app| app.name.clone()),
            });
            use_all_button.set_visible(matches!(chosen, Selection::App(_)));
            search_button.set_visible(chosen != Selection::Defaults);
            if chosen == Selection::Defaults {
                search_bar.set_search_mode(false);
                // Otherwise typing anywhere still pops a search bar over a page
                // that has nothing to search.
                search_bar.set_key_capture_widget(gtk::Widget::NONE);
                refresh_default_rows(&catalog, &overrides.borrow(), &default_rows);
            } else {
                search_bar.set_key_capture_widget(Some(&window));
            }
            *selection.borrow_mut() = chosen;
            group_filter.changed(gtk::FilterChange::Different);
            update_view();
            scroll_to_top(&scroller);
            if split_view.is_collapsed() {
                split_view.set_show_content(true);
            }
        }
    );

    groups.connect_row_selected(glib::clone!(
        #[strong]
        selections,
        #[strong]
        select,
        move |_, row| {
            if let Some(row) = row
                && let Some(chosen) = selections.borrow().get(row.index() as usize)
            {
                select(chosen.clone());
            }
        }
    ));

    let rebuild_sidebar = glib::clone!(
        #[weak]
        groups,
        #[weak]
        sidebar_mode,
        #[strong]
        selections,
        #[strong]
        selection,
        #[strong]
        catalog,
        #[strong]
        store,
        move || {
            let keep = selection.borrow().clone();
            let by_app = sidebar_mode.selected() == 0;
            *selections.borrow_mut() = fill_sidebar(&groups, &store, &catalog, by_app);

            let position = selections
                .borrow()
                .iter()
                .position(|candidate| *candidate == keep)
                .unwrap_or(0);
            groups.select_row(groups.row_at_index(position as i32).as_ref());
        }
    );

    sidebar_mode.connect_selected_notify(glib::clone!(
        #[strong]
        rebuild_sidebar,
        move |_| rebuild_sidebar()
    ));

    // Changing a default flips its "modified" flag and moves the counts.
    store.connect_items_changed(glib::clone!(
        #[strong]
        rebuild_sidebar,
        move |_, _, _, _| rebuild_sidebar()
    ));

    use_all_button.connect_clicked(glib::clone!(
        #[weak]
        window,
        #[weak]
        toasts,
        #[strong]
        catalog,
        #[strong]
        selection,
        #[strong]
        store,
        #[strong]
        overrides,
        move |_| {
            let Selection::App(id) = selection.borrow().clone() else {
                return;
            };
            let Some(position) = catalog.iter().position(|app| app.id == id) else {
                return;
            };
            confirm_assign(
                &window, &toasts, &catalog, position, None, &store, &overrides,
            );
        }
    ));

    reset_all_button.connect_clicked(glib::clone!(
        #[weak]
        window,
        #[weak]
        toasts,
        #[strong]
        catalog,
        #[strong]
        store,
        #[strong]
        overrides,
        move |_| confirm_reset_all(&window, &toasts, &catalog, &store, &overrides)
    ));

    list.connect_activate(glib::clone!(
        #[weak]
        window,
        #[weak]
        toasts,
        #[strong]
        store,
        #[strong]
        overrides,
        move |list, position| {
            let Some(entry) = list
                .model()
                .and_then(|model| model.item(position))
                .and_downcast::<MimeEntry>()
            else {
                return;
            };
            open_chooser(&window, &toasts, &store, &overrides, &entry);
        }
    ));

    let assign_group = gio::SimpleAction::new("assign-group", Some(&String::static_variant_type()));
    assign_group.connect_activate(glib::clone!(
        #[weak]
        window,
        #[weak]
        toasts,
        #[strong]
        catalog,
        #[strong]
        selection,
        #[strong]
        store,
        #[strong]
        overrides,
        move |_, parameter| {
            let Some(group) = parameter.and_then(|value| value.get::<String>()) else {
                return;
            };
            let Selection::App(id) = selection.borrow().clone() else {
                return;
            };
            let Some(position) = catalog.iter().position(|app| app.id == id) else {
                return;
            };
            confirm_assign(
                &window,
                &toasts,
                &catalog,
                position,
                Some(group),
                &store,
                &overrides,
            );
        }
    ));
    window.add_action(&assign_group);

    let about = gio::SimpleAction::new("about", None);
    about.connect_activate(glib::clone!(
        #[weak]
        window,
        move |_, _| {
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
                            .replace("{path}", &associations_path()),
                )
                .translator_credits(gettext("translator-credits"))
                .website("https://github.com/sachesi/mimebind")
                .issue_url("https://github.com/sachesi/mimebind/issues")
                .build()
                .present(Some(&window));
        }
    ));
    app.add_action(&about);

    rebuild_sidebar();
    window.present();
    scroll_to_top(&scroller);
    glib::idle_add_local_once(glib::clone!(
        #[weak]
        groups,
        move || {
            groups.grab_focus();
        }
    ));
}

/// Refill the whole store in one splice, so the sidebar rebuilds once.
/// The file GIO writes every association to, with the home directory shortened.
pub(crate) fn associations_path() -> String {
    let path = glib::user_config_dir().join("mimeapps.list");
    match path.strip_prefix(glib::home_dir()) {
        Ok(relative) => format!("~/{}", relative.display()),
        Err(_) => path.display().to_string(),
    }
}

/// GtkListView scrolls its first item into view once the list settles, which
/// parks the view one section header below the top. Put it back afterwards.
fn scroll_to_top(scroller: &gtk::ScrolledWindow) {
    glib::idle_add_local_once(glib::clone!(
        #[weak]
        scroller,
        move || scroller.vadjustment().set_value(0.0)
    ));
}

pub(crate) fn reload(
    store: &gio::ListStore,
    catalog: &[AppEntry],
    overrides: &Rc<RefCell<HashSet<String>>>,
) {
    overrides.replace(user_overrides());
    // Types the user assigned belong in the list even when nothing installed
    // declares them any more, or "Modified" would hide part of what they changed.
    let mut types = installed_mime_types(catalog);
    types.extend(overrides.borrow().iter().cloned());
    let entries: Vec<MimeEntry> = types
        .iter()
        .map(|mime| MimeEntry::new(mime, &overrides.borrow()))
        .collect();
    store.splice(0, store.n_items(), &entries);
}

/// Rebuild one row so it shows the new default. A fresh object is required:
/// `items_changed` over an identical item lets GtkListView keep the old widget.
pub(crate) fn refresh(
    store: &gio::ListStore,
    entry: &MimeEntry,
    overrides: &Rc<RefCell<HashSet<String>>>,
) {
    overrides.replace(user_overrides());
    if let Some(position) = store.find(entry) {
        store.splice(
            position,
            1,
            &[MimeEntry::new(&entry.mime(), &overrides.borrow())],
        );
    }
}

pub(crate) fn toast(toasts: &adw::ToastOverlay, message: &str) {
    toasts.add_toast(adw::Toast::new(message));
}
