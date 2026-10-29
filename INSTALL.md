# Install

You can install Zoi using a package manager, an installer script, or by building it from source.

## Scripts

You can use our installer scripts for a quick setup.

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

#### Nix / NixOS

Install or run Zoi directly with Nix flakes, or include it in your NixOS configuration:

**Run directly:**

```sh
nix run gitlab:zillowe/zillwen/zusty/zoi -- --version
```

**Install to user profile:**

```sh
nix profile install gitlab:zillowe/zillwen/zusty/zoi
```

**In your NixOS `flake.nix`:**

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    zoi.url = "gitlab:zillowe/zillwen/zusty/zoi";
  };

  outputs = { self, nixpkgs, zoi, ... }: {
    nixosConfigurations.myhostname = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        ({ pkgs, ... }: {
          environment.systemPackages = [
            zoi.packages.''${pkgs.system}.default
          ];
        })
      ];
    };
  };
}
```

Or via overlay:

```nix
nixpkgs.overlays = [ zoi.overlays.default ];
environment.systemPackages = [ pkgs.zoi ];
```

#### Arch Linux (AUR)

Install [`zoi-bin`](https://aur.archlinux.org/packages/zoi-bin)
(Pre-compiled binary) or [`zoi`](https://aur.archlinux.org/packages/zoi)
(built from source) from the AUR using your favorite helper (e.g. `yay`, `paru`):

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

Download the `.deb` package for your architecture from the
[latest release](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/releases)
and install it using `apt` or `dpkg`:

```sh
# Using apt (recommended, handles dependencies)
sudo apt install ./zoi-*.deb

# Using dpkg
sudo dpkg -i zoi-*.deb
```

Or you can add our official repository on the [OpenSUSE Build Service (OBS)](https://build.opensuse.org/package/show/home:zillowez/zoi)
(Supports Debian Unstable, Debian Testing, Debian 13, and Ubuntu on `x86_64` and `aarch64`):

**Debian Unstable (Sid):**

```sh
sudo install -d -m 0755 /etc/apt/keyrings
curl -fsSL https://download.opensuse.org/repositories/home:/zillowez/Debian_Unstable/Release.key | gpg --dearmor | sudo tee /etc/apt/keyrings/home_zillowez.gpg > /dev/null
echo "deb [signed-by=/etc/apt/keyrings/home_zillowez.gpg] https://download.opensuse.org/repositories/home:/zillowez/Debian_Unstable/ ./" | sudo tee /etc/apt/sources.list.d/home_zillowez.list
sudo apt update
sudo apt install zoi
```

**Debian Testing:**

```sh
sudo install -d -m 0755 /etc/apt/keyrings
curl -fsSL https://download.opensuse.org/repositories/home:/zillowez/Debian_Testing/Release.key | gpg --dearmor | sudo tee /etc/apt/keyrings/home_zillowez.gpg > /dev/null
echo "deb [signed-by=/etc/apt/keyrings/home_zillowez.gpg] https://download.opensuse.org/repositories/home:/zillowez/Debian_Testing/ ./" | sudo tee /etc/apt/sources.list.d/home_zillowez.list
sudo apt update
sudo apt install zoi
```

**Debian 13 (Trixie):**

```sh
sudo install -d -m 0755 /etc/apt/keyrings
curl -fsSL https://download.opensuse.org/repositories/home:/zillowez/Debian_13/Release.key | gpg --dearmor | sudo tee /etc/apt/keyrings/home_zillowez.gpg > /dev/null
echo "deb [signed-by=/etc/apt/keyrings/home_zillowez.gpg] https://download.opensuse.org/repositories/home:/zillowez/Debian_13/ ./" | sudo tee /etc/apt/sources.list.d/home_zillowez.list
sudo apt update
sudo apt install zoi
```

**Ubuntu (26.04 LTS & 26.10):**

```sh
# For Ubuntu 26.04:
sudo install -d -m 0755 /etc/apt/keyrings
curl -fsSL https://download.opensuse.org/repositories/home:/zillowez/xUbuntu_26.04/Release.key | gpg --dearmor | sudo tee /etc/apt/keyrings/home_zillowez.gpg > /dev/null
echo "deb [signed-by=/etc/apt/keyrings/home_zillowez.gpg] https://download.opensuse.org/repositories/home:/zillowez/xUbuntu_26.04/ ./" | sudo tee /etc/apt/sources.list.d/home_zillowez.list
sudo apt update
sudo apt install zoi

# For Ubuntu 26.10:
sudo install -d -m 0755 /etc/apt/keyrings
curl -fsSL https://download.opensuse.org/repositories/home:/zillowez/xUbuntu_26.10/Release.key | gpg --dearmor | sudo tee /etc/apt/keyrings/home_zillowez.gpg > /dev/null
echo "deb [signed-by=/etc/apt/keyrings/home_zillowez.gpg] https://download.opensuse.org/repositories/home:/zillowez/xUbuntu_26.10/ ./" | sudo tee /etc/apt/sources.list.d/home_zillowez.list
sudo apt update
sudo apt install zoi
```

#### Fedora / RHEL (.rpm)

Download the `.rpm` package for your architecture from the
[latest release](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/releases)
and install it using `dnf` or `rpm`:

```sh
# Using dnf (recommended)
sudo dnf install ./zoi-*.rpm

# Using rpm
sudo rpm -i zoi-*.rpm
```

Or you can install Zoi from Fedora
[COPR](https://copr.fedorainfracloud.org/coprs/zillowe/zoi/)
(Supports Fedora 43, 44, 45, Rawhide, and EPEL 10 currently):

```sh
sudo dnf copr enable zillowe/zoi
sudo dnf install zoi
```

Or you can install `zoi-rs` on Fedora from [Terra](https://terra.fyralabs.com) repo:

```sh
# add terra repo
sudo dnf install --nogpgcheck --repofrompath 'terra,https://repos.fyralabs.com/terra$releasever' terra-release
# install Zoi
sudo dnf install zoi-rs
```

More information and instructions for using Terra are available
[here](https://developer.fyralabs.com/terra/installing).

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

## Build from Source

If you prefer, you can build Zoi from source.
You'll need [Rust](https://www.rust-lang.org) installed.

**Build the release binary:**

```sh
./configure
just build
```

**Install it locally:**

```sh
sudo just install
```
