<div align="center">
    <img width="120" height="120" hspace="10" alt="ZDS Logo" src="https://gitlab.com/Zillowe/Zillwen/Zusty/ZDS/-/raw/main/img/zds.png"/>
    <h1>Zoi</h1>
    Universal Package Manager & Environment Setup Tool
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

You can install Zoi via [homebrew]()

```sh
brew tap Zillowe/zoi
brew install zoi
```


### Scripts

To install Zoi, you need to run this command:

```sh
# For Linux/macOS
curl -fsSL https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/app/install.sh | bash
# For Windows
powershell -c "irm gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/app/install.ps1|iex"
```

### Build

To build Zoi from source you need to have [`Rust`](https://www.rust-lang.org) installed.

Then run this command to build it:

```sh
# For Linux/macOS
./build/build-release.sh
# For Windows
./build/build-release.ps1
```

Then you can run Zoi locally:

```sh
./build/release/zoi
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

Zoi is licensed under the [Apache-2.0](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/blob/main/LICENSE) License.
