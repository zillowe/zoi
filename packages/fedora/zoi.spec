# Maintainer: ZilloweZ <zillowez@gmail.com>

Name:           zoi
Version:        __VERSION__
Release:        1%{?dist}
Summary:        Advanced Package Manager & Environment Orchestrator

License:        Apache-2.0
URL:            https://gitlab.com/zillowe/zillwen/zusty/zoi
Source0:        %{url}/-/archive/Prod-Release-%{version}/Prod-Release-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gcc
BuildRequires:  openssl-devel
BuildRequires:  pkgconfig
BuildRequires:  xz-devel
BuildRequires:  git
BuildRequires:  clang
BuildRequires:  clang-devel
BuildREquires:  bubblewrap

Requires:       git
Requires:       gnupg

%description
Zoi is an advanced package manager and environment orchestrator, designed to simplify package management and environment configuration across multiple operating systems.

%prep
%setup -q

%build
cargo build --release --bin zoi

%install
install -D -m 755 target/release/zoi %{buildroot}%{_bindir}/zoi

mkdir -p %{buildroot}%{_datadir}/bash-completion/completions
mkdir -p %{buildroot}%{_datadir}/zsh/site-functions
mkdir -p %{buildroot}%{_datadir}/fish/vendor_completions.d

./target/release/zoi generate-completions bash > %{buildroot}%{_datadir}/bash-completion/completions/zoi
./target/release/zoi generate-completions zsh > %{buildroot}%{_datadir}/zsh/site-functions/_zoi
./target/release/zoi generate-completions fish > %{buildroot}%{_datadir}/fish/vendor_completions.d/zoi.fish

mkdir -p %{buildroot}%{_mandir}/man1
./target/release/zoi generate-manual
cp manuals/*.1 %{buildroot}%{_mandir}/man1/

%files
%license LICENSE
%doc README.md
%{_bindir}/zoi
%{_datadir}/bash-completion/completions/zoi
%{_datadir}/zsh/site-functions/_zoi
%{_datadir}/fish/vendor_completions.d/zoi.fish
%{_mandir}/man1/zoi*.1*

%changelog
* Wed Jul 08 2026 Zillowe Foundation <contact@zillowe.qzz.io> - 1.21.0-1
- Initial release for Fedora COPR
- Added shell completions and man pages
