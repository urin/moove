Name:           moove
Version:        0.4.5
Release:        1%{?dist}
Summary:        Manipulate file names and locations using a text editor

# Dual-licensed: user may choose either license.
License:        MIT OR Apache-2.0
URL:            https://github.com/urin/moove
Source0:        %{url}/archive/refs/tags/v%{version}.tar.gz#/%{name}-%{version}.tar.gz

BuildRequires:  rust-packaging >= 21

%description
moove opens the list of files and directories in a text editor, allowing you
to rename or move them by editing the list directly. It supports renaming,
moving, copying, and deleting files using your preferred editor.

%prep
%autosetup -n %{name}-%{version}
%cargo_prep

%build
%cargo_build

%install
%cargo_install

%files
%license LICENSE-MIT LICENSE-APACHE
%doc README.md
%{_bindir}/moove

%changelog
* Mon May 04 2026 urin <urinkun@gmail.com> - 0.4.5-1
- Release v0.4.5
* Thu Jan 01 2026 urin <urin@urin.net> - 0.4.4-1
- Initial COPR package
