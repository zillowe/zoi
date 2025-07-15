<div align="center">
    <img width="120" height="120" hspace="10" alt="ZDS Logo" src="https://gitlab.com/Zillowe/Zillwen/Zusty/ZDS/-/raw/main/img/zds.png"/>
    <h1>Zoi</h1>
    Universal Package Manager & Environment Setup Tool
</div>

<br/>

<p align="center">
  <a href="https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/wikis/home">Wiki</a> • 
  <a href="https://discord.gg/P4R7yaA3hf">Discord</a> • 
  <a href="./SECURITY.md">Security</a> • 
  <a href="./CODE_OF_CONDUCT.md">Code of Conduct</a> • 
  <a href="https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/issues">Report an Issue</a> • 
  <a href="https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/security/vulnerability_report">Report a Vulnerability</a>
</p>

<hr/>

## Overview

Zoi is a universal package manager and environment setup tool.
It aims to simplify package management and environment configuration for various operating systems.

<details>
<summary>Table of Contents</summary>

- [Overview](#overview)
- [Installation](#installation)
  - [Package Managers](#package-managers)
  - [Scripts](#scripts)
  - [Build](#build)
- [Update](#update)
- [Documentation](#documentation)
- [Footer](#footer)
  - [License](#license)
  
</details>

## Installation

You can either build it from source or install it using installer scripts

### Package Managers

You can install Zoi via these package managers:

```sh
# AUR
yay -Sy zoi # or paru
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

then run this command to build it:

```sh
# For Linux/macOS
./build/build-release.sh
# For Windows
./build/build-release.ps1
```

or using Cargo CLI:

```sh
cargo install https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi
```

## Update

You can update Zoi by running this command:

```sh
zoi update zoi
```

## Documentation

To get started with Zoi please refer to the [Wiki](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/wikis/home).

## Footer

Zoi is developed by Zusty < Zillowe Foundation, part of the [Zillowe Development Suite (ZDS)](https://gitlab.com/Zillowe/Zillwen/Zusty/ZDS)

### License

Zoi is licensed under the [Apache-2.0](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/blob/main/LICENSE) License.
