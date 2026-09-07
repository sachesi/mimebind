use crate::catalog::AppEntry;
use crate::entry::{MimeEntry, ViewState};
use crate::window::Selection;
use adw::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib, pango};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

pub(crate) fn property_expression(property: &str) -> gtk::PropertyExpression {
    gtk::PropertyExpression::new(MimeEntry::static_type(), None::<gtk::Expression>, property)
}

pub(crate) fn string_sorter(property: &str) -> gtk::StringSorter {
    gtk::StringSorter::new(Some(property_expression(property)))
}

/// Rebuild the group list: defaults, all types, modified, then either every
/// application that can open something or every media group.
pub(crate) fn fill_sidebar(
    groups: &gtk::ListBox,
    store: &gio::ListStore,
    catalog: &[AppEntry],
    by_app: bool,
) -> Vec<Selection> {
    while let Some(row) = groups.first_child() {
        groups.remove(&row);
    }

    let entries: Vec<MimeEntry> = (0..store.n_items())
        .filter_map(|position| store.item(position).and_downcast::<MimeEntry>())
        .collect();
    let modified = entries.iter().filter(|entry| entry.modified()).count() as u32;

    groups.append(&sidebar_row(
        &gettext("Default Apps"),
        None,
        "object-select-symbolic",
        None,
    ));
    groups.append(&sidebar_row(
        &gettext("All File Types"),
        None,
        "view-list-symbolic",
        Some(entries.len() as u32),
    ));
    groups.append(&sidebar_row(
        &gettext("Modified"),
        None,
        "document-edit-symbolic",
        Some(modified),
    ));
    let mut selections = vec![Selection::Defaults, Selection::All, Selection::Modified];

    if by_app {
        for app in catalog {
            groups.append(&sidebar_row(
                &app.name,
                app.info.icon(),
                "application-x-executable-symbolic",
                Some(app.types.len() as u32),
            ));
            selections.push(Selection::App(app.id.clone()));
        }
        return selections;
    }

    let mut counts: BTreeMap<String, (u32, Option<gio::Icon>)> = BTreeMap::new();
    for entry in &entries {
        let slot = counts.entry(entry.type_group()).or_insert((0, None));
        slot.0 += 1;
        if slot.1.is_none() {
            slot.1 = gio::functions::content_type_get_generic_icon_name(&entry.mime())
                .map(|name| gio::ThemedIcon::new(&name).upcast());
        }
    }
    for (name, (count, icon)) in counts {
        groups.append(&sidebar_row(
            &name,
            icon,
            "text-x-generic-symbolic",
            Some(count),
        ));
        selections.push(Selection::Media(name));
    }
    selections
}

pub(crate) fn sidebar_row(
    title: &str,
    icon: Option<gio::Icon>,
    fallback: &str,
    count: Option<u32>,
) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(title))
        .build();

    let image = gtk::Image::new();
    match icon {
        // Application icons carry meaning at a glance, so give them the same
        // size the file type icons get in the list.
        Some(icon) => {
            image.set_pixel_size(32);
            image.set_from_gicon(&icon);
        }
        None => image.set_icon_name(Some(fallback)),
    }
    row.add_prefix(&image);

    // Every number in this column counts file types, so the categories row,
    // which would count something else, carries none.
    if let Some(count) = count {
        row.add_suffix(
            &gtk::Label::builder()
                .label(count.to_string())
                .css_classes(["dim-label", "numeric"])
                .build(),
        );
    }
    row
}

/// Section header: the media group, how many types it holds, and, when an
/// application is selected, a button that assigns just this group to it.
pub(crate) fn header_factory(
    selection: Rc<RefCell<Selection>>,
    catalog: Rc<Vec<AppEntry>>,
) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_bind(move |_, header| {
        let header = header.downcast_ref::<gtk::ListHeader>().unwrap();
        let Some(entry) = header.item().and_downcast::<MimeEntry>() else {
            return;
        };
        let group = entry.type_group();

        let row = gtk::Box::builder()
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(12)
            .margin_end(12)
            .build();
        row.append(
            &gtk::Label::builder()
                .label(format!("{group} ({})", header.n_items()))
                .xalign(0.0)
                .hexpand(true)
                .css_classes(["heading"])
                .build(),
        );

        let chosen = selection.borrow().clone();
        if let Selection::App(id) = chosen
            && let Some(app) = catalog.iter().find(|app| app.id == id)
        {
            // The window owns the action, so the button does not need to capture
            // anything that is built after this factory.
            let button = gtk::Button::builder()
                .label(gettext("Use for These"))
                .valign(gtk::Align::Center)
                .css_classes(["flat"])
                .action_name("win.assign-group")
                .action_target(&group.to_variant())
                .tooltip_text(
                    ngettext(
                        "Use {app} for the one {group} file type",
                        "Use {app} for all {count} {group} file types",
                        header.n_items(),
                    )
                    .replace("{app}", &app.name)
                    .replace("{count}", &header.n_items().to_string())
                    .replace("{group}", &group),
                )
                .build();
            row.append(&button);
        }

        header.set_child(Some(&row));
    });
    factory
}

/// A list row: type icon, description, MIME name, and the current default app.
/// Plain widgets rather than `AdwActionRow` because a `GtkListBoxRow` outside a
/// `GtkListBox` hits `gtk_list_box_row_grab_focus: assertion 'box != NULL'`.
pub(crate) fn build_row(
    entry: &MimeEntry,
    state: &ViewState,
    reset: &Rc<dyn Fn(&MimeEntry)>,
) -> gtk::Widget {
    let mime = entry.mime();

    let row = gtk::Box::builder().spacing(12).build();

    let type_icon = gtk::Image::builder().pixel_size(32).build();
    type_icon.set_from_gicon(&gio::functions::content_type_get_icon(&mime));
    row.append(&type_icon);

    let text = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .valign(gtk::Align::Center)
        .hexpand(true)
        .build();
    text.append(&ellipsized(&entry.description(), &[]));
    text.append(&ellipsized(&mime, &["dim-label", "caption"]));
    row.append(&text);

    let default_app = gio::AppInfo::default_for_type(&mime, false);
    // Always present, so a type with no handler does not shift the column.
    let app_icon = gtk::Image::builder().pixel_size(16).build();
    if let Some(icon) = default_app.as_ref().and_then(|app| app.icon()) {
        app_icon.set_from_gicon(&icon);
    }
    row.append(&app_icon);
    let name = default_app
        .as_ref()
        .map(|app| app.display_name().to_string())
        .unwrap_or_else(|| gettext("Not set"));
    let name_label = ellipsized(&name, &["dim-label"]);
    name_label.set_max_width_chars(20);
    // No room for a second column of text on a narrow window; the icon carries it.
    state
        .bind_property("narrow", &name_label, "visible")
        .invert_boolean()
        .sync_create()
        .build();
    row.append(&name_label);

    // The button keeps its place on every row so the column above it stays
    // straight; on an unchanged type it is simply invisible and inert.
    let revert = gtk::Button::builder()
        .icon_name("edit-undo-symbolic")
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build();
    if entry.modified() {
        revert.set_tooltip_text(Some(&gettext("Reset to system default")));
        revert.update_property(&[gtk::accessible::Property::Label("Reset to system default")]);
        revert.connect_clicked(glib::clone!(
            #[strong]
            entry,
            #[strong]
            reset,
            move |_| reset(&entry)
        ));
    } else {
        revert.set_opacity(0.0);
        revert.set_can_target(false);
        revert.set_can_focus(false);
        revert.update_state(&[gtk::accessible::State::Hidden(true)]);
    }
    row.append(&revert);

    row.append(&gtk::Image::from_icon_name("go-next-symbolic"));

    row.update_property(&[gtk::accessible::Property::Label(&format!(
        "{}, {mime}, opens with {name}",
        entry.description()
    ))]);

    row.upcast()
}

pub(crate) fn ellipsized(text: &str, css_classes: &[&str]) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .ellipsize(pango::EllipsizeMode::End)
        .css_classes(css_classes.to_vec())
        .build()
}
