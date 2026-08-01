# Install Zoi on your system

You can install Zoi using a package manager, an installer script, or by building it from source.

## Scripts

You can also use our installer scripts for a quick setup.

**Linux / macOS :**

```sh
curl -fsSL https://zillowe.pages.dev/scripts/zoi/install.sh | bash
```

Or if you want a truly safe way to run this script use [ZSM](https://zillowe.qzz.io/docs/zds/zsm).

```sh
curl -fsSL https://zillowe.pages.dev/scripts/zoi/install.zsm | zsm
```

**Windows:**

```powershell
powershell -c "irm zillowe.pages.dev/scripts/zoi/install.ps1|iex"
```

## Package Managers

### Linux

Install Zoi on Linux distros.

#### Arch Linux (AUR)

Install [`zoi-bin`](https://aur.archlinux.org/packages/zoi-bin) (Pre-compiled binary) or [`zoi`](https://aur.archlinux.org/packages/zoi) (built from source) from the AUR using your favorite helper (e.g. `yay`, `paru`):

```sh
paru -S zoi-bin
```

Or manually without any helpers:

```sh
git clone https://aur.archlinux.org/zoi-bin.git
cd zoi-bin
makepkg -si
```

#### Debian / Ubuntu (.deb)

Download the `.deb` package for your architecture from the [latest release](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/releases) and install it using `apt` or `dpkg`:

```sh
# Using apt (recommended, handles dependencies)
sudo apt install ./zoi-*.deb

# Using dpkg
sudo dpkg -i zoi-*.deb
```

#### Fedora / RHEL (.rpm)

Download the `.rpm` package for your architecture from the [latest release](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/releases) and install it using `dnf` or `rpm`:

```sh
# Using dnf (recommended)
sudo dnf install ./zoi-*.rpm

# Using rpm
sudo rpm -i zoi-*.rpm
```

#### Fedora (COPR)

Install `zoi` from Fedora [COPR](https://copr.fedorainfracloud.org/coprs/zillowez/zoi/) (Supports Fedora 43, 44, Rawhide, CentOS Stream 9/10, EPEL 9, and openSUSE Tumbleweed):

```sh
sudo dnf copr enable zillowez/zoi
sudo dnf install zoi
```

#### Fedora (Terra)

Install `zoi-rs` on Fedora from [Terra](https://terra.fyralabs.com) repo (not maintained by us so updates can be late, uses [Crates.io](#cratesio) version):

```sh
# add terra repo
dnf install --nogpgcheck --repofrompath 'terra,https://repos.fyralabs.com/terra$releasever' terra-release
# install Zoi
sudo dnf install zoi-rs
```

More information and instructions for using Terra are available [here](https://developer.fyralabs.com/terra/installing).

### macOS

Install Zoi on macOS.

#### Homebrew

Install Zoi using Homebrew (Also supports linux):

```sh
brew install zillowe/tap/zoi
```

### Windows

Install Zoi on Windows.

#### Scoop

Install Zoi using Scoop:

```powershell
scoop bucket add zillowe https://github.com/zillowe/scoop.git
scoop install zoi
```

### Crates.io

You can install `zoi` directly from [crates.io](https://crates.io/crates/zoi-rs) using `cargo`:

```sh
cargo install zoi-rs
```

### NPM

You can install `@zillowe/zoi` from `npm` also.

```sh
npx @zillowe/zoi
bunx @zillowe/zoi
pnpm dlx @zillowe/zoi
yarn dlx @zillowe/zoi
```

## Build from Source

If you prefer, you can build Zoi from source. You'll need [Rust](https://www.rust-lang.org) installed.

**Build the release binary:**

```sh
./configure
just build
```

**Install it locally:**

```sh
sudo just install
```
