# Mimebind

A GTK4 and libadwaita application for viewing and changing which application opens each file
type. Mimebind writes associations through GIO, so they apply on desktops that follow the
freedesktop.org MIME applications specification.

## Features

- Choose defaults for web links, mail, calendars, images, audio and video
- Browse and search by application, media group, description or MIME type
- See, change and reset the application assigned to each type
- Assign one application to every supported type or to one media group
- Review and reset user-defined associations
- Adaptive layout for narrow windows

## Build and install

Requires [just](https://github.com/casey/just), Rust 1.88 or newer, GTK 4.12 or newer and
libadwaita 1.5 or newer with their development packages.

```sh
just build
sudo just install                  # prefix /usr/local
just prefix=$HOME/.local install
```

Run without installing:

```sh
just run
```

## Checks

```sh
just check
```

This runs rustfmt, Clippy, the test suite and the desktop and AppStream validators.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
