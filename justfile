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

# List the available recipes.
default:
    @just --list

# Release build.
build:
    cargo build --release

# Debug build.
build-debug:
    cargo build

# Run the debug build uninstalled.
run *args: build-debug
    target/debug/mimebind {{args}}

# Lints: rustfmt, clippy, tests, desktop file and metainfo validation.
check:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
    desktop-file-validate data/{{app_id}}.desktop
    appstreamcli validate --no-net data/{{app_id}}.metainfo.xml

# Install the binary, desktop entry, metainfo and icons.
install: build
    install -Dm755 target/release/mimebind {{bindir}}/mimebind
    install -Dm644 data/{{app_id}}.desktop {{datadir}}/applications/{{app_id}}.desktop
    install -Dm644 data/{{app_id}}.metainfo.xml {{datadir}}/metainfo/{{app_id}}.metainfo.xml
    install -Dm644 data/icons/hicolor/scalable/apps/{{app_id}}.svg {{datadir}}/icons/hicolor/scalable/apps/{{app_id}}.svg
    install -Dm644 data/icons/hicolor/symbolic/apps/{{app_id}}-symbolic.svg {{datadir}}/icons/hicolor/symbolic/apps/{{app_id}}-symbolic.svg

# Remove the installed files.
uninstall:
    rm -f {{bindir}}/mimebind
    rm -f {{datadir}}/applications/{{app_id}}.desktop
    rm -f {{datadir}}/metainfo/{{app_id}}.metainfo.xml
    rm -f {{datadir}}/icons/hicolor/scalable/apps/{{app_id}}.svg
    rm -f {{datadir}}/icons/hicolor/symbolic/apps/{{app_id}}-symbolic.svg

# Remove build artefacts.
clean:
    cargo clean
