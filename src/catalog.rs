use crate::entry::media_group;
use gettextrs::{gettext, ngettext};
use gtk::prelude::*;
use gtk::{gio, glib};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

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

/// Every content type some application declares.
pub(crate) fn declared_types(apps: &[AppEntry]) -> BTreeSet<String> {
    apps.iter()
        .flat_map(|app| app.declared.iter().cloned())
        .collect()
}

/// Every content type the shared MIME database knows about, plus anything an
/// application declares that the database has not heard of.
pub(crate) fn known_mime_types(declared: &BTreeSet<String>) -> BTreeSet<String> {
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
    types.extend(declared.iter().cloned());
    types
}

/// A desktop file lists the types an application handles directly, but the MIME
/// database makes many types a subtype of one of them: an editor that claims
/// `text/plain` also opens `text/rust` and `text/x-c++src`, and GIO resolves that
/// on its own. Maps each declared type to the known types derived from it.
///
/// Roughly 130k `content_type_is_a` calls, about 0.2s, so the window runs this on
/// a worker thread. If that ever matters, parse <data dir>/mime/subclasses into a
/// parent map instead.
pub(crate) fn subtypes_of(declared: &BTreeSet<String>) -> BTreeMap<String, Vec<String>> {
    let known = known_mime_types(declared);
    let mut subtypes = BTreeMap::new();
    for parent in declared {
        let children: Vec<String> = known
            .iter()
            .filter(|kind| *kind != parent && gio::functions::content_type_is_a(kind, parent))
            .cloned()
            .collect();
        if !children.is_empty() {
            subtypes.insert(parent.clone(), children);
        }
    }
    subtypes
}

/// Credit every application with the subtypes of what it declares, so it shows
/// what it can really open.
pub(crate) fn expand_supported_types(
    apps: &mut [AppEntry],
    subtypes: &BTreeMap<String, Vec<String>>,
) {
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
    defaults_in(&glib::user_config_dir().join("mimeapps.list"))
}

/// The types with an entry under [Default Applications] in a mimeapps.list.
fn defaults_in(path: &Path) -> HashSet<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
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

/// True when the application with this id already wins the type, so nothing
/// needs writing.
pub(crate) fn already_default(id: &str, mime: &str) -> bool {
    gio::AppInfo::default_for_type(mime, false)
        .and_then(|current| current.id())
        .is_some_and(|current| current == id)
}

/// `path` as the user would type it, with the home directory shortened to `~`.
pub(crate) fn home_relative(path: &Path) -> String {
    match path.strip_prefix(glib::home_dir()) {
        Ok(relative) => format!("~/{}", relative.display()),
        Err(_) => path.display().to_string(),
    }
}

/// The desktop-specific list, such as gnome-mimeapps.list, that names a default
/// for `mime`. The XDG spec reads one next to mimeapps.list before it, and GIO
/// only ever writes mimeapps.list, so a default set there cannot be overridden.
fn overriding_list(mime: &str) -> Option<PathBuf> {
    let desktops = std::env::var("XDG_CURRENT_DESKTOP").ok()?;
    desktops
        .split(':')
        .map(|desktop| {
            glib::user_config_dir().join(format!("{}-mimeapps.list", desktop.to_lowercase()))
        })
        .find(|path| defaults_in(path).contains(mime))
}

/// GIO accepts a default it then does not apply when another list takes
/// precedence, so check that the application with this id really opens `mime`.
fn verify_default(id: &str, mime: &str) -> Result<(), glib::Error> {
    if already_default(id, mime) {
        return Ok(());
    }
    let reason = match overriding_list(mime) {
        // Translators: {path} is a file such as ~/.config/gnome-mimeapps.list that
        // chooses another application and is read before the one this app writes.
        Some(path) => gettext("{path} takes precedence").replace("{path}", &home_relative(&path)),
        None => gettext("another setting takes precedence"),
    };
    Err(glib::Error::new(gio::IOErrorEnum::Failed, &reason))
}

/// What a bulk assignment did: how many types the application opens now that it
/// did not before, and how many it still does not.
pub(crate) struct Assignment {
    pub(crate) gained: usize,
    pub(crate) failed: usize,
    /// The first refusal, kept so the report can name a cause rather than a count.
    pub(crate) error: Option<glib::Error>,
}

impl Assignment {
    /// What to tell the user after a bulk assignment.
    pub(crate) fn report(&self, app: &str) -> String {
        match (self.gained, &self.error) {
            (0, None) => {
                gettext("{app} already opens every file type it supports").replace("{app}", app)
            }
            (set, None) => ngettext(
                "{app} now opens one more file type",
                "{app} now opens {count} more file types",
                set as u32,
            )
            .replace("{app}", app)
            .replace("{count}", &set.to_string()),
            (set, Some(error)) => ngettext(
                "{app} now opens one more file type, {failed} could not be set: {error}",
                "{app} now opens {count} more file types, {failed} could not be set: {error}",
                set as u32,
            )
            .replace("{app}", app)
            .replace("{count}", &set.to_string())
            .replace("{failed}", &self.failed.to_string())
            .replace("{error}", &error.to_string()),
        }
    }
}

/// The installed application with this desktop id. Writes run on a worker thread,
/// and a `gio::AppInfo` cannot be sent to one, so the worker looks it up itself.
fn find_app(id: &str) -> Result<gio::AppInfo, glib::Error> {
    gio::AppInfo::all()
        .into_iter()
        .find(|info| info.id().is_some_and(|candidate| candidate == id))
        .ok_or_else(|| {
            glib::Error::new(
                gio::IOErrorEnum::NotFound,
                // Translators: {app} is a desktop file name, such as org.gnome.TextEditor.desktop.
                &gettext("{app} is not installed").replace("{app}", id),
            )
        })
}

/// Make the application with this id the default for one type, even when it
/// already is, so the choice is recorded as the user's own.
pub(crate) fn set_default(id: &str, mime: &str) -> Result<(), glib::Error> {
    find_app(id)?.set_as_default_for_type(mime)?;
    verify_default(id, mime)
}

/// Make the application with this id the default for the given types, skipping
/// the ones it already opens.
pub(crate) fn assign_types(id: &str, mimes: &[String]) -> Assignment {
    let mut outcome = Assignment {
        gained: 0,
        failed: 0,
        error: None,
    };
    let info = match find_app(id) {
        Ok(info) => info,
        Err(error) => {
            outcome.failed = mimes.len();
            outcome.error = Some(error);
            return outcome;
        }
    };
    let pending: Vec<&String> = mimes
        .iter()
        .filter(|mime| !already_default(id, mime))
        .collect();
    let mut refused = HashSet::new();
    for mime in &pending {
        // A subtype with no default of its own follows its supertype, which may
        // have been written already.
        if already_default(id, mime) {
            continue;
        }
        if let Err(error) = info.set_as_default_for_type(mime) {
            outcome.failed += 1;
            outcome.error.get_or_insert(error);
            refused.insert(*mime);
        }
    }
    for mime in pending.into_iter().filter(|mime| !refused.contains(mime)) {
        match verify_default(id, mime) {
            Ok(()) => outcome.gained += 1,
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
pub(crate) fn scoped_types(app: &AppEntry, group: Option<&str>) -> Vec<String> {
    app.types
        .iter()
        .filter(|mime| group.is_none_or(|group| media_group(mime) == group))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    /// Point GIO at a scratch configuration, so the tests never write to the
    /// user's own mimeapps.list.
    fn test_config_home() -> PathBuf {
        static INIT: Once = Once::new();
        // One per run, so two checkouts testing at once keep apart; `create_dir`
        // rather than `create_dir_all`, so a directory someone else left under that
        // name stops the run instead of receiving its writes.
        let scratch =
            std::env::temp_dir().join(format!("mimebind-test-config-{}", std::process::id()));
        INIT.call_once(|| {
            std::fs::create_dir(&scratch).unwrap();
            // SAFETY: no other test reads the environment; the ones that reach GIO
            // wait on this `Once` before they call into it.
            unsafe {
                std::env::set_var("XDG_CONFIG_HOME", &scratch);
                std::env::set_var("XDG_CURRENT_DESKTOP", "mimebind-test");
            }
        });
        // GLib reads the variable once and keeps the answer. Should anything have
        // asked before the line above, stop rather than reset the real associations.
        assert_eq!(
            glib::user_config_dir(),
            scratch,
            "GIO kept the real config dir"
        );
        scratch
    }

    /// Installed applications with their full set of openable types, as the window
    /// builds them.
    fn build_catalog() -> Vec<AppEntry> {
        let mut apps = installed_apps();
        let subtypes = subtypes_of(&declared_types(&apps));
        expand_supported_types(&mut apps, &subtypes);
        apps
    }

    /// An application that claims a supertype must be credited with the subtypes
    /// the MIME database derives from it, or an editor claiming `text/plain`
    /// looks like it cannot open source files.
    #[test]
    fn supported_types_follow_the_mime_database() {
        test_config_home();
        let declared = installed_apps();
        let mut expanded = installed_apps();
        expand_supported_types(&mut expanded, &subtypes_of(&declared_types(&declared)));

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

    /// An application removed while the window is open refuses every type with a
    /// reason, rather than reporting that it now opens nothing new.
    #[test]
    fn assigning_to_a_missing_application_fails_every_type() {
        test_config_home();
        let mimes = vec!["text/plain".to_string(), "image/png".to_string()];
        let outcome = assign_types("mimebind-no-such-application.desktop", &mimes);
        assert_eq!((outcome.gained, outcome.failed), (0, 2));
        assert!(outcome.error.is_some());
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
        assert!(!already_default(&widest.id, &stolen));

        let changing = widest
            .types
            .iter()
            .filter(|mime| !already_default(&widest.id, mime))
            .count();
        let every: Vec<String> = widest.types.iter().cloned().collect();
        let outcome = assign_types(&widest.id, &every);
        assert_eq!(outcome.failed, 0, "{:?}", outcome.error);
        // Subtypes that follow a supertype are never written, and still count.
        assert_eq!(outcome.gained, changing);
        for mime in &widest.types {
            assert_eq!(
                gio::AppInfo::default_for_type(mime, false).map(|a| a.id()),
                Some(widest.info.id()),
                "{mime} did not take the bulk default"
            );
        }

        // A desktop-specific list is read before mimeapps.list, so a default it
        // names wins over the one GIO writes, and neither write may claim success.
        let shadowed = widest.types.first().unwrap();
        std::fs::write(
            scratch.join("mimebind-test-mimeapps.list"),
            format!("[Default Applications]\n{shadowed}={}\n", thief.id),
        )
        .unwrap();
        let error = set_default(&widest.id, shadowed).unwrap_err();
        assert!(
            error.message().contains("mimebind-test-mimeapps.list"),
            "{error}"
        );
        let outcome = assign_types(&widest.id, std::slice::from_ref(shadowed));
        assert_eq!((outcome.gained, outcome.failed), (0, 1));

        for mime in user_overrides() {
            gio::AppInfo::reset_type_associations(&mime);
        }
        assert!(user_overrides().is_empty(), "bulk reset left overrides");

        std::fs::remove_dir_all(&scratch).ok();
    }
}
