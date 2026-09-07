# Mimebind build and install tasks.
#
#   just build
#   sudo just install              (prefix /usr/local)
#   just prefix=$HOME/.local install

set shell := ["bash", "-euo", "pipefail", "-c"]

app_id := "io.github.sachesi.mimebind"
prefix := env("PREFIX", "/usr/local")
destdir := env("DESTDIR", "")
bindir := destdir + prefix + "/bin"
datadir := destdir + prefix + "/share"
localedir := prefix + "/share/locale"
version := shell("sed -n 's/^version = \"\\(.*\\)\"/\\1/p' Cargo.toml | head -n1")

# List the available recipes.
default:
    @just --list

# Release build.
build:
    MIMEBIND_LOCALEDIR={{ localedir }} cargo build --release

# Debug build.
build-debug:
    cargo build

# Run the debug build uninstalled, with translations from target/locale.
run *args: build-debug catalogs
    MIMEBIND_LOCALEDIR=$PWD/target/locale target/debug/mimebind {{args}}

# Compile every po file into target/locale for running uninstalled.
catalogs:
    for lang in $(cat po/LINGUAS); do \
      mkdir -p target/locale/$lang/LC_MESSAGES; \
      msgfmt -o target/locale/$lang/LC_MESSAGES/mimebind.mo po/$lang.po; \
    done

# Regenerate po/mimebind.pot from the sources and merge it into every po file.
pot:
    # xgettext has no Rust mode; the C lexer copes once lifetimes are stripped.
    rm -rf target/pot && cp -r src target/pot
    find target/pot -name '*.rs' -exec sed -i -E "s/'([A-Za-z_][A-Za-z0-9_]*)([^'A-Za-z0-9_]|$)/\1\2/g" {} +
    xgettext --from-code=UTF-8 --language=C --add-comments=Translators --sort-by-file \
      --package-name=mimebind --package-version={{ version }} \
      --msgid-bugs-address=https://github.com/sachesi/mimebind/issues \
      --keyword= --keyword=gettext --keyword=ngettext:1,2 --keyword=N_ \
      -o po/mimebind.pot $(cd target/pot && ls *.rs | sed 's|^|target/pot/|')
    for lang in $(cat po/LINGUAS); do msgmerge -U --backup=none po/$lang.po po/mimebind.pot; done

# Lints: rustfmt, clippy, tests, desktop file and metainfo validation.
check:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
    desktop-file-validate data/{{app_id}}.desktop
    appstreamcli validate --no-net data/{{app_id}}.metainfo.xml
    for lang in $(cat po/LINGUAS); do msgfmt -c -o /dev/null po/$lang.po; done

# Install the binary, desktop entry, metainfo and icons.
install: build
    install -Dm755 target/release/mimebind {{bindir}}/mimebind
    install -Dm644 data/{{app_id}}.desktop {{datadir}}/applications/{{app_id}}.desktop
    install -Dm644 data/{{app_id}}.metainfo.xml {{datadir}}/metainfo/{{app_id}}.metainfo.xml
    install -Dm644 data/icons/hicolor/scalable/apps/{{app_id}}.svg {{datadir}}/icons/hicolor/scalable/apps/{{app_id}}.svg
    install -Dm644 data/icons/hicolor/symbolic/apps/{{app_id}}-symbolic.svg {{datadir}}/icons/hicolor/symbolic/apps/{{app_id}}-symbolic.svg
    for lang in $(cat po/LINGUAS); do \
      install -d {{datadir}}/locale/$lang/LC_MESSAGES; \
      msgfmt -o {{datadir}}/locale/$lang/LC_MESSAGES/mimebind.mo po/$lang.po; \
    done

# Remove the installed files.
uninstall:
    rm -f {{bindir}}/mimebind
    rm -f {{datadir}}/applications/{{app_id}}.desktop
    rm -f {{datadir}}/metainfo/{{app_id}}.metainfo.xml
    rm -f {{datadir}}/icons/hicolor/scalable/apps/{{app_id}}.svg
    rm -f {{datadir}}/icons/hicolor/symbolic/apps/{{app_id}}-symbolic.svg
    for lang in $(cat po/LINGUAS); do rm -f {{datadir}}/locale/$lang/LC_MESSAGES/mimebind.mo; done

# Remove build artefacts.
clean:
    cargo clean
