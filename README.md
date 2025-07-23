<div align="center">
    <img width="120" height="120" hspace="10" alt="ZDS Logo" src="https://gitlab.com/Zillowe/Zillwen/Zusty/ZDS/-/raw/main/img/zds.png"/>
    <h1>Zoi</h1>
    Universal Package Manager & Environment Setup Tool
</div>

<br/>

<div align="center">
  <a href="https://aur.archlinux.org/packages/zoi-bin">
    <img alt="AUR Version" src="https://img.shields.io/aur/version/zoi-bin?style=flat&logo=archlinux&logoColor=%23ffff&label=AUR&labelColor=5452f1&color=282696"/>
  </a> •
  <a href="https://github.com/Zillowe/scoop">
    <img alt="Scoop Version" src="https://img.shields.io/scoop/v/zoi?bucket=https%3A%2F%2Fgithub.com%2FZillowe%2Fscoop&style=flat&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPCFET0NUWVBFIHN2ZyBQVUJMSUMgIi0vL1czQy8vRFREIFNWRyAxLjEvL0VOIiAiaHR0cDovL3d3dy53My5vcmcvR3JhcGhpY3MvU1ZHLzEuMS9EVEQvc3ZnMTEuZHRkIj4NCjwhLS0gVXBsb2FkZWQgdG86IFNWRyBSZXBvLCB3d3cuc3ZncmVwby5jb20sIFRyYW5zZm9ybWVkIGJ5OiBTVkcgUmVwbyBNaXhlciBUb29scyAtLT4KPHN2ZyB3aWR0aD0iODAwcHgiIGhlaWdodD0iODAwcHgiIHZpZXdCb3g9IjAgMCAyNCAyNCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIiBmaWxsPSIjZmZmZmZmIj4KDTxnIGlkPSJTVkdSZXBvX2JnQ2FycmllciIgc3Ryb2tlLXdpZHRoPSIwIi8%2BCg08ZyBpZD0iU1ZHUmVwb190cmFjZXJDYXJyaWVyIiBzdHJva2UtbGluZWNhcD0icm91bmQiIHN0cm9rZS1saW5lam9pbj0icm91bmQiLz4KDTxnIGlkPSJTVkdSZXBvX2ljb25DYXJyaWVyIj4gPHRpdGxlPm1pY3Jvc29mdDwvdGl0bGU%2BIDxyZWN0IHdpZHRoPSIyNCIgaGVpZ2h0PSIyNCIgZmlsbD0ibm9uZSIvPiA8cGF0aCBkPSJNMiwzaDl2OUgyVjNtOSwxOUgyVjEzaDl2OU0yMSwzdjlIMTJWM2g5bTAsMTlIMTJWMTNoOVoiLz4gPC9nPgoNPC9zdmc%2B&logoColor=%23ffff&label=Scoop&labelColor=%235452f1&color=%23282696"/>
  </a>
</div>

<br/>

<div align="center">
  <a href="https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/wikis/home">Wiki</a> • 
  <a href="https://discord.gg/P4R7yaA3hf">Discord</a> • 
  <a href="./SECURITY.md">Security</a> • 
  <a href="./CODE_OF_CONDUCT.md">Code of Conduct</a> • 
  <a href="https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/issues">Report an Issue</a> 
</div>

<hr/>

<details>
<summary>Table of Contents</summary>

- [Overview](#overview)
- [Installation](#installation)
  - [Package Managers](#package-managers)
    - [AUR](#aur)
    - [Homebrew](#homebrew)
    - [Scoop](#scoop)
  - [Scripts](#scripts)
  - [Build](#build)
- [Update](#update)
- [Documentation](#documentation)
- [Footer](#footer)
  - [License](#license)
  
</details>

## Overview

Zoi is a universal package manager and environment setup tool.
It aims to simplify package management and environment configuration for various operating systems.

To start using Zoi first sync the packages

```sh
zoi sync
```

Then you can start installing a package, e.g. vim

```sh
zoi install vim
```

To uninstall a package

```sh
zoi uninstall vim
```

To add a repo

```sh
zoi repo list all # to list all the repos
zoi repo add <repo-name>
```

Or run this to bring the available repos

```sh
zoi repo add
```

To remove a repo run
```sh
zoi repo rm <repo-name> # or remove
```

## Installation

You can either build it from source or install it using installer scripts

### Package Managers

You can install Zoi via these package managers:

#### AUR

You can install Zoi via AUR helpers, like [yay](https://github.com/Jguer/yay)

```sh
yay -S zoi-bin
```

Or [paru](https://github.com/Morganamilo/paru)

```sh
paru -S zoi-bin
```

Or manually

```sh
git clone https://aur.archlinux.org/zoi-bin.git
cd zoi-bin
makepkg -si
```

#### Homebrew

You can install Zoi via [Homebrew](https://brew.sh) (MacOS only)

```sh
brew install Zillowe/tap/zoi
```

#### Scoop

You can install Zoi via [Scoop]()

First add Zillowe Scoop Bucket

```powershell
scoop bucket add zillowe https://github.com/Zillowe/scoop.git
```

Then install Zoi

```powershell
scoop install zoi
```

### Scripts

To install Zoi, you need to run this command:

For Linux/MacOS

```sh
curl -fsSL https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/app/install.sh | bash
```

For Windows

```powershell
powershell -c "irm gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/app/install.ps1|iex"
```

### Build

To build Zoi from source you need to have [`Rust`](https://www.rust-lang.org) installed.

Then run this command to build it:

For Linux/MacOS

```sh
./build/build-release.sh
```

For Windows

```sh
./build/build-release.ps1
```

Then you can run Zoi locally:

```sh
./build/release/zoi # or .exe if you're on Windows
```

If you want to install it to the current user:

```sh
cargo clean # if you have build it before

./configure
make
sudo make install
make install-completion # to install cli completion for your shell (bash, zsh or fish)
```

Or using Cargo CLI:

```sh
cargo install https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi
```

## Update

You can update Zoi by running this command:

```sh
zoi upgrade
```

## Documentation

To get started with Zoi please refer to the [Wiki](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/wikis/home).

## Footer

Zoi is developed by Zusty < Zillowe Foundation, part of the [Zillowe Development Suite (ZDS)](https://gitlab.com/Zillowe/Zillwen/Zusty/ZDS)

### License

Zoi is licensed under the [Apache-2.0](./LICENSE) License.
