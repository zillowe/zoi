<div align="center">
    <img width="120" height="120" hspace="10" alt="ZDS Logo" src="https://gitlab.com/Zillowe/Zillwen/Zusty/ZDS/-/raw/main/img/zds.png"/>
    <h1>Zoi</h1>
    <p><strong>Universal Package Manager & Environment Setup Tool</strong></p>
</div>

<br/>

<div align="center">
  <a href="https://aur.archlinux.org/packages/zoi-bin">
    <img alt="AUR Version" src="https://img.shields.io/aur/version/zoi-bin?style=flat&logo=archlinux&logoColor=%23ffff&label=AUR&labelColor=5452f1&color=282696"/>
  </a>
  <a href="https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases">
    <img alt="GitLab Latest Release" src="https://img.shields.io/gitlab/v/release/Zillowe%2FZillwen%2FZusty%2FZoi?sort=date&display_name=release&style=flat&logo=gitlab&logoColor=%23fff&label=Latest%20Release&labelColor=%235452f1&color=%23282696"/>
  </a>
  <a href="https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/pipelines">
    <img alt="GitLab Pipeline Status" src="https://img.shields.io/gitlab/pipeline-status/Zillowe%2FZillwen%2FZusty%2FZoi?style=flat&logo=gitlab&logoColor=%23fff&label=Pipeline&labelColor=%235452f1&color=%23282696"/>
  </a>
  <a href="https://github.com/Zillowe/scoop">
    <img alt="Scoop Version" src="https://img.shields.io/scoop/v/zoi?bucket=https%3A%2F%2Fgithub.com%2FZillowe%2Fscoop&style=flat&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPCFET0NUWVBFIHN2ZyBQVUJMSUMgIi0vL1czQy8vRFREIFNWRyAxLjEvL0VOIiAiaHR0cDovL3d3dy53My5vcmcvR3JhcGhpY3MvU1ZHLzEuMS9EVEQvc3ZnMTEuZHRkIj4NCjwhLS0gVXBsb2FkZWQgdG86IFNWRyBSZXBvLCB3d3cuc3ZncmVwby5jb20sIFRyYW5zZm9ybWVkIGJ5OiBTVkcgUmVwbyBNaXhlciBUb29scyAtLT4KPHN2ZyB3aWR0aD0iODAwcHgiIGhlaWdodD0iODAwcHgiIHZpZXdCb3g9IjAgMCAyNCAyNCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIiBmaWxsPSIjZmZmZmZmIj4KDTxnIGlkPSJTVkdSZXBvX2JnQ2FycmllciIgc3Ryb2tlLXdpZHRoPSIwIi8%2BCg08ZyBpZD0iU1ZHUmVwb190cmFjZXJDYXJyaWVyIiBzdHJva2UtbGluZWNhcD0icm91bmQiIHN0cm9rZS1saW5lam9pbj0icm91bmQiLz4KDTxnIGlkPSJTVkdSZXBvX2ljb25DYXJyaWVyIj4gPHRpdGxlPm1pY3Jvc29mdDwvdGl0bGU%2BIDxyZWN0IHdpZHRoPSIyNCIgaGVpZ2h0PSIyNCIgZmlsbD0ibm9uZSIvPiA8cGF0aCBkPSJNMiwzaDl2OUgyVjNtOSwxOUgyVjEzaDl2OU0yMSwzdjlIMTJWM2g5bTAsMTlIMTJWMTNoOVoiLz4gPC9nPgoNPC9zdmc%2B&logoColor=%23ffff&label=Scoop&labelColor=%235452f1&color=%23282696"/>
  </a>
</div>

<br/>

<div align="center">
  <a href="https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/wikis/home"><strong>Wiki</strong></a> • 
  <a href="https://discord.gg/P4R7yaA3hf"><strong>Discord</strong></a> • 
  <a href="./SECURITY.md"><strong>Security</strong></a> • 
  <a href="./CODE_OF_CONDUCT.md"><strong>Code of Conduct</strong></a> • 
  <a href="https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/issues"><strong>Report an Issue</strong></a> 
</div>

<hr/>

<details>
<summary>Table of Contents</summary>

- [Features](#-features)
- [Getting Started](#-getting-started)
- [Installation](#-installation)
  - [Package Managers](#package-managers)
    - [Arch Linux (AUR)](#arch-linux-aur)
    - [macOS (Homebrew)](#macos-homebrew)
    - [Windows (Scoop)](#windows-scoop)
  - [Scripts](#-scripts)
  - [Build from Source](#%EF%B8%8F-build-from-source)
- [Usage](#-usage)
- [Contributing](#-contributing)
- [License](#-license)
  
</details>

Zoi is a universal package manager and environment setup tool, designed to simplify package management and environment configuration across multiple operating systems.

## ✨ Features

- **Universal:** Works on Linux, macOS, and Windows.
- **Repository-based:** Manage packages from different sources.
- **Environment Setup:** Configure project environments with ease.
- **Extensible:** Add your own repositories and packages.
- **Simple CLI:** An intuitive and easy-to-use command-line interface.

## 🚀 Getting Started

Getting started with Zoi is simple. Just follow these three steps:

1.  **Install Zoi:**
    Choose one of the [installation methods](#-installation) below.

2.  **Sync Repositories:**
    Before you can install packages, you need to sync the package repositories.
    ```sh
    zoi sync
    ```

3.  **Install a Package:**
    Now you can install any package you want. For example, to install `hello`:
    ```sh
    zoi install hello
    ```

## 📦 Installation

You can install Zoi using a package manager, an installer script, or by building it from source.

### Package Managers

#### Arch Linux (AUR)
Install `zoi-bin` from the AUR using your favorite helper (e.g. `yay`, `paru`):
```sh
yay -S zoi-bin
```

Or manually without any helpers:
```sh
git clone https://aur.archlinux.org/zoi-bin.git
cd zoi-bin
makepkg -si
```

#### macOS (Homebrew)
Install Zoi using [Homebrew](https://brew.sh):
```sh
brew install Zillowe/tap/zoi
```

#### Windows (Scoop)
Install Zoi using [Scoop](https://scoop.sh):
```powershell
scoop bucket add zillowe https://github.com/Zillowe/scoop.git
scoop install zoi
```

### 📜 Scripts

You can also use our installer scripts for a quick setup.

**Linux / macOS:**
```sh
curl -fsSL https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/app/install.sh | bash
```

**Windows:**
```powershell
powershell -c "irm gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/app/install.ps1|iex"
```

### 🛠️ Build from Source

If you prefer, you can build Zoi from source. You'll need [Rust](https://www.rust-lang.org) installed.

**Build the release binary:**
```sh
# For Linux/macOS
./build/build-release.sh

# For Windows
./build/build-release.ps1
```

**Install it locally:**
```sh
# For Linux/macOS
./configure
make
sudo make install
make install-completion # Install CLI completions
```

## 💡 Usage

Here are some common commands to get you started.

- **Install a package:**
  ```sh
  zoi install <package_name>
  ```

- **Uninstall a package:**
  ```sh
  zoi uninstall <package_name>
  ```

- **Install from a specific repository:**
  ```sh
  zoi install @<repo_name>/<package_name>
  ```

- **List all available packages from active repos:**
  ```sh
  zoi list all
  ```

- **List packages from a specific repo:**
  ```sh
  zoi list all @<repo_name>
  ```

- **Search for a package:**
  ```sh
  zoi search <term>
  ```

- **Add a new repository:**
  ```sh
  # Interactively
  zoi repo add
  
  # By name
  zoi repo add <repo_name>
  ```

- **Update Zoi to the latest version:**
  ```sh
  zoi upgrade
  ```

For more detailed information, please refer to the [**Wiki**](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/wikis/home).

## 🤝 Contributing

We welcome contributions from the community! If you'd like to contribute, please read our [**Contributing Guidelines**](./CONTRIBUTING.md) for more information.

## 📜 License

Zoi is licensed under the [Apache-2.0 License](./LICENSE).

<hr/>

<div align="center">
  <p>Zoi is developed by Zusty & Zillowe Foundation, part of the <a href="https://gitlab.com/Zillowe/Zillwen/Zusty/ZDS">Zillowe Development Suite (ZDS)</a></p>
</div>
