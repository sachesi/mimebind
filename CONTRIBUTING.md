# Contributing

Bugs and ideas go to the [issue tracker](https://github.com/sachesi/mimebind/issues);
security problems do not, see [SECURITY.md](SECURITY.md).

Before a change goes in:

- `just check` and `just test` pass. CI runs both on Fedora 44, with `cargo deny check`, for
  every push and pull request.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/):
  `fix:`, `feat:`, `perf:`, `docs:` and so on, with a subject that says what changed for
  someone using Mimebind.
- Every string the user sees goes through `gettext`. `just po` updates the catalogues in
  `po/`, and a change that adds strings brings their translations along where it can.

## Where things are

    build.rs            runs blueprint-compiler and bundles the GResource
    data/ui/*.blp       the window and a row of the file type list
    src/main.rs         the application: its actions, the About dialog, one window
    src/window.rs       the window: sidebar selection, filtering, the win.* actions
    src/mime_row.rs     a row of the list, recycled by the list view
    src/rows.rs         the sidebar rows and the section headers of the list
    src/dialogs.rs      choosing an application, and the confirmations for bulk changes
    src/catalog.rs      installed applications and their types, the user's overrides,
                        bulk assignment
    src/category.rs     the Default Apps categories and which application owns each
    src/entry.rs        MimebindMimeEntry, one content type in the list model
    src/i18n.rs         gettext setup

GIO owns `mimeapps.list`: every write goes through `GAppInfo`, and the file is only read
directly to tell the user's own choices from the system's, which GIO has no call for.

The list is a `GtkListView` over a `GListStore` of `MimebindMimeEntry`, filtered and sorted
in the Blueprint file. Changing one default replaces that entry with a new object, because
`items-changed` over the same object lets the view keep the old row.

## Running

    just run            # debug build, translations from target/locale
    just check          # fmt, clippy -D warnings, blueprint, validators, catalogues
    just test           # the unit tests
    cargo deny check    # advisories, licences and sources of the dependencies

The tests read the applications installed on the machine and need at least two that
declare a common type. They write associations to a scratch `XDG_CONFIG_HOME` under the
temporary directory, and stop before writing if GLib has already settled on your own.

User-visible strings use `{name}` placeholders filled in with `str::replace`; counts use
`ngettext` even where English would not need it, because the plural rules of other
languages do. `just pot` regenerates the template from the Rust sources, the Blueprint
files, the desktop entry and the metainfo, and `just po` merges it into every
`po/<lang>.po`. A new language is a new line in `po/LINGUAS` plus the `.po` file. `install`
compiles the catalogues and merges the desktop and metainfo translations with `msgfmt`.
