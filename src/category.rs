use crate::catalog::AppEntry;
use gtk::gio;
use gtk::prelude::*;
use std::collections::{BTreeSet, HashSet};

#[derive(Clone, Copy)]
pub(crate) enum DefaultKind {
    Browser,
    Mail,
    Calendar,
    Media(&'static str),
}

#[derive(Clone, Copy)]
pub(crate) struct DefaultCategory {
    pub(crate) title: &'static str,
    pub(crate) icon: &'static str,
    pub(crate) anchor: &'static str,
    pub(crate) kind: DefaultKind,
}

impl DefaultCategory {
    /// Whether an application can be offered for this category. Only what the
    /// desktop file declares counts: SVG and iCalendar are `text/plain` subtypes,
    /// so every code editor inherits them, and none of them is an image viewer or
    /// a calendar. Any part of a media family qualifies; the other categories
    /// need the anchor itself.
    pub(crate) fn qualifies(&self, declared: &BTreeSet<String>) -> bool {
        match self.kind {
            DefaultKind::Media(_) => declared.iter().any(|mime| is_default_type(mime, self.kind)),
            _ => declared.contains(self.anchor),
        }
    }
}

pub(crate) const DEFAULT_CATEGORIES: [DefaultCategory; 6] = [
    DefaultCategory {
        title: "Web Browser",
        icon: "web-browser-symbolic",
        anchor: "x-scheme-handler/https",
        kind: DefaultKind::Browser,
    },
    DefaultCategory {
        title: "Mail",
        icon: "mail-unread-symbolic",
        anchor: "x-scheme-handler/mailto",
        kind: DefaultKind::Mail,
    },
    DefaultCategory {
        title: "Calendar",
        icon: "x-office-calendar-symbolic",
        anchor: "text/calendar",
        kind: DefaultKind::Calendar,
    },
    DefaultCategory {
        title: "Images",
        icon: "image-x-generic-symbolic",
        anchor: "image/png",
        kind: DefaultKind::Media("image"),
    },
    DefaultCategory {
        title: "Audio",
        icon: "audio-x-generic-symbolic",
        anchor: "audio/mpeg",
        kind: DefaultKind::Media("audio"),
    },
    DefaultCategory {
        title: "Video",
        icon: "video-x-generic-symbolic",
        anchor: "video/mp4",
        kind: DefaultKind::Media("video"),
    },
];

pub(crate) fn default_types(app: &AppEntry, kind: DefaultKind) -> Vec<&String> {
    app.types
        .iter()
        .filter(|mime| is_default_type(mime, kind))
        .collect()
}

pub(crate) fn is_default_type(mime: &str, kind: DefaultKind) -> bool {
    match kind {
        DefaultKind::Browser => matches!(
            mime,
            "text/html"
                | "application/xhtml+xml"
                | "x-scheme-handler/http"
                | "x-scheme-handler/https"
        ),
        DefaultKind::Mail => matches!(mime, "message/rfc822" | "x-scheme-handler/mailto"),
        DefaultKind::Calendar => matches!(
            mime,
            "text/calendar" | "x-scheme-handler/webcal" | "x-scheme-handler/webcals"
        ),
        DefaultKind::Media(media) => mime
            .strip_prefix(media)
            .is_some_and(|rest| rest.starts_with('/')),
    }
}

/// Which application a category reports, as an id. `resolve` answers "which
/// application currently opens this type", so the rule itself stays testable.
///
/// A media category is a family of interchangeable types, so whatever opens any
/// of them owns it; the others are defined by their anchor alone, because an
/// editor that opens text/html is not the web browser. Only declared types count:
/// SVG and iCalendar are `text/plain` subtypes, so every code editor inherits
/// them and none of them is an image viewer or a calendar. The exception is a
/// default the user set by hand, which is their decision to report.
pub(crate) fn category_owner(
    declared: &[(&str, &BTreeSet<String>)],
    overrides: &HashSet<String>,
    category: DefaultCategory,
    resolve: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let current = resolve(category.anchor).or_else(|| {
        if !matches!(category.kind, DefaultKind::Media(_)) {
            return None;
        }
        let mut family: BTreeSet<&String> = BTreeSet::new();
        for (_, types) in declared {
            family.extend(
                types
                    .iter()
                    .filter(|mime| is_default_type(mime, category.kind)),
            );
        }
        family.into_iter().find_map(|mime| resolve(mime))
    })?;

    let chosen_by_hand = overrides.contains(category.anchor);
    let fits = declared
        .iter()
        .any(|(id, types)| *id == current && category.qualifies(types));
    (chosen_by_hand || fits).then_some(current)
}

/// The application a category reports, looked up against the live system.
pub(crate) fn category_default(
    catalog: &[AppEntry],
    overrides: &HashSet<String>,
    category: DefaultCategory,
) -> Option<gio::AppInfo> {
    let declared: Vec<(&str, &BTreeSet<String>)> = catalog
        .iter()
        .map(|app| (app.id.as_str(), &app.declared))
        .collect();
    let owner = category_owner(&declared, overrides, category, |mime| {
        gio::AppInfo::default_for_type(mime, false)
            .and_then(|app| app.id())
            .map(|id| id.to_string())
    })?;

    catalog
        .iter()
        .find(|app| app.id == owner)
        .map(|app| app.info.clone())
        .or_else(|| {
            // Hand-assigned to something that declares no types of its own.
            gio::AppInfo::all()
                .into_iter()
                .find(|app| app.id().is_some_and(|id| id == owner))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn types(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|kind| kind.to_string()).collect()
    }

    fn category(title: &str) -> DefaultCategory {
        *DEFAULT_CATEGORIES
            .iter()
            .find(|category| category.title == title)
            .unwrap()
    }

    /// A category belongs to the applications that claim it, not to whatever the
    /// MIME database happens to route there: an editor inherits SVG and
    /// iCalendar from text/plain and is neither an image viewer nor a calendar.
    #[test]
    fn categories_ignore_applications_that_only_inherit_the_types() {
        // The editor declares text/plain and nothing else; the MIME database is
        // what routes image/svg+xml and text/calendar to it.
        let editor = types(&["text/plain"]);
        let viewer = types(&["image/png", "image/jpeg"]);
        let no_overrides = HashSet::new();

        assert!(!category("Images").qualifies(&editor));
        assert!(!category("Calendar").qualifies(&editor));
        assert!(category("Images").qualifies(&viewer));

        // The editor opens both types today, and owns neither category.
        let declared: Vec<(&str, &BTreeSet<String>)> = vec![("editor", &editor)];
        let resolve = |_: &str| Some("editor".to_string());
        assert_eq!(
            category_owner(&declared, &no_overrides, category("Images"), resolve),
            None
        );
        assert_eq!(
            category_owner(&declared, &no_overrides, category("Calendar"), resolve),
            None
        );
    }

    /// A media category is a family: owning any declared part of it is enough,
    /// even when the anchor type itself has no default. The narrow categories
    /// have no such fallback.
    #[test]
    fn media_categories_fall_back_to_the_family() {
        let viewer = types(&["image/jpeg"]);
        let browser = types(&["text/html"]);
        let declared: Vec<(&str, &BTreeSet<String>)> =
            vec![("viewer", &viewer), ("browser", &browser)];
        let no_overrides = HashSet::new();

        // Nothing opens image/png, but the viewer opens image/jpeg.
        let resolve = |mime: &str| (mime == "image/jpeg").then(|| "viewer".to_string());
        assert_eq!(
            category_owner(&declared, &no_overrides, category("Images"), resolve),
            Some("viewer".to_string())
        );

        // text/html is not the browser anchor, and there is no family to widen to.
        let resolve = |mime: &str| (mime == "text/html").then(|| "browser".to_string());
        assert_eq!(
            category_owner(&declared, &no_overrides, category("Web Browser"), resolve),
            None
        );
    }

    /// Hiding a default the user set by hand would make this page lie about a
    /// change they made elsewhere in the same application.
    #[test]
    fn a_hand_set_default_is_reported_even_when_it_does_not_qualify() {
        let editor = types(&["text/plain"]);
        let declared: Vec<(&str, &BTreeSet<String>)> = vec![("editor", &editor)];
        let resolve = |_: &str| Some("editor".to_string());

        let overrides: HashSet<String> = ["text/calendar".to_string()].into_iter().collect();
        assert_eq!(
            category_owner(&declared, &overrides, category("Calendar"), resolve),
            Some("editor".to_string())
        );
        assert_eq!(
            category_owner(&declared, &HashSet::new(), category("Calendar"), resolve),
            None
        );
    }

    #[test]
    fn default_categories_have_strict_boundaries() {
        assert!(is_default_type(
            "x-scheme-handler/https",
            DefaultKind::Browser
        ));
        assert!(is_default_type("text/html", DefaultKind::Browser));
        assert!(!is_default_type(
            "x-scheme-handler/mailto",
            DefaultKind::Browser
        ));
        assert!(is_default_type("message/rfc822", DefaultKind::Mail));
        assert!(is_default_type(
            "x-scheme-handler/mailto",
            DefaultKind::Mail
        ));
        assert!(!is_default_type("text/calendar", DefaultKind::Mail));
        assert!(is_default_type("text/calendar", DefaultKind::Calendar));
        assert!(is_default_type(
            "x-scheme-handler/webcal",
            DefaultKind::Calendar
        ));
        assert!(!is_default_type(
            "x-scheme-handler/mailto",
            DefaultKind::Calendar
        ));
        assert!(is_default_type("image/png", DefaultKind::Media("image")));
        assert!(!is_default_type(
            "application/x-image",
            DefaultKind::Media("image")
        ));
    }
}
