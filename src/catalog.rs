use crate::entry::media_group;
use gtk::prelude::*;
use gtk::{gio, glib};
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// An installed application together with every content type it declares.
pub(crate) struct AppEntry {
    pub(crate) info: gio::AppInfo,
    pub(crate) id: String,
    pub(crate) name: String,
    /// Exactly what the desktop file claims.
    pub(crate) declared: BTreeSet<String>,
    /// `declared` plus everything the MIME database derives from it.
    pub(crate) types: BTreeSet<String>,
}

/// Applications that declare at least one content type, ordered by name.
pub(crate) fn installed_apps() -> Vec<AppEntry> {
    let mut unique = BTreeMap::<String, AppEntry>::new();
    for info in gio::AppInfo::all() {
        let Some(id) = info.id().map(|id| id.to_string()) else {
            continue;
        };
        let types: BTreeSet<String> = info
            .supported_types()
            .iter()
            .map(|kind| kind.to_string())
            .collect();
        if types.is_empty() {
            continue;
        }
        unique
            .entry(id.clone())
            .and_modify(|app| {
                app.declared.extend(types.clone());
                app.types.extend(types.clone());
            })
            .or_insert_with(|| AppEntry {
                name: info.display_name().to_string(),
                id,
                declared: types.clone(),
                types,
                info,
            });
    }
    let mut apps: Vec<AppEntry> = unique.into_values().collect();
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps
}

/// Every content type the shared MIME database knows about, plus anything an
/// application declares that the database has not heard of.
pub(crate) fn known_mime_types(apps: &[AppEntry]) -> BTreeSet<String> {
    let mut types = BTreeSet::new();
    let mut dirs = glib::system_data_dirs();
    dirs.push(glib::user_data_dir());
    for dir in dirs {
        if let Ok(text) = std::fs::read_to_string(dir.join("mime").join("types")) {
            types.extend(
                text.lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(str::to_string),
            );
        }
    }
    types.extend(apps.iter().flat_map(|app| app.types.iter().cloned()));
    types
}

/// A desktop file lists the types an application handles directly, but the MIME
/// database makes many types a subtype of one of them: an editor that claims
/// `text/plain` also opens `text/rust` and `text/x-c++src`, and GIO resolves that
/// on its own. Fold those in so an application shows what it can really open.
pub(crate) fn expand_supported_types(apps: &mut [AppEntry], known: &BTreeSet<String>) {
    // Roughly 130k content_type_is_a calls, about 0.2s once at startup. If that
    // ever matters, parse <data dir>/mime/subclasses into a parent map instead.
    let declared: BTreeSet<String> = apps
        .iter()
        .flat_map(|app| app.types.iter().cloned())
        .collect();
    let mut subtypes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for parent in &declared {
        let children: Vec<String> = known
            .iter()
            .filter(|kind| *kind != parent && gio::functions::content_type_is_a(kind, parent))
            .cloned()
            .collect();
        if !children.is_empty() {
            subtypes.insert(parent.clone(), children);
        }
    }

    for app in apps {
        let inherited: Vec<String> = app
            .types
            .iter()
            .filter_map(|kind| subtypes.get(kind))
            .flatten()
            .cloned()
            .collect();
        app.types.extend(inherited);
    }
}

/// Installed applications with their full set of openable types.
pub(crate) fn build_catalog() -> Vec<AppEntry> {
    let mut apps = installed_apps();
    let known = known_mime_types(&apps);
    expand_supported_types(&mut apps, &known);
    apps
}

/// Every content type some installed application can open. Types nothing handles
/// are left out: there would be nothing to choose for them.
pub(crate) fn installed_mime_types(apps: &[AppEntry]) -> BTreeSet<String> {
    apps.iter()
        .flat_map(|app| app.types.iter().cloned())
        .collect()
}

/// Types the user has assigned themselves, read from the file GIO writes.
/// GIO has no API for "is this mine or the system's", so the file is parsed here.
pub(crate) fn user_overrides() -> HashSet<String> {
    let path = glib::user_config_dir().join("mimeapps.list");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return HashSet::new();
    };

    let mut overrides = HashSet::new();
    let mut in_defaults = false;
    for line in text.lines().map(str::trim) {
        if let Some(section) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            in_defaults = section == "Default Applications";
        } else if in_defaults && let Some((mime, _)) = line.split_once('=') {
            overrides.insert(mime.trim().to_string());
        }
    }
    overrides
}

/// True when this application already wins the type, so nothing needs writing.
pub(crate) fn already_default(app: &AppEntry, mime: &str) -> bool {
    gio::AppInfo::default_for_type(mime, false)
        .and_then(|current| current.id())
        .is_some_and(|current| current == app.id)
}

/// Make one application the default for the given types, skipping the ones it
/// already opens. Returns how many were set and how many were refused.
pub(crate) struct Assignment {
    pub(crate) set: usize,
    pub(crate) failed: usize,
    /// The first refusal, kept so the report can name a cause rather than a count.
    pub(crate) error: Option<glib::Error>,
}

impl Assignment {
    /// What to tell the user after a bulk assignment.
    pub(crate) fn report(&self, app: &str) -> String {
        match (self.set, &self.error) {
            (0, None) => format!("{app} already opened every type it supports"),
            (set, None) => format!("{app} now opens {set} more file types"),
            (set, Some(error)) => format!(
                "{app} now opens {set} more file types, {} could not be set: {error}",
                self.failed
            ),
        }
    }
}

pub(crate) fn assign_types<'a>(
    app: &AppEntry,
    mimes: impl Iterator<Item = &'a String>,
) -> Assignment {
    let mut outcome = Assignment {
        set: 0,
        failed: 0,
        error: None,
    };
    for mime in mimes {
        if already_default(app, mime) {
            continue;
        }
        match app.info.set_as_default_for_type(mime) {
            Ok(()) => outcome.set += 1,
            Err(error) => {
                outcome.failed += 1;
                outcome.error.get_or_insert(error);
            }
        }
    }
    outcome
}

/// The types a bulk action covers: everything an application supports, or only
/// the part of it in one media group.
pub(crate) fn scoped_types<'a>(app: &'a AppEntry, group: Option<&'a str>) -> Vec<&'a String> {
    app.types
        .iter()
        .filter(|mime| group.is_none_or(|group| media_group(mime) == group))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Once;

    fn test_config_home() -> PathBuf {
        static INIT: Once = Once::new();
        let scratch = std::env::temp_dir().join("mimebind-test-config");
        INIT.call_once(|| {
            std::fs::remove_dir_all(&scratch).ok();
            std::fs::create_dir_all(&scratch).unwrap();
            unsafe { std::env::set_var("XDG_CONFIG_HOME", &scratch) };
        });
        scratch
    }

    /// An application that claims a supertype must be credited with the subtypes
    /// the MIME database derives from it, or an editor claiming `text/plain`
    /// looks like it cannot open source files.
    #[test]
    fn supported_types_follow_the_mime_database() {
        test_config_home();
        let declared = installed_apps();
        let known = known_mime_types(&declared);
        let mut expanded = installed_apps();
        expand_supported_types(&mut expanded, &known);

        let Some(position) = declared
            .iter()
            .position(|app| app.types.contains("text/plain"))
        else {
            return; // nothing installed claims text/plain on this machine
        };

        let gained: Vec<&String> = expanded[position]
            .types
            .difference(&declared[position].types)
            .collect();
        assert!(
            !gained.is_empty(),
            "{} gained no subtypes of text/plain",
            declared[position].name
        );
        for kind in gained {
            assert!(
                declared[position]
                    .types
                    .iter()
                    .any(|parent| gio::functions::content_type_is_a(kind, parent)),
                "{kind} is not a subtype of anything {} declares",
                declared[position].name
            );
        }
    }

    /// The app rests on GIO owning `mimeapps.list`: single and bulk writes must
    /// land in XDG_CONFIG_HOME, be visible to the "Modified" group, and be undone
    /// by `reset_type_associations`.
    #[test]
    fn association_round_trips() {
        let scratch = test_config_home();

        let apps = build_catalog();
        assert!(!apps.is_empty(), "no installed application declares a type");
        assert!(
            !installed_mime_types(&apps).is_empty(),
            "no content types found"
        );

        // One type.
        let app = &apps[0];
        let mime = app.types.iter().next().unwrap().clone();
        app.info.set_as_default_for_type(&mime).unwrap();
        assert_eq!(
            gio::AppInfo::default_for_type(&mime, false).map(|a| a.id()),
            Some(app.info.id())
        );
        assert!(user_overrides().contains(&mime), "{mime} not an override");

        gio::AppInfo::reset_type_associations(&mime);
        assert!(
            !user_overrides().contains(&mime),
            "{mime} still an override"
        );

        // Every type the widest application supports. Another application takes
        // one of them first, so the bulk write has something to reclaim.
        let widest = apps.iter().max_by_key(|app| app.types.len()).unwrap();
        let (thief, stolen) = apps
            .iter()
            .filter(|other| other.id != widest.id)
            .find_map(|other| {
                other
                    .types
                    .iter()
                    .find(|kind| widest.types.contains(*kind))
                    .map(|kind| (other, kind.clone()))
            })
            .expect("no type shared by two applications");
        thief.info.set_as_default_for_type(&stolen).unwrap();
        assert!(!already_default(widest, &stolen));

        let changing = widest
            .types
            .iter()
            .filter(|mime| !already_default(widest, mime))
            .count();
        let outcome = assign_types(widest, widest.types.iter());
        assert_eq!(outcome.failed, 0, "{:?}", outcome.error);
        // Fewer writes than planned is correct: assigning a supertype makes its
        // subtypes resolve to the same application, so they get skipped.
        assert!(
            outcome.set > 0 && outcome.set <= changing,
            "wrote {} of a planned {changing}",
            outcome.set
        );
        for mime in &widest.types {
            assert_eq!(
                gio::AppInfo::default_for_type(mime, false).map(|a| a.id()),
                Some(widest.info.id()),
                "{mime} did not take the bulk default"
            );
        }

        for mime in user_overrides() {
            gio::AppInfo::reset_type_associations(&mime);
        }
        assert!(user_overrides().is_empty(), "bulk reset left overrides");

        std::fs::remove_dir_all(&scratch).ok();
    }
}
