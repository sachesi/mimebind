use crate::catalog::{AppEntry, already_default, assign_types, scoped_types};
use crate::category::{DefaultCategory, category_default, default_types};
use crate::entry::MimeEntry;
use crate::window::{refresh, reload, toast};
use adw::prelude::*;
use gtk::{gio, glib};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

/// Bulk assignment rewrites a lot of the user's configuration, so it is confirmed
/// first and the message says how many types would change hands.
/// `group` limits it to one media group; `None` means everything the app supports.
pub(crate) fn confirm_assign(
    parent: &adw::ApplicationWindow,
    toasts: &adw::ToastOverlay,
    catalog: &Rc<Vec<AppEntry>>,
    position: usize,
    group: Option<String>,
    store: &gio::ListStore,
    overrides: &Rc<RefCell<HashSet<String>>>,
) {
    let app = &catalog[position];
    let mimes = scoped_types(app, group.as_deref());
    let total = mimes.len();
    if total == 0 {
        return;
    }
    let changing = mimes
        .iter()
        .filter(|mime| !already_default(app, mime))
        .count();

    let scope = match &group {
        Some(group) => format!("{group} file types"),
        None => "file types".to_string(),
    };
    let dialog = adw::AlertDialog::builder()
        .heading(match &group {
            Some(group) => format!("Use {} for {group}?", app.name),
            None => format!("Use {} for All Supported Types?", app.name),
        })
        .body(match changing {
            0 => format!("{} already opens all {total} of those {scope}.", app.name),
            _ => format!(
                "{} supports {total} {scope} and would take over {changing} of them \
                 from another application.",
                app.name
            ),
        })
        .build();
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("apply", "Set Defaults");
    dialog.set_response_appearance("apply", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("apply"));
    dialog.set_close_response("cancel");

    dialog.connect_response(
        None,
        glib::clone!(
            #[weak]
            toasts,
            #[strong]
            catalog,
            #[strong]
            store,
            #[strong]
            overrides,
            move |_, response| {
                if response != "apply" {
                    return;
                }
                let app = &catalog[position];
                let outcome = assign_types(app, scoped_types(app, group.as_deref()).into_iter());
                reload(&store, &catalog, &overrides);
                toast(&toasts, &outcome.report(&app.name));
            }
        ),
    );

    dialog.present(Some(parent));
}

pub(crate) fn confirm_reset_all(
    parent: &adw::ApplicationWindow,
    toasts: &adw::ToastOverlay,
    catalog: &Rc<Vec<AppEntry>>,
    store: &gio::ListStore,
    overrides: &Rc<RefCell<HashSet<String>>>,
) {
    let count = overrides.borrow().len();
    let dialog = adw::AlertDialog::builder()
        .heading("Reset All Changes?")
        .body(format!(
            "{count} file types go back to the application the system chose."
        ))
        .build();
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("reset", "Reset All");
    dialog.set_response_appearance("reset", adw::ResponseAppearance::Destructive);
    dialog.set_close_response("cancel");

    dialog.connect_response(
        None,
        glib::clone!(
            #[weak]
            toasts,
            #[strong]
            catalog,
            #[strong]
            store,
            #[strong]
            overrides,
            move |_, response| {
                if response != "reset" {
                    return;
                }
                for mime in overrides.borrow().iter() {
                    gio::AppInfo::reset_type_associations(mime);
                }
                reload(&store, &catalog, &overrides);
                toast(&toasts, &format!("Reset {count} file types"));
            }
        ),
    );

    dialog.present(Some(parent));
}

pub(crate) fn refresh_default_rows(
    catalog: &[AppEntry],
    overrides: &HashSet<String>,
    rows: &[(adw::ActionRow, DefaultCategory)],
) {
    for (row, category) in rows {
        let current = category_default(catalog, overrides, *category)
            .map(|app| app.display_name().to_string())
            .unwrap_or_else(|| "Not set".into());
        row.set_subtitle(&current);
    }
}

pub(crate) fn open_default_chooser(
    parent: &adw::ApplicationWindow,
    toasts: &adw::ToastOverlay,
    catalog: &Rc<Vec<AppEntry>>,
    store: &gio::ListStore,
    overrides: &Rc<RefCell<HashSet<String>>>,
    source_row: &adw::ActionRow,
    category: DefaultCategory,
) {
    let dialog = adw::AlertDialog::builder()
        .heading(format!("Default {}", category.title))
        .body("The selected application will handle every supported type in this category.")
        .build();
    dialog.add_response("cancel", "Cancel");
    dialog.set_close_response("cancel");

    // Offer exactly what the row can then report back, so a choice never lands
    // somewhere the category cannot see it.
    let current = category_default(catalog, &overrides.borrow(), category).and_then(|app| app.id());
    let mut candidates: Vec<usize> = catalog
        .iter()
        .enumerate()
        .filter(|(_, app)| category.qualifies(&app.declared))
        .map(|(position, _)| position)
        .collect();
    // A default the user set by hand stays listed even if the application would
    // not qualify, so the row and this list never disagree.
    if let Some(current) = &current
        && let Some(position) = catalog.iter().position(|app| app.id == *current)
        && !candidates.contains(&position)
    {
        candidates.push(position);
    }
    if candidates.is_empty() {
        dialog.set_body("No installed application declares support for these file types.");
        dialog.present(Some(parent));
        return;
    }

    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();

    for position in candidates {
        let app = &catalog[position];
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&app.name))
            .activatable(true)
            .build();
        if let Some(icon) = app.info.icon() {
            let image = gtk::Image::builder().pixel_size(32).build();
            image.set_from_gicon(&icon);
            row.add_prefix(&image);
        }
        if current.as_deref() == Some(app.id.as_str()) {
            row.add_suffix(&gtk::Image::from_icon_name("object-select-symbolic"));
        }
        row.connect_activated(glib::clone!(
            #[weak]
            dialog,
            #[weak]
            toasts,
            #[strong]
            catalog,
            #[strong]
            store,
            #[strong]
            overrides,
            #[weak]
            source_row,
            move |_| {
                let app = &catalog[position];
                let outcome = assign_types(app, default_types(app, category.kind).into_iter());
                reload(&store, &catalog, &overrides);
                let current = category_default(&catalog, &overrides.borrow(), category)
                    .map(|app| app.display_name().to_string())
                    .unwrap_or_else(|| "Not set".into());
                source_row.set_subtitle(&current);
                let message = match &outcome.error {
                    None => format!("{} is now the default for {}", app.name, category.title),
                    Some(error) => format!(
                        "{} of the {} types could not be set: {error}",
                        outcome.failed, category.title
                    ),
                };
                toast(&toasts, &message);
                dialog.close();
            }
        ));
        list.append(&row);
    }

    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(420)
        .child(&list)
        .build();
    dialog.set_extra_child(Some(&scroller));
    dialog.present(Some(parent));
}

pub(crate) fn open_chooser(
    parent: &adw::ApplicationWindow,
    toasts: &adw::ToastOverlay,
    store: &gio::ListStore,
    overrides: &Rc<RefCell<HashSet<String>>>,
    entry: &MimeEntry,
) {
    let mime = entry.mime();
    let dialog = adw::AlertDialog::builder()
        .heading(entry.description())
        .body(&mime)
        .build();
    dialog.add_response("close", "Cancel");
    dialog.set_close_response("close");
    if entry.modified() {
        dialog.add_response("reset", "Reset to System Default");
        dialog.set_response_appearance("reset", adw::ResponseAppearance::Destructive);
    }

    let candidates = gio::AppInfo::all_for_type(&mime);
    let current = gio::AppInfo::default_for_type(&mime, false);

    if candidates.is_empty() {
        dialog.set_body(&format!(
            "{mime}\n\nNo installed application declares support for this type."
        ));
    } else {
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();

        for app in &candidates {
            let row = adw::ActionRow::builder()
                .title(glib::markup_escape_text(&app.display_name()))
                .activatable(true)
                .build();
            if let Some(description) = app.description() {
                row.set_subtitle(&glib::markup_escape_text(&description));
            }
            if let Some(icon) = app.icon() {
                let image = gtk::Image::builder().pixel_size(32).build();
                image.set_from_gicon(&icon);
                row.add_prefix(&image);
            }
            if current.as_ref().map(|c| c.id()) == Some(app.id()) {
                row.add_suffix(&gtk::Image::from_icon_name("object-select-symbolic"));
            }

            let app = app.clone();
            let mime = mime.clone();
            let store = store.clone();
            let entry = entry.clone();
            let overrides = overrides.clone();
            let toasts = toasts.clone();
            row.connect_activated(glib::clone!(
                #[weak]
                dialog,
                move |_| {
                    match app.set_as_default_for_type(&mime) {
                        Ok(()) => {
                            toast(&toasts, &format!("{} now opens {mime}", app.display_name()))
                        }
                        Err(error) => toast(&toasts, &format!("Could not set default: {error}")),
                    }
                    refresh(&store, &entry, &overrides);
                    dialog.close();
                }
            ));
            list.append(&row);
        }

        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .max_content_height(420)
            .child(&list)
            .build();
        dialog.set_extra_child(Some(&scroller));
    }

    let store = store.clone();
    let entry = entry.clone();
    let overrides = overrides.clone();
    let toasts = toasts.clone();
    dialog.connect_response(None, move |_, response| {
        if response == "reset" {
            gio::AppInfo::reset_type_associations(&mime);
            refresh(&store, &entry, &overrides);
            toast(&toasts, &format!("Reset {mime} to the system default"));
        }
    });

    dialog.present(Some(parent));
}
