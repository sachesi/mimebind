# Mimebind build and install tasks.
#
# `build` needs cargo and blueprint-compiler; `install` only copies what is already in
# target/release. The prefix is fixed at build time for the translations, so give the
# same one to both:
#
#   just build
#   sudo just install              (prefix /usr/local)
#   just prefix=$HOME/.local build install

set shell := ["bash", "-euo", "pipefail", "-c"]

app_id := "io.github.sachesi.mimebind"
prefix := env("PREFIX", "/usr/local")
destdir := env("DESTDIR", "")
bindir := destdir + prefix + "/bin"
datadir := destdir + prefix + "/share"
localedir := prefix + "/share/locale"
release := "target/release"
pot_dir := "target/pot"
check_dir := "target/check"
bugs := "https://github.com/sachesi/mimebind/issues"
version := `sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1`

default:
    @just --list

# Release build.
build:
    MIMEBIND_LOCALEDIR={{localedir}} cargo build --release

# Debug build.
build-debug:
    cargo build

# Compile every catalogue into target/locale for running uninstalled.
catalogs:
    for lang in $(cat po/LINGUAS); do mkdir -p target/locale/$lang/LC_MESSAGES; msgfmt -o target/locale/$lang/LC_MESSAGES/mimebind.mo po/$lang.po; done

# Run the debug build uninstalled, with translations from target/locale. The catalogue
# directory is fixed at build time, so this builds with it.
run *args: catalogs
    MIMEBIND_LOCALEDIR=$PWD/target/locale cargo build
    target/debug/mimebind {{args}}

# Lints: rustfmt, clippy, blueprint, desktop file and metainfo validation, catalogues.
check:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    mkdir -p {{check_dir}}
    blueprint-compiler batch-compile {{check_dir}} data/ui data/ui/*.blp >/dev/null
    desktop-file-validate data/{{app_id}}.desktop
    appstreamcli validate --no-net data/{{app_id}}.metainfo.xml
    for lang in $(cat po/LINGUAS); do msgfmt -c -o /dev/null po/$lang.po; done

# Unit tests. They write associations to a scratch XDG_CONFIG_HOME, never to yours.
test:
    cargo test

# Regenerate po/mimebind.pot from the Rust sources, the Blueprint files, the desktop
# entry and the metainfo.
pot:
    mkdir -p {{pot_dir}}/ui
    blueprint-compiler batch-compile {{pot_dir}}/ui data/ui data/ui/*.blp >/dev/null
    # xgettext has no Rust mode; the C lexer copes once lifetimes ('a, 'static) are stripped.
    rm -rf {{pot_dir}}/src && cp -r src {{pot_dir}}/src
    find {{pot_dir}}/src -name '*.rs' -exec sed -i -E "s/'([A-Za-z_][A-Za-z0-9_]*)([^'A-Za-z0-9_]|$)/\1\2/g" {} +
    xgettext --from-code=UTF-8 --package-name=mimebind --package-version={{version}} \
        --msgid-bugs-address={{bugs}} \
        --language=C --keyword= --keyword=gettext --keyword=ngettext:1,2 --keyword=N_ \
        --flag=gettext:1:no-c-format --flag=ngettext:1:no-c-format --flag=ngettext:2:no-c-format \
        --add-comments=Translators --sort-by-file --directory={{pot_dir}} -o po/mimebind.pot $(cd {{pot_dir}} && find src -name '*.rs' | sort)
    xgettext -j --from-code=UTF-8 --package-name=mimebind --package-version={{version}} --msgid-bugs-address={{bugs}} --add-comments=Translators --sort-by-file --directory={{pot_dir}} -o po/mimebind.pot $(cd {{pot_dir}} && ls ui/*.ui)
    xgettext -j --from-code=UTF-8 --package-name=mimebind --package-version={{version}} --msgid-bugs-address={{bugs}} --language=Desktop --sort-by-file -o po/mimebind.pot data/{{app_id}}.desktop
    xgettext -j --from-code=UTF-8 --package-name=mimebind --package-version={{version}} --msgid-bugs-address={{bugs}} --sort-by-file -o po/mimebind.pot data/{{app_id}}.metainfo.xml

# Merge the current template into every po/<lang>.po.
po: pot
    for lang in $(cat po/LINGUAS); do msgmerge --update --backup=none --quiet po/$lang.po po/mimebind.pot; done
    for lang in $(cat po/LINGUAS); do msgfmt --statistics -o /dev/null po/$lang.po; done

# Install the release build. Does not build: run `just build` first.
install:
    @test -x {{release}}/mimebind || { echo "error: {{release}}/mimebind missing; run 'just build' first" >&2; exit 1; }
    install -Dm755 {{release}}/mimebind {{bindir}}/mimebind
    mkdir -p {{datadir}}/applications {{datadir}}/metainfo
    msgfmt --desktop --template=data/{{app_id}}.desktop -d po -o {{datadir}}/applications/{{app_id}}.desktop
    msgfmt --xml --template=data/{{app_id}}.metainfo.xml -d po -o {{datadir}}/metainfo/{{app_id}}.metainfo.xml
    install -Dm644 data/icons/hicolor/scalable/apps/{{app_id}}.svg {{datadir}}/icons/hicolor/scalable/apps/{{app_id}}.svg
    install -Dm644 data/icons/hicolor/symbolic/apps/{{app_id}}-symbolic.svg {{datadir}}/icons/hicolor/symbolic/apps/{{app_id}}-symbolic.svg
    for lang in $(cat po/LINGUAS); do install -d {{datadir}}/locale/$lang/LC_MESSAGES; msgfmt -o {{datadir}}/locale/$lang/LC_MESSAGES/mimebind.mo po/$lang.po; done
    update-desktop-database -q {{datadir}}/applications || true
    gtk4-update-icon-cache -qtf {{datadir}}/icons/hicolor || gtk-update-icon-cache -qtf {{datadir}}/icons/hicolor || true
    @echo "installed to {{prefix}}"

uninstall:
    rm -f {{bindir}}/mimebind
    rm -f {{datadir}}/applications/{{app_id}}.desktop {{datadir}}/metainfo/{{app_id}}.metainfo.xml
    rm -f {{datadir}}/icons/hicolor/scalable/apps/{{app_id}}.svg {{datadir}}/icons/hicolor/symbolic/apps/{{app_id}}-symbolic.svg
    for lang in $(cat po/LINGUAS); do rm -f {{datadir}}/locale/$lang/LC_MESSAGES/mimebind.mo; done
    update-desktop-database -q {{datadir}}/applications || true
    # A cache that still lists the removed icons hides the same icons installed elsewhere.
    gtk4-update-icon-cache -qtf {{datadir}}/icons/hicolor || gtk-update-icon-cache -qtf {{datadir}}/icons/hicolor || true

# Remove build artefacts.
clean:
    cargo clean
