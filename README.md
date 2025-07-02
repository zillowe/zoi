<div align="center">
    <img width="120" height="120" hspace="10" alt="ZDS Logo" src="https://gitlab.com/Zillowe/Zillwen/Zusty/ZDS/-/raw/main/img/zds.png"/>
    <h1>Zoi</h1>
    Universal Package Manager & Environment Setup Tool
<br/>
More links
<br/>
<a href="https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/blob/main/SECURITY.md">Security</a>

</div>

<hr/>

## Overview

Zoi is a universal package manager and environment setup tool.
It aims to simplify package management and environment configuration for various operating systems.

Table of Contents

- [Overview](#overview)
- [Installation](#installation)
  - [Package Managers](#package-managers)
  - [Scripts](#scripts)
  - [Build](#build)
- [Update](#update)
- [Documentation](#documentation)
- [Footer](#footer)
  - [License](#license)

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

To build Zoi from source you need to have [`go`](https://go.dev) installed.

then run this command to build it:

```sh
# For Linux/macOS
./build/build-release.sh
# For Windows
./build/build-release.ps1
```

or using Go CLI:

```sh
go install https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi
```

## Update

You can update Zoi by running this command:

```sh
zoi update
```

## Documentation

To get started with Zoi please refer to the [Wiki](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/wikis/home).

## Footer

Zoi is developed by Zusty < Zillowe Foundation, part of the Zillowe Development Suite (ZDS)

### License

Zoi is licensed under the [Apache-2.0](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/blob/main/LICENSE) License.
