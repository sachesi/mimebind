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

## Packages

Fedora 44, 45 and Rawhide, from the Copr project
[sachesi/software](https://copr.fedorainfracloud.org/coprs/sachesi/software/):

    sudo dnf copr enable sachesi/software
    sudo dnf install mimebind

openSUSE Tumbleweed and Slowroll, from the OBS project
[home:sachesi:software](https://build.opensuse.org/project/show/home:sachesi:software); for
Slowroll the address has `openSUSE_Slowroll` in it, and on aarch64 `openSUSE_Factory_ARM`:

    sudo zypper addrepo https://download.opensuse.org/repositories/home:sachesi:software/openSUSE_Tumbleweed/home:sachesi:software.repo
    sudo zypper install mimebind

Debian testing and Ubuntu 26.04, from the same OBS project; for Ubuntu the addresses
have `xUbuntu_26.04` in place of `Debian_Testing`:

    sudo install -d /etc/apt/keyrings
    curl -fsSL https://download.opensuse.org/repositories/home:sachesi:software/Debian_Testing/Release.key | sudo gpg --dearmor -o /etc/apt/keyrings/sachesi-software.gpg
    echo 'deb [signed-by=/etc/apt/keyrings/sachesi-software.gpg] https://download.opensuse.org/repositories/home:sachesi:software/Debian_Testing/ /' | sudo tee /etc/apt/sources.list.d/sachesi-software.list
    sudo apt update
    sudo apt install mimebind

Arch Linux: the AUR package `mimebind`, built from
[packaging/aur/PKGBUILD](packaging/aur/PKGBUILD), which each release tag updates.

The same packages are attached to each [release](https://github.com/sachesi/mimebind/releases).

## Building and installing

    just build
    sudo just install                   # prefix /usr/local
    just prefix=$HOME/.local build install

Build needs Rust 1.92, `blueprint-compiler`, `just`, gettext and the development packages
for GTK 4.12 and libadwaita 1.5 or newer. `just run` starts the debug build without
installing it. `just uninstall` removes what `install` put in place, with the same prefix.

The interface is available in English, Russian and Ukrainian.

- [Contributing](CONTRIBUTING.md), including where things are in the code, and
  [reporting a vulnerability](SECURITY.md)

GPL-3.0-or-later.
