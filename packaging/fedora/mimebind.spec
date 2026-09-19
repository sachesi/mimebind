%define _debugsource_template %{nil}
%define debug_package %{nil}

%global app_id io.github.sachesi.mimebind

Name:           mimebind
Version:        0.2.0
Release:        3%{?dist}
Summary:        Choose which application opens which file type

License:        GPL-3.0-or-later
URL:            https://github.com/sachesi/mimebind
Source0:        %{url}/archive/refs/tags/v%{version}.tar.gz#/%{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust >= 1.92
BuildRequires:  gcc
BuildRequires:  blueprint-compiler
BuildRequires:  desktop-file-utils
BuildRequires:  gettext
BuildRequires:  appstream
BuildRequires:  pkgconfig(gtk4) >= 4.12
BuildRequires:  pkgconfig(libadwaita-1) >= 1.5

Requires:       gtk4%{?_isa} >= 4.12
Requires:       libadwaita%{?_isa} >= 1.5
Requires:       hicolor-icon-theme
Requires:       shared-mime-info

%description
Mimebind lists every MIME type an installed application can open, shows which
application currently handles it, and lets that choice be changed or reset.
Associations are read and written through GIO, which stores them in
~/.config/mimeapps.list as defined by the freedesktop.org MIME applications
specification, so the result applies on GNOME, KDE Plasma, Xfce, sway and
anything else that follows the spec.

Applications are credited with the subtypes the MIME database derives from what
they declare, so an editor claiming text/plain is listed for text/rust and
text/x-c++src too. One application can be made the default for every type it
supports or for a single media group of them, and every association set this
way is listed in one place and resettable.

%prep
%autosetup -n %{name}-%{version}

%build
export CARGO_HOME="$PWD/.cargo-home"
export RUSTFLAGS="%{?build_rustflags}"
export MIMEBIND_LOCALEDIR="%{_datadir}/locale"
%if 0%{?_cargo_target_dir:1}
export CARGO_TARGET_DIR="%{_cargo_target_dir}"
%endif
cargo build --release

%install
%if 0%{?_cargo_target_dir:1}
target="%{_cargo_target_dir}/release"
%else
target="target/release"
%endif
install -Dpm 0755 "$target/mimebind" %{buildroot}%{_bindir}/mimebind

install -d %{buildroot}%{_datadir}/applications %{buildroot}%{_metainfodir}
msgfmt --desktop --template=data/%{app_id}.desktop -d po \
  -o %{buildroot}%{_datadir}/applications/%{app_id}.desktop
msgfmt --xml --template=data/%{app_id}.metainfo.xml -d po \
  -o %{buildroot}%{_metainfodir}/%{app_id}.metainfo.xml
install -Dpm 0644 data/icons/hicolor/scalable/apps/%{app_id}.svg \
  %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/%{app_id}.svg
install -Dpm 0644 data/icons/hicolor/symbolic/apps/%{app_id}-symbolic.svg \
  %{buildroot}%{_datadir}/icons/hicolor/symbolic/apps/%{app_id}-symbolic.svg

for lang in $(cat po/LINGUAS); do
  install -d %{buildroot}%{_datadir}/locale/$lang/LC_MESSAGES
  msgfmt -o %{buildroot}%{_datadir}/locale/$lang/LC_MESSAGES/%{name}.mo po/$lang.po
done
%find_lang %{name}

%check
desktop-file-validate %{buildroot}%{_datadir}/applications/%{app_id}.desktop
appstreamcli validate --no-net %{buildroot}%{_metainfodir}/%{app_id}.metainfo.xml
test -x %{buildroot}%{_bindir}/mimebind

%files -f %{name}.lang
%license LICENSE
%doc README.md
%{_bindir}/mimebind
%{_datadir}/applications/%{app_id}.desktop
%{_metainfodir}/%{app_id}.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/%{app_id}.svg
%{_datadir}/icons/hicolor/symbolic/apps/%{app_id}-symbolic.svg

%changelog
