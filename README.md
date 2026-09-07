# Mimebind

Mimebind shows which application opens each file type and link, and lets you change it.
It is written in Rust with GTK 4 and libadwaita, and writes associations through GIO to
`~/.config/mimeapps.list`, so the choice holds on any desktop that follows the
freedesktop.org MIME applications specification, not only GNOME.

A Default Apps page covers the web browser, mail, calendar, images, audio and video. The
full list can be browsed by application or by media group and searched by description or
MIME type; each row shows the application that opens the type now. An application can take
every type it supports, or one media group of them, in one step. An application is credited
with the subtypes the MIME database derives from what it declares, so an editor that claims
`text/plain` is listed for `text/rust` and `text/x-c++src`, the way GIO already resolves
them. Modified lists every association you set, each resettable on its own or all at once.

## Building and installing

    just build
    sudo just install                   # prefix /usr/local
    just prefix=$HOME/.local build install

Build needs Rust 1.92, `blueprint-compiler`, `just`, gettext and the development packages
for GTK 4.12 and libadwaita 1.5 or newer. `just run` starts the debug build without
installing it. `just uninstall` removes what `install` put in place, with the same prefix.

The interface is available in English, Russian and Ukrainian.

GPL-3.0-or-later.
