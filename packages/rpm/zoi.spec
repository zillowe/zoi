# Maintainer: ZilloweZ <zillowez@proton.me>
# mock_bootstrap: 0

%global debug_package %{nil}
%global _enable_debug_package 0
%global _debuginfo_subpackages 0
%undefine _debugsource_packages
%global _pkgverify_level none

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
BuildRequires:  rubygem-asciidoctor
BuildRequires:  openssl-devel
BuildRequires:  pkgconfig
BuildRequires:  xz-devel
BuildRequires:  git
BuildRequires:  clang
BuildRequires:  clang-devel
BuildRequires:  pkgconfig(openssl)
BuildRequires:  perl(FindBin)
BuildRequires:  perl(IPC::Cmd)
BuildRequires:  perl(File::Compare)
BuildRequires:  perl(File::Copy)
BuildRequires:  perl(lib)
BuildRequires:  perl(Time::Piece)

Requires:       git
Requires:       gnupg

Recommends: bubblewrap

%description
Zoi is an advanced package manager and environment orchestrator, designed to simplify package management and environment configuration across multiple operating systems.

%prep
%setup -q -c -T
tar -xf %{SOURCE0} --strip-components=1
cargo fetch --locked

%build
cargo build --release --locked --bin zoi

%install
install -D -m 755 target/release/zoi %{buildroot}%{_bindir}/zoi

mkdir -p %{buildroot}%{_datadir}/bash-completion/completions
mkdir -p %{buildroot}%{_datadir}/zsh/site-functions
mkdir -p %{buildroot}%{_datadir}/fish/vendor_completions.d

./target/release/zoi generate-completions bash > %{buildroot}%{_datadir}/bash-completion/completions/zoi
./target/release/zoi generate-completions zsh > %{buildroot}%{_datadir}/zsh/site-functions/_zoi
./target/release/zoi generate-completions fish > %{buildroot}%{_datadir}/fish/vendor_completions.d/zoi.fish

mkdir -p %{buildroot}%{_mandir}/man1
mkdir -p %{buildroot}%{_mandir}/man3
mkdir -p %{buildroot}%{_mandir}/man5
asciidoctor -b manpage -D %{buildroot}%{_mandir}/man1 man/zoi.adoc
asciidoctor -b manpage -D %{buildroot}%{_mandir}/man3 man/zoi-rs.adoc
asciidoctor -b manpage -D %{buildroot}%{_mandir}/man5 man/zoi-lua.adoc

%files
%license LICENSE
%doc README.md
%{_bindir}/zoi
%{_datadir}/bash-completion/completions/zoi
%{_datadir}/zsh/site-functions/_zoi
%{_datadir}/fish/vendor_completions.d/zoi.fish
%{_mandir}/man1/zoi.1*
%{_mandir}/man3/zoi-rs.3*
%{_mandir}/man5/zoi-lua.5*

%changelog
* Wed Jul 08 2026 Zillowe Foundation <contact@zillowe.qzz.io> - 1.21.0-1
- Initial release for Fedora COPR
- Added shell completions and man pages
