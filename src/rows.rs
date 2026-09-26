use crate::catalog::AppEntry;
use crate::entry::MimeEntry;
use crate::window::{MimebindWindow, Selection};
use adw::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};
use std::collections::BTreeMap;

/// One row of the sidebar: what it selects and what it shows.
pub(crate) struct SidebarItem {
    pub(crate) selection: Selection,
    title: String,
    icon: Option<gio::Icon>,
    fallback: &'static str,
    pub(crate) count: Option<u32>,
}

/// The group list: defaults, all types, modified, then either every application
/// that can open something or every media group.
pub(crate) fn sidebar_items(
    store: &gio::ListStore,
    catalog: &[AppEntry],
    by_app: bool,
) -> Vec<SidebarItem> {
    let entries: Vec<MimeEntry> = (0..store.n_items())
        .filter_map(|position| store.item(position).and_downcast::<MimeEntry>())
        .collect();
    let modified = entries.iter().filter(|entry| entry.modified()).count() as u32;

    let mut items = vec![
        SidebarItem {
            selection: Selection::Defaults,
            title: gettext("Default Applications"),
            icon: None,
            fallback: "object-select-symbolic",
            count: None,
        },
        SidebarItem {
            selection: Selection::All,
            title: gettext("All File Types"),
            icon: None,
            fallback: "view-list-symbolic",
            count: Some(entries.len() as u32),
        },
        SidebarItem {
            selection: Selection::Modified,
            title: gettext("Modified"),
            icon: None,
            fallback: "document-edit-symbolic",
            count: Some(modified),
        },
    ];

    if by_app {
        items.extend(catalog.iter().map(|app| SidebarItem {
            selection: Selection::App(app.id.clone()),
            title: app.name.clone(),
            icon: app.info.icon(),
            fallback: "application-x-executable-symbolic",
            count: Some(app.types.len() as u32),
        }));
        return items;
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
    items.extend(counts.into_iter().map(|(name, (count, icon))| SidebarItem {
        selection: Selection::Media(name.clone()),
        title: name,
        icon,
        fallback: "text-x-generic-symbolic",
        count: Some(count),
    }));
    items
}

/// The row for `item`, and the label that shows its count.
pub(crate) fn sidebar_row(item: &SidebarItem) -> (adw::ActionRow, Option<gtk::Label>) {
    let row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(&item.title))
        .activatable(true)
        .build();

    let image = gtk::Image::new();
    match &item.icon {
        // Application icons carry meaning at a glance, so give them the same
        // size the file type icons get in the list.
        Some(icon) => {
            image.set_pixel_size(32);
            image.set_from_gicon(icon);
        }
        None => image.set_icon_name(Some(item.fallback)),
    }
    row.add_prefix(&image);

    // Every number in this column counts file types, so the categories row,
    // which would count something else, carries none.
    let count = item.count.map(|count| {
        let label = gtk::Label::builder()
            .label(count.to_string())
            .css_classes(["dim-label", "numeric"])
            .build();
        row.add_suffix(&label);
        label
    });
    (row, count)
}

/// Section header: the media group, how many types it holds, and, when an
/// application is selected, a button that assigns just this group to it.
pub(crate) fn header_factory(window: &MimebindWindow) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, header| {
        let header = header
            .downcast_ref::<gtk::ListHeader>()
            .expect("a list header");
        let row = gtk::Box::builder()
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(12)
            .margin_end(12)
            .build();
        row.append(
            &gtk::Label::builder()
                .xalign(0.0)
                .hexpand(true)
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .css_classes(["heading"])
                .build(),
        );
        row.append(
            &gtk::Button::builder()
                .label(gettext("Use for These"))
                .valign(gtk::Align::Center)
                .css_classes(["flat"])
                .build(),
        );
        header.set_child(Some(&row));
    });
    factory.connect_bind(glib::clone!(
        #[weak]
        window,
        move |_, header| {
            let header = header
                .downcast_ref::<gtk::ListHeader>()
                .expect("a list header");
            let Some(entry) = header.item().and_downcast::<MimeEntry>() else {
                return;
            };
            let row = header.child().expect("a header row from setup");
            let label = row.first_child().and_downcast::<gtk::Label>();
            let button = row.last_child().and_downcast::<gtk::Button>();
            let (Some(label), Some(button)) = (label, button) else {
                return;
            };
            let group = entry.type_group();
            label.set_label(
                // Translators: a section of the list: a group of file types, such as
                // "Images", and how many there are.
                &gettext("{group} ({count})")
                    .replace("{group}", &group)
                    .replace("{count}", &header.n_items().to_string()),
            );

            let app = match window.selection() {
                Selection::App(id) => window.catalog().iter().find(|app| app.id == id),
                _ => None,
            };
            button.set_visible(app.is_some());
            if let Some(app) = app {
                // The window owns the action, so the button does not need to capture
                // anything that is built after this factory. The target goes first:
                // an action name without one is a type mismatch GTK warns about.
                button.set_action_target_value(Some(&group.to_variant()));
                button.set_action_name(Some("win.assign-group"));
                button.set_tooltip_text(Some(
                    &ngettext(
                        "Use {app} for the one {group} file type",
                        "Use {app} for all {count} {group} file types",
                        header.n_items(),
                    )
                    .replace("{app}", &app.name)
                    .replace("{count}", &header.n_items().to_string())
                    .replace("{group}", &group),
                ));
            }
        }
    ));
    factory
}
