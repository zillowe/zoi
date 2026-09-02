# Changelog

You can install any of these versions: `zoi upgrade --tag --force <tag>`

To install Zoi: `curl -fsSL https://zillowe.pages.dev/scripts/zoi/install.sh | bash`, [more installation methods](https://zillowe.qzz.io/docs/zds/zoi).

## [Prod. Release 1.26.1] - 2026-08-26

### ♻️ Refactor

- [`af9c3cca`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/af9c3ccac3eca55a0f7a45ca0e26817627fc1062) Update binary installation paths to use standard local bin directories
- [`69e7040e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/69e7040ef8d2fd449d7a05a85146ebd7badd1c57) *(cli)* Consolidate clean command into cache clear

### 🩹 Bug Fixes

- [`1f3472f0`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1f3472f0981b8c00f739706bd8b5fb2292f07369) *(package)* Relocate ELFs before pooling to ensure manifest consistency

## [Prod. Release 1.26.0] - 2026-08-25

### ⏩ Merged

- [`cbf12732`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/cbf127329177ffc85a037dccb685d5b0c87e3d65) Branch 'minor-release' into 'main'

### ♻️ Refactor

- [`b3512413`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b3512413a802b82409814a96538ef3efef3ad474) Enhance packaging security, service management, and directory copying

## [Prod. Release 1.25.5] - 2026-08-22

### ⏩ Merged

- [`a04d45c0`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a04d45c09a406c13ac5dd29be8354cd47d4f58b5) Branch 'fix-zoi' into 'main'

### 🛡️ Dependencies

- [`773e738c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/773e738c5dd8ce1ffeaa01f4f7e834cfc9b4f57b) Update zbsdiff to v1.5.3

## [Prod. Release 1.25.2] - 2026-08-16

### 🛠️ Build

- [`ca56aa18`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ca56aa18125fc88dcf112ef872d7e0b1ae65d3ef) Fix COPR RPM spec

## [Prod. Release 1.25.0] - 2026-08-15

### ⏩ Merged

- [`13a1f035`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/13a1f03529506a7ba8575e4a3011ee45086d429d) Branch 'renovate/clap_mangen-0.x' into 'main'
- [`8311032f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8311032f77c9013d53a9f675ce4f94fab2c0d91b) Branch 'renovate/serde_yaml-0.x' into 'main'
- [`67804c1d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/67804c1dbcfb09e6271cd9dee30285cefec9a385) Branch 'renovate/comfy-table-8.x' into 'main'
- [`15d4b5ac`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/15d4b5ac5c00d1b71b0a4d069e33bfcc808e97e8) Branch 'renovate/spdx-0.x' into 'main'
- [`ce1b2dfd`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ce1b2dfddbdc20d6c4660319c85b51cb8fb9b4e0) Branch 'renovate/thiserror-2.x' into 'main'
- [`27a3ec28`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/27a3ec289e228ed6f846e1353e70664a5104f161) Branch 'renovate/rusqlite-0.x' into 'main'
- [`914ec818`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/914ec81875ecd3c8ade503d9a1be787395362036) Branch 'renovate/ignore-0.x' into 'main'
- [`c9f57849`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/c9f57849f7a90f7e346f7116615f069db4292f33) Branch 'renovate/clap-4.x' into 'main'
- [`17593df9`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/17593df9ba473bdd18c72b1d7c1a75a024999dc2) Branch 'renovate/base64-0.x' into 'main'

### ♻️ Refactor

- [`acbea5f8`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/acbea5f899f9af5011205d4fa505cf61b528a8aa) *(core)* Migrate from bsdiff to zbsdiff for delta upgrades

### ✨ Features

- [`2d6eb52e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/2d6eb52e4bdab68fa30ac44ba5e1e9f1b2006274) *(system)* Add generation pinning and ZoiSEC key export/import
- [`dd0a3128`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/dd0a31283fd853424bcd60659d38647c4d94018d) *(license)* Implement SPDX expression evaluation for license policies
- [`d430c0fc`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/d430c0fcd8d5e2a21755a2b4e099afbb8f941066) *(core)* Implement ZoiOS-only package constraints

### 🎨 Styling

- [`82ecad4f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/82ecad4f69730a50255886a890f3ac18047d6f0b) Update Codeberg repo URL
- [`2faadd16`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/2faadd161120f6c2aba5cfa36463fefa5fab21be) Remove trailing comma
- [`b60615a2`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b60615a29d6c5d965d71e2358978a6a0301c0d54) Add lint and fmt rules
- [`57c1ee71`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/57c1ee711e10d4f99ab82a7e031f298a896bdc2f) Format using rustfmt
- [`03c02088`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/03c0208850196fb8b07b64016b5c17b74337296a) Adhere to Clippy style

### 🔧 Configuration

- [`a18dc5e9`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a18dc5e9ea11bbb3045e4720cec1936162fdee21) *(gct)* Upgrade Gemini model version in configuration
- [`3d9b9e95`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/3d9b9e95f8369664e496029eccc1916972fbd43d) Update .editorconfig file
- [`f80c0164`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f80c016403302ee7996e8ee66c635e23f8a579f4) Add .editorconfig file

### 🛠️ Build

- [`fcb66526`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/fcb66526de7914b9547c3cb62dc04533a980cdff) Update zoi.lua

### 🛡️ Dependencies

- [`adf9dac4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/adf9dac420e9a085ee2f8c7ced7aa4dffdd7fdd3) Update
- [`91d5d200`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/91d5d2001baabd7185305cd1795bcba53d480c93) Update Rust crate clap_mangen to 0.3.3
- [`24bdb341`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/24bdb34170dcea8b7a74449b699acffd18519047) Update Rust crate serde_yaml to 0.10.6
- [`4455e77a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/4455e77a70793eba570737d472627595d09798d0) Update code to comfy_table v8
- [`fc73a0e2`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/fc73a0e2d8b7191c3e6b1557678b2dc55e2e4869) Update Rust crate clap_complete to 4.6.9
- [`7a7e1e8d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/7a7e1e8da337519d0231e742dfa4241a52fbb76e) Update Rust crate comfy-table to v8
- [`7c9db58b`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/7c9db58b1817339baf9d268739bd1f68cc59237a) Update Rust crate spdx to 0.13.5
- [`00863f0f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/00863f0fabc6664547e3d79fac97e96978da40e6) Update Rust crate thiserror to 2.0.20
- [`9ebae7be`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/9ebae7be13255d3a698deadb04131529b7abb121) Update Rust crate rusqlite to 0.40.2
- [`9ba40801`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/9ba40801d085d1acc79d1a24717d7651e73d4e35) Update Rust crate ignore to 0.4.33
- [`2e10a3cc`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/2e10a3cc3a6306c14c14f1c2691c98c2642e43eb) Update Rust crate clap_mangen to 0.3.2
- [`143550df`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/143550df41c884b34d49f730da4295b3be3c02ee) Update Rust crate clap to 4.6.6
- [`dc7de0a4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/dc7de0a4214c4919fd7bc38905a7f1daadb53f0a) Update Rust crate base64 to 0.23.1

### 🧹 Cleanup

- [`8b356718`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8b356718b696417c8c3a1c4d4c30fac6218c3dd5) Remove NPM Zoi installer completely

## [Prod. Release 1.24.3] - 2026-08-01

### 🛡️ Dependencies

- [`e6e3aeeb`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e6e3aeebe846b5e40da97ce5d41a762e9f447d5e) Update

## [Prod. Release 1.24.1] - 2026-08-01

### ⏩ Merged

- [`113b0c51`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/113b0c51b381055bd97e3de050162e0da2c1928d) Branch 'renovate/clap-4.x' into 'main'
- [`aface922`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/aface922fe071453d5369ee51124fcfe5d9ea67b) Branch 'renovate/toml-1.x' into 'main'

### ✨ Features

- [`c3c98127`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/c3c98127e71756aad7f85417039f13af6dba7e13) *(zoios)* Add support for package options and optionals
- [`52bf8552`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/52bf85525a94f9ff21bacd1881e0ff846995648a) *(core)* Add support for doas as privilege escalator
- [`8ba65e63`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8ba65e63a32e8cde1f68ce72d3a019d156f7d001) *(lua)* Add init system detection to system environment

### 🛡️ Dependencies

- [`75131a87`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/75131a87b3f369d9f2dd18be89fa98707904b9bb) Update Rust crate clap to 4.6.5
- [`bfd7f4dd`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/bfd7f4dd1d3a1ff3b27097f4139d4818c24de235) Update Rust crate toml to 1.1.4

### 🩹 Bug Fixes

- [`5636ed73`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/5636ed73940e4a16fbba7367a780415fa7b58276) *(upgrade)* Handle release and stable tag formats

## [Prod. Release 1.24.0] - 2026-07-30

### ⏩ Merged

- [`bb839fc9`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/bb839fc96ccfea60890bb4490f9093565d713ed9) Branch 'renovate/clap_complete-4.x' into 'main'
- [`369f42a6`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/369f42a6c4698297973c59638773449a771e113a) Branch 'renovate/base64-0.x' into 'main'
- [`67a1c2c7`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/67a1c2c7c7b3b9e28377e5d755acc37da46c1f42) Branch 'renovate/serde-monorepo' into 'main'
- [`1bdca07a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1bdca07a24bd44d91583c530a5cafd8a9e4dc203) Branch 'renovate/diffy-0.x' into 'main'
- [`44c739f7`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/44c739f7a363c666c67c23030951dd18396c99d8) Branch 'renovate/clap-4.x' into 'main'
- [`1cc887ab`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1cc887ab0c5d032051c5e9bcffcd0923ad2b428d) Branch 'renovate/anyhow-1.x' into 'main'
- [`85f68454`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/85f6845486796ef03839c88057202d33a9c3cea3) Branch 'renovate/ignore-0.x' into 'main'
- [`bb9d14d8`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/bb9d14d8f153218c48444a14c1bf947e6fcf2ea3) Branch 'renovate/glob-0.x' into 'main'
- [`994379b9`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/994379b90a825a0e73381df292478db6503b872d) Branch 'renovate/serde_json-1.x' into 'main'
- [`40f8bb1f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/40f8bb1f04cd4e94b2860d86e4be730033a1f8e3) Branch 'renovate/thiserror-2.x' into 'main'

### ♻️ Refactor

- [`5ea382c3`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/5ea382c373a310caf893cc77bd74ecd95a9ce5ee) *(system)* Improve cross-platform compilation support
- [`8e52cf84`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8e52cf8475c2ee0800842a7c0e3d01b9862a1b07) *(core)* Update git source identifier syntax

### ✨ Features

- [`730e92ac`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/730e92ac7da017adfabd1c805c9554259f63a52e) *(pkg)* Replace remote manual spec with zman helper
- [`87477398`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8747739831b406df2d84fe8d2400c6a2ab3414ca) *(lua)* Add UTILS.DOWNLOAD utility with hash verification
- [`94b8cddf`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/94b8cddf37432a5af95d9e35b5c36de5e8a08221) *(api)* Implement persistent shell sessions for cmd()
- [`fae911c5`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/fae911c5e3a04c1451e81b98238f7fe6dbf89a22) Implement package epochs and test dependency support
- [`964a74ba`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/964a74ba172208f679731b987593af24e9e40a7f) *(lua)* Support .zpa and .zsa archive formats
- [`65eec4b2`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/65eec4b2dcb3a2d1f926922799972ad17bf10581) *(package)* Add pure build mode with isolated sysroot
- [`06598001`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/065980015f1fbc8c01ca8a7cb78165a9fc788d84) *(system)* Implement bootstrap diagnostics and improve chroot isolation
- [`438605f8`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/438605f8e101176d63915577cf3454e29cc55e7b) *(pkg)* Improve backup file handling and add progress tracking
- [`a6e52fa3`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a6e52fa35f5ff6947221ad3d6050978b90a7e73c) *(daemon)* Integrate zoid daemon into build and packaging
- [`d572d707`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/d572d70770e6414a449cec59328a592413b6558a) *(sandbox)* Implement sysroot execution via bubblewrap
- [`ac0130e3`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ac0130e3de5f74d32e0020c1f11d76e7b9c9835d) *(system)* Transition to transaction-based OS management

### 🏗️ Structure

- [`cb8ae4d0`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/cb8ae4d016724014965ffad9592082f4a2842b8d) Reorganize core path and home directory logic

### 🔧 Configuration

- [`f5bfd151`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f5bfd1519e81fcdcd1854b5190bc4fbe5450ca7f) *(build)* Add shebang to Justfile

### 🛡️ Dependencies

- [`6815c780`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6815c78087eee1d4e5cd4d0d685acfa58390532d) Update Rust crate clap_complete to 4.6.8
- [`7cb82f5a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/7cb82f5a5cd3098bec3bfa678be760583c68a04f) Update Rust crate base64 to 0.23.0
- [`fffbd06d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/fffbd06d63134328bfdd5b52c0759fefba81e6a6) Update Rust crate serde to 1.0.229
- [`78b9913a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/78b9913a6748031773d54b15c73959ded2d05ef6) Update Rust crate diffy to 0.5.1
- [`023d18e8`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/023d18e87d318c8e9007f31cb9d1e0581c8ba38c) Update Rust crate clap to 4.6.4
- [`2905ebe7`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/2905ebe716f68978470f3bb86a159b9d2cae61df) Update Rust crate thiserror to 2.0.19
- [`50af1e38`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/50af1e38419f9bc3fd8738a27cdd54515d8489bb) Update Rust crate serde_json to 1.0.151
- [`1db79c44`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1db79c445d63a7cae6e8034965ba056511ce9abb) Update Rust crate glob to 0.3.4
- [`3ccdcefb`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/3ccdcefbb6b1cacebf721eba51047589fbf2eec1) Update Rust crate ignore to 0.4.31
- [`dfcfe887`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/dfcfe887ea2ca42245229b14b17ad7fa353dd82e) Update Rust crate clap to 4.6.3
- [`160566bb`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/160566bb9636d409156f6fa9cd3dabd2e4fd543f) Update Rust crate anyhow to 1.0.104

### 🧹 Cleanup

- [`cb90355c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/cb90355c67937a8570f803964bc2d5faf630ce2a) *(distro)* Simplify system diagnostics and improve error reporting

### 🩹 Bug Fixes

- [`e55d5218`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e55d52182ced89d2f5f2dbbcdd5dbc0a24002c4d) *(package)* Correct scoping of unix-specific path operations
- [`a4e076cb`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a4e076cb319fe6433eb2fdd25f282391d8555173) *(zoiignore)* Implement recursive directory ignoring during bundle
- [`6155e192`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6155e19207f72407389aa321f33ac33c223c8824) *(config)* Change default build binary to zoi

## [Prod. Release 1.23.0] - 2026-07-19

### ⏩ Merged

- [`097d1348`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/097d13487eddc4b1d5b11d9319b0f3f1360b3da9) Branch 'distro-builder' into 'main'
- [`bf686843`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/bf68684300e056647af92a6d5cb0038b0295612d) Branch 'renovate/uuid-1.x' into 'main'
- [`675bb97c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/675bb97c57f42af857fcc397540ea3941154cdb6) Branch 'renovate/toml-1.x' into 'main'
- [`88e8c332`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/88e8c3323ee8215425f51b8899dbda60544f4acd) Branch 'renovate/regex-1.x' into 'main'
- [`cd6c75cc`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/cd6c75cceffbd8ccd406d5496c993f8cb0df6820) Branch 'renovate/clap-4.x' into 'main'

### ♻️ Refactor

- [`e6a0a946`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e6a0a946ca92e614235d3bd95ed08cacb041bd8c) *(system)* Strengthen security and messaging protocols
- [`491bd54c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/491bd54cad5061ac16e6741b6834a2c2769358f7) *(cli)* Rename frozen-lock and frozen-lockfile flags to frozen

### ✨ Features

- [`f1c0596c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f1c0596c36ad4abfc922f2a353c11d3c07969235) *(hooks)* Add support for scanning bundled package hooks
- [`69c81b12`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/69c81b12077a6d584ea024c213039a832b407be1) *(service)* Add service enable and disable functionality
- [`40afed0a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/40afed0a1efcf79a6d8ac3c745ea6aaffb287d9e) Implement ZoiOS system and home management
- [`cb86bfcb`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/cb86bfcba458c5d2cde9a76cc06f5202b2d3b0e0) *(bundle)* Implement .zoiignore support and pre-bundled source detection
- [`372808ab`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/372808ab7b952438cc7ae7da743c0918eec1e81d) *(pkg)* Implement 3-way merge for configuration files
- [`eb62c434`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/eb62c434671c0925d0bc2a77ac69f3e47b9c2088) *(cli)* Add download command and PGP signing for bundles
- [`15f9f64f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/15f9f64fb7fa1373f380917f780d885dac4531ba) Introduce zpa package format and zsa source bundles
- [`d0109706`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/d01097060d39a4102eb819e9c8adebe9d3dfff62) *(package)* Implement ELF relocation engine and improve scope resolution
- [`471373db`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/471373db9d44606c3cf6e08b5c34c56b18aea8ed) *(package)* Implement hermetic build-time sandboxing
- [`4cfc8eab`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/4cfc8eab0de4e5c4904e26cb83ac994673723784) *(install)* Implement context-aware default scoping
- [`85adcfd1`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/85adcfd134b6054df7776498cc7290172ecac55f) *(upgrade)* Implement delta upgrades via bsdiff
- [`bc31fdd9`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/bc31fdd9a0fb8ccfc6af9b5be0068681eef4af5d) *(core)* Implement explicit installation scope control

### 🛠️ Build

- [`b6e4fc75`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b6e4fc75aa914ca3133eef00ede654da3189a332) Fix rpm spec
- [`276e67d2`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/276e67d21a26e7731324ab212fc93031fe3eea2c) Remove incremental build number tracking

### 🛡️ Dependencies

- [`cfa74453`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/cfa744532d14e519d65f9509156ca39100ebdf23) Update Rust crate uuid to 1.24.0
- [`a8507718`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a85077188405b3b1e78f55d6acb20bb9e3dd04d2) Update Rust crate regex to 1.13.1
- [`ecca938a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ecca938ab0c73540f50d540327222bf264728259) Update Rust crate clap to 4.6.2
- [`0b1415f6`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0b1415f655ff02676d038d2e586041c12ac93c49) Update Rust crate toml to 1.1.3

## [Prod. Release 1.22.0] - 2026-07-12

### ✨ Features

- [`586acef2`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/586acef295729fdb4c1da12937ee3540c3e08a41) *(lua)* Update LOCATION table to reflect staging directory paths
- [`a8d08935`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a8d08935f9e0bac8926869abebf5f6a8c5f2e652) *(lua)* Add LOCATION table to Lua environment

### 🛠️ Build

- [`2c1c4dd1`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/2c1c4dd1cf1739b271de91701c6f2b20c1cc43b2) Fix Zoi packages

### 🛡️ Dependencies

- [`28a588d3`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/28a588d387d19a6eb2e9c7860a587ad95d9bd79f) Update Rust crate regex to 1.13.0
- [`d6c27ea8`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/d6c27ea8c54c2f2d2eb8fa4fea40a354ba14f4f9) Update Rust crate sequoia-openpgp to 2.4.1

### 🧹 Cleanup

- [`fe39f7fc`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/fe39f7fc1e610765a80d0b9e810099fa5af9ece2) Remove MD5 support

## [Prod. Release 1.21.1] - 2026-07-11

### ✨ Features

- [`39c7ff2c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/39c7ff2c54f115c607c8643c933116b3b68411e1) *(package)* Add install_deps flag to docker build process
- [`d0fabf4e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/d0fabf4e2a2785038be36cabd0b57c6f51afc2fd) *(package)* Add zshell API for automatic shell completion management
- [`f6e7f3c3`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f6e7f3c34415684f23165010cb6c5e584baa73eb) *(uninstall)* Implement dry-run mode

### 🛠️ Build

- [`cb89599b`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/cb89599b7257e76072889b55346bae5771bb3fc3) Implement incremental build number tracking

### 🩹 Bug Fixes

- [`8d816a8b`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8d816a8bdf16d5e1ada731301e18588e53b2bc82) *(clippy)* Remove redundant borrows and unnecessary to_string
- [`7ed496c7`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/7ed496c785873237b53c8c49368582b07b065656) *(cli)* Implement dynamic shell completion support
- [`6376f1a0`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6376f1a0e307c70c24ab0e18e72d22051ef915bb) *(uninstall)* Resolve correct package file paths during uninstallation

## [Prod. Release 1.21.0] - 2026-07-08

### ⏩ Merged

- [`b6d21306`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b6d21306739bdcf8d2fb2a7ec5ebb668360442a6) Merge branch 'renovate/mlua-0.x' into 'main'

### ♻️ Refactor

- [`e3b36af8`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e3b36af846a67c963bb7b21312ea38c37a8da1e4) *(lockfile)* Implement automated integrity hashing and sync improvements
- [`e8825a09`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e8825a09ee5e964afa9391cd796f3c4ac8e21f31) Unify Cargo.toml for all crates

### ✨ Features

- [`bf4495b7`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/bf4495b78de258b266e28e983588284e995d59ec) Implement parallel package preparation and align CLI with Spec v2
- [`85fe1fde`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/85fe1fde87af41323fe732edd21f62cd43435592) *(install)* Implement two-phase package installation process

### 🛠️ Build

- [`a298c533`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a298c5338755fbaa59fb24687f7eff5eb7afcde3) Add Clang as a dep
- [`1adac52d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1adac52d133f056c1e522d8d72c333dd0b588ec1) Add zoi.lock
- [`fcd9c24d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/fcd9c24df601642b6ce5994c2e971765eb34054c) Remove Makefile in favour of Justfile

### 🛡️ Dependencies

- [`759d904f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/759d904f0e7b6f99a92f478babe3bf417c253a61) Fix cargo-deny
- [`5485d185`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/5485d18578f7c080eceb385a36b8540f4ebadddb) Update Rust crate mlua to 0.12.0

### 🩹 Bug Fixes

- [`6f1fd750`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6f1fd75010071ae97990766b29333001216c1011) *(project)* Ensure absolute integrity and state determinism
- [`84480d57`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/84480d5772c6f24e1392dbbd62e3788ce081f378) *(use)* Add support for zoi.lua detection instead of zoi.yaml

## [Prod. Release 1.20.3] - 2026-07-01

### ♻️ Refactor

- [`84c979d4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/84c979d4e6f0883856ed5998511d772d3e0bc4fe) *(resolver)* Improve default registry discovery logic

## [Prod. Release 1.20.1] - 2026-07-01

### 🛡️ Dependencies

- [`c53b2d85`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/c53b2d85fa8f1dfa1d0f584918dce4c2be7794d9) Update Rust crate serde_yaml to 0.10.4

## [Prod. Release 1.20.0] - 2026-07-01

### ♻️ Refactor

- [`76a6602a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/76a6602ae0350baa5122c867bbd6d12dfc50c4a6) Replace deprecated serde_yaml with yaml_serde and improve code consistency
- [`8fbfd747`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8fbfd74756d210112b40d45297dd4a62a0ca0e50) *(core)* Switch hash maps to btreemaps for deterministic ordering

### ⚡ Performance

- [`2723d638`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/2723d63899f0524132ea9c84babdceb924785ced) Optimize package verification and registry sync

### ✨ Features

- [`35d2de91`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/35d2de91223aac4cd383523da6796522719e3abc) Add registry type field, local sync, semver ranges, and platform lockfiles
- [`60d83e86`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/60d83e869df46f6b60d31760a7d070575c767dad) Implement Zoi Specification v2
- [`5bb9e3ba`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/5bb9e3ba3d97319339d28eb494d7bfe6264065f5) Add optional build lifecycle function
- [`33ca56b0`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/33ca56b0d4302d62221116ed189251321c10ed97) *(man)* Add support for multiple manual pages and TUI navigation
- [`6012990b`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6012990bc05dc73df07f777d759f116d2351f59d) *(install)* Gate PGP signature messages behind --verbose
- [`6b6f4499`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6b6f4499e3eff645731bac2863f52e39c30c8508) *(install)* Resume interrupted pkg.tar.zst downloads
- [`0b41f4b9`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0b41f4b99f56274d9033e766ec9d8cd7bf8c91c5) *(install)* Implement PGP signature verification using registry authorities
- [`c942d6d0`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/c942d6d0507b033782500a9558da5c29b57d1f93) Add junction crate and improve Windows symlink fallback
- [`999edd63`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/999edd6399d9fe62a639db8267d361f93b5c3f08) *(installer)* Add CI runner support and conditional package recording
- [`0e67f170`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0e67f17083efb8bba91cec429962c126d176b282) *(registry)* Allow optional package and repository arguments for add-advisory
- [`a1ce3227`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a1ce32274b0e48c3526f6e38772385db8613c572) *(exec)* Refactor exec command for full dependency resolution
- [`b66999d4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b66999d47f8a94bd93f75371a2b2faae00c8a1ec) Making packages sizes sync with 'sync' command
- [`f70624ca`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f70624caa5b64b353ff8efd5afcd77e082ba0bdf) *(sync)* Add force flag to rebuild databases from scratch
- [`59cc5ada`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/59cc5adaef9187bde74d915df42b2b4a16835c72) *(db)* Add support for sub-package resolution and indexing

### 🎨 Styling

- [`73fdedad`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/73fdedad4fffec408b9e0f81459cf8644dbcd04e) Format markdown files
- [`c6b93fe1`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/c6b93fe1f657d9263f10d5d238fe67562d5644f5) Update banners to use Zeno Sans font

### 🔒 Security

- [`e21aa981`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e21aa981d9c9f0dba73532553605fb204af6101b) *(config)* Restrict Lua env for --repo and fix scope/local bugs

### 🛠️ Build

- [`88659856`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/886598560dd393a220242ada1e57135793d563cd) *(config)* Load environment variables during build process

### 🛡️ Dependencies

- [`94fa989a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/94fa989a68db35269c353356c316694681d0d32e) Disable default features for workspace dependencie

### 🧪 Testing

- [`afac14c5`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/afac14c5f575bd647ed9724c77fca0780e785477) *(man)* Add integration tests for manual page functionality

### 🩹 Bug Fixes

- [`8cb21a71`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8cb21a7169ed18aab3681005309566700286bf8d) Warn on cross-platform lockfile mismatch
- [`431ad3f1`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/431ad3f1ffbef7107f4cb49ca124fa95f773921a) Hash project-local db for registries_hash

## [Pub. Release 1.19.1] - 2026-06-26

### ✨ Features

- [`9ad7ee73`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/9ad7ee7391d0eea751ad2cd825ac6fc2f184fb0c) *(release)* Bump version to 1.19.1 and update build configuration

## [Pub. Release 1.19.0] - 2026-06-26

### ♻️ Refactor

- [`4b111310`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/4b111310d5e2a42168dfeb6f09b68007a5986828) Transition to workspace-based CI and linting
- [`1a449d95`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1a449d95f40b34e6b4934a7af0155437989a6714) *(ci)* Update crate metadata and release process
- [`1fac66cc`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1fac66cc85e93a780e77fcbce054b49d89338945) Inherit workspace lints in crate configuration
- [`907ad7b3`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/907ad7b37cdf239de11ebc9b4559364f9361e154) *(build)* Unify workspace linting and crate structure
- [`db3cf5b1`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/db3cf5b12eaa6cccd347ab743bd90935bf591e35) Move Zoi into crates
- [`c0429a8d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/c0429a8deff27fc485c2f7b43a5ce471831d2670) *(cli)* Improve installation progress and binary resolution

### ✨ Features

- [`41fa4f49`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/41fa4f49aaf683756f911ebcb866c524c5747863) *(install)* Implement stage-based sequential package installation

### 🏗️ Structure

- [`6a4e9b64`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6a4e9b64ffca8502cccc3ffba615c90628d70541) Add zoi-rs crate and refactor project structure
- [`be6e5532`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/be6e5532c519f57d6212c60f3a24336aa235cd70) *(tests)* Move integration tests to dedicated crate

### 🔧 Configuration

- [`9efd39c3`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/9efd39c3d02cd50fa05b0307e870b8dae85f2da2) *(renovate)* Enable Cargo manager only

### 🛠️ Build

- [`2763949c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/2763949c9158b31e80f3251b230b84112a9e50b4) Add Renovate support

### 🛡️ Dependencies

- [`44b16ec3`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/44b16ec3012c1eb2aa8ef1396e1cbb44433c456b) Update Cargo dependencies

## [Prod. Release 1.18.6] - 2026-06-18

### ♻️ Refactor

- [`a2f8f8ef`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a2f8f8ef9086b23c2db7349b4586c412e28284d0) *(hash)* Implement multi-algorithm support and improve sync robustness

### ✨ Features

- [`60d82a02`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/60d82a023b5afeaced4f6673c67656d3773a2699) *(api)* Add filesystem and patch utility functions
- [`2ce286db`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/2ce286db3bbdedea439e876b060604bffa0393b6) *(pkg)* Add quiet mode to uninstall process
- [`918d3743`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/918d3743b5a2db9f788dcc2f7dd5d3f3897976cc) *(shell)* Add verbose flag to ephemeral shell

### 🛠️ Build

- [`ace216a9`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ace216a9a6b981c0c5f37943030b6df8ab2c541b) *(ci)* Refactor CI/CD pipeline and build scripts
- [`28cc1992`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/28cc1992124423cbf9629900330629c336902b53) Add debug build support to Justfile and Makefile

### 🩹 Bug Fixes

- [`35cb825d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/35cb825d86765fc2b7e641fd4bc283313f7bdb2f) *(update)* Accurately display download and net sizes
- [`fe1174a5`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/fe1174a5842d6bc0f9d37fd9be5dbc0ef0083d8b) *(purl)* Implement local resolution support and bypass PGP verification in tests

## [Prod. Release 1.18.4] - 2026-06-13

### ✨ Features

- [`50ba7186`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/50ba7186939060d8ec12492feef8ac2275b03bd9) *(pkg)* Add 'revision' support
- [`08887459`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/08887459b6da08336fd556edbb79fae049c781fc) *(cli)* Add clone command to clone package's git repository
- [`f7682132`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f7682132501710249b23abfbe0e4c452c43be921) *(sandbox)* Implement native Linux sandboxing with Bubblewrap
- [`aa6486b3`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/aa6486b3a143a0ee8bca43b9efa955c169115ab1) *(cli)* Add verbose flag to 'exec' for execution details
- [`3a74eaf3`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/3a74eaf31e861709131c83daab2704a12d3d5a89) *(install)* Optimize git clone operations and refine non-zoi dependency resolution

### 🎨 Styling

- [`0cd83f6f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0cd83f6fec62b315e53698388b5b4507f6af5625) Update gitlab repo naming

### 🔧 Configuration

- [`4df95a58`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/4df95a58ccab9fbc8cf1829428fcbf012fc0820d) *(build)* Update Justfile to use env
- [`cd9aafcc`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/cd9aafccc37317e41cc8e7cc075f5dc588e7ca02) *(build)* Update installation instructions in configure script

### 🛠️ Build

- [`736464d8`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/736464d83af653a4f27bf94f8f23c5ef30748c19) Add deb and rpm package support

### 🩹 Bug Fixes

- [`e07ab0a0`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e07ab0a020e5d05b59a2f1eaf07a542d68aa62c9) *(cli)* Resolve missing command output colors

## [Prod. Release 1.18.3] - 2026-06-08

### 🩹 Bug Fixes

- [`8a43b1b9`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8a43b1b9901cf73d53e6d952e3cbdcdaeb575198) *(lua)* Add retry mechanism for file downloads

## [Prod. Release 1.18.2] - 2026-06-08

### 🛡️ Dependencies

- [`63478ae4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/63478ae47af738d05862f0b03d2d5e0f09f1f969) Update Cargo dependencies

## [Prod. Release 1.18.1] - 2026-06-08

### 🩹 Bug Fixes

- [`231ed98d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/231ed98d6ec72c4a6140288c35e75860a8c61e75) *(package)* Resolve Docker panics and metadata file extensions

## [Prod. Release 1.18.0] - 2026-06-07

### ♻️ Refactor

- [`345e706d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/345e706dbee2ad1f0a7194c17d2c7030fbe7ce2d) Replace expect with proper error handling
- [`71baa740`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/71baa740846056ec0f988c85ce78e0a62e91c186) *(pkg)* Improve robustness of Lua integration
- [`53700608`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/5370060816091da3a0b2881e3768f97905c071dc) *(registry)* Remove official registry-specific naming from init command

### ✨ Features

- [`4ea1d36e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/4ea1d36eafde2400ecb8ef40cfabca2386222341) *(package)* Add --fakeroot option to force root ownership
- [`8251829f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8251829f9e2d477b62cee09d5c28cb585fb37a9c) *(package)* Add inspect command and refactor Lua API modules
- [`7ae42277`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/7ae4227724c1cb15cfdb4523133a01dda83c92d8) *(pkg)* Re-add and integrate display_updates for important package notices

### 🎯 UX

- [`fd1faebc`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/fd1faebcff3f7c39459951634c7d146ade130d7f) Remove newline at start of update all cmd

### 🛠️ Build

- [`a97e35ca`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a97e35cad7fc7279c47ac5fddb6eb5e585f16ae9) *(config)* Add Justfile for project management and update configure script

### 🧹 Cleanup

- [`edb2c2e5`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/edb2c2e548aa305f743c09b66013a21964f5c137) *(pkg)* Remove unused Declarative install reason

## [Prod. Release 1.17.0] - 2026-06-04

### ♻️ Refactor

- [`ff4d1264`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ff4d12647fcadcc823c268d471f5b4b44ee658b9) Improve CLI UI and package extension management
- [`cc3c825d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/cc3c825d8d6eb87cada2619eb7b2fe39cb4a2aa8) *(audit)* Transition audit log to structured JSON format
- [`ff360e5d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ff360e5d964dcb489ef24d04ba954b2a92439b1d) *(cmd)* Remove pager implementation

### ✨ Features

- [`cc45e1b8`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/cc45e1b8052771ead74b37ae1617b8b51d379980) *(shell)* Improve tab completion with context-aware package lists and descriptions
- [`84415d07`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/84415d078436e58d30fbb6375a01d3a650d33b27) *(pkg)* Implement just-in-time privilege escalation

### 🩹 Bug Fixes

- [`76989c2e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/76989c2e060c9bf1aebf55ab36c3b39b16701f40) *(audit)* Correct history export format and fix test tampering logic
- [`e2ef0c82`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e2ef0c8237b030b577733287156bd3d10b159590) *(cmd)* Make uninstall scope consistent with install and clean up UI
- [`07ec18a4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/07ec18a4c94d53128b63d66e70fef8048d7fe4ee) *(hooks)* Resolve builtin security warning and fix loading precedence

## [Prod. Release 1.16.0] - 2026-06-03

### ✨ Features

- [`eea89e18`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/eea89e18d93d3f073bf5024b9f8116fb478c2353) *(lsp)* Add Lua language server support for package definitions
- [`4b03d940`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/4b03d940c4ac8e66d36eb3aab2f2d7ec9a55226f) *(registry)* Add registry management commands
- [`31bea2a7`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/31bea2a7e2def45477542b2a907c77983ef00788) Add Zoi use command and project task runner enhancements

### 🛠️ Build

- [`f95c5609`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f95c5609485433b3ee1ac749e73d769eab1670bf) *(docker)* Migrate base image to Arch Linux

### 🩹 Bug Fixes

- [`6780dd73`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6780dd7319cf2330bb0b6fd6f3fa64ef93b6b370) Correct cli test
- [`ee08fd62`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ee08fd62cf594667c2f88521fe890be5b4f12d47) *(sync)* Enable HTTPS and vendored-openssl features for git2

## [Prod. Release 1.15.0] - 2026-05-26

### ♻️ Refactor

- [`bd09149a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/bd09149a2797b7cf0b58d9cfa181c0f1295d9294) *(remote)* Refactor remote registry fetching and enhance error handling
- [`24733722`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/247337220b6bc520c4bf6667931d561628cd48a2) *(core)* Make PluginManager optional and add remote verification
- [`2f7d5a91`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/2f7d5a91e875980a497709b4eb77e8af5862a64f) *(build)* Refactor build script and reorganize builtin asset paths

### ✨ Features

- [`9e59061a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/9e59061a3c2dea62da0206178b756d5f0f78d72e) *(policy)* Implement centralized security policy distribution
- [`75d19616`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/75d19616958520c8f34253fe874723af5fa8b1b0) *(show)* Update show commannd
- [`67cdee4d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/67cdee4d3e778de9a4661455aca82f8a955c9b40) *(deps)* Add slice accessors for dependency groups
- [`9f79a2f4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/9f79a2f4b72c2dc3d902705c061d63dbad0c7f15) *(hooks)* Add builtin system hooks for caches and ldconfig
- [`158b846c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/158b846c1e5618436dfa574a9718da48a7fed388) *(sync)* Add system git fallback for package sync

### 🔒 Security

- [`be63c4b6`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/be63c4b68ca22c8adc95569af39c56e37c38176d) Harden system against untrusted code and path traversal

### 🔧 Configuration

- [`367f171a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/367f171a039cbccdd7f0eba7678e0632e954adbf) *(pgp)* Include built-in PGP keys
- [`8c593901`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8c5939018339e7b2a655bba0aeafa1cfe826a7b9) *(cargo)* Refine crate package exclusion

### 🧪 Testing

- [`0d025c2c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0d025c2c9aef8c41ea7dc5a0da8a9f7dacdbc8df) Add tests for policy merging and path validation

### 🧹 Cleanup

- [`aa6288c4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/aa6288c47b29b1b4da54a5e31c8b9c433572379f) *(system-config)* Remove declarative system configuration feature

### 🩹 Bug Fixes

- [`a84590f9`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a84590f9bf61c034bac5d7fc7b44040869a354c3) *(scripts)* Resolve makepkg permission error for temporary directory

## [Prod. Release 1.14.0] - 2026-05-22

### ✨ Features

- [`a1453164`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a1453164b016488e31f61cdaff5f033152c8e4fc) *(pkg)* Add macOS .dmg/.pkg extraction and .app handling
- [`48b345f2`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/48b345f29f868ee5f6216387a3cf9684e263e40c) *(purl)* Enforce repository path in PURL resolution
- [`56c0dfd4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/56c0dfd4e272d6b65fcd11f3b6c1129de395a3f7) *(cli)* Add PURL support and new validate command
- [`260c4152`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/260c41521e097c88b4b8e22e79ff86657387809b) *(pkg/purl)* Add PURL package management and validation
- [`fc0b6e6a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/fc0b6e6acba2c35399b29792bbe3cd2193c9ea74) *(hooks)* Add directory-based triggers for global hooks

### 🎨 Styling

- [`b2655861`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b265586140ebaf64dae7ae00a98c6e3dfa71975d) *(sh)* Format shell scripts

### 🔒 Security

- [`719a2046`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/719a2046bccc5495739f71ff126c5eed636ab799) *(mini)* Disable plugin loading to prevent untrusted code execution
- [`58e01379`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/58e01379ae3df3e8c599c870919bbbf932a3c8f3) *(zoi-mini)* Implement checksum and GPG signature verification for zm scripts

### 🛠️ Build

- [`4435fa71`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/4435fa711d3eccfba1aef488e4cea8308d47c8fc) *(pubgrub)* Update integration for 0.4.0

### 🛡️ Dependencies

- [`849d9fd5`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/849d9fd59594550469778b66670ce27ecf049408) *(cargo)* Add purl dependency
- [`8cb76861`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8cb76861b09f4275ed7dc0cd5bb68882a8c2ed02) Update Cargo dependencies

### 🧪 Testing

- [`37d45593`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/37d45593b4aed7d22c52ed42392fe712711b3287) Add tests for helper command
- [`92561dbb`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/92561dbb975217f3214da35ff281d7303497fcdf) *(assets)* Add package and advisory test data

### 🩹 Bug Fixes

- [`100e7b56`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/100e7b56eebca300f7439a9affbfd46b6c5c93f2) Correct purl test expected package output
- [`2ad815cb`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/2ad815cbe2ba3466d0026d88c73c9ebdf1405c22) Correct tests path
- [`5cb085a5`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/5cb085a594d738e73aa4d82d20699eb741fbadd3) *(sync)* Resolve git2 API breaking changes

## [Prod. Release 1.13.0] - 2026-05-14

### ♻️ Refactor

- [`0c9ae652`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0c9ae65291f9141a77b9ca8f9a64ef7313838b9f) *(timezone)* Isolate Unix-specific timezone management

### ✨ Features

- [`e19e8d25`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e19e8d2566046c964baeccdb5f6279e019f0cbe0) *(mini)* Enhance Zoi Mini with aliases, shim support, and caching
- [`c2d200cf`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/c2d200cf13c11dfc81b67b75ddd87752e4f37359) *(mini)* Add minimal package manager
- [`db4f6225`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/db4f6225c8cc8182713d99c52f30f0cd562883d0) *(zoi-mini)* Add platform-specific app entrypoint scripts

### 🛠️ Build

- [`d0b4ae61`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/d0b4ae618694021569537321842ec2f8eeccd646) Revert release opt-level to 3
- [`71e17a76`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/71e17a76e90de0c953071303ae781b7fc37ba882) Consolidate release profiles and optimize binary size
- [`a2060cc1`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a2060cc16bd340b452a29b26e2062a7164018f96) Remove FreeBSD and OpenBSD support
- [`3a78e1f1`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/3a78e1f1e833979919c96cbe50bd4a3a35d2beee) Make build script builds and install all project binaries
- [`293f956e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/293f956e0348e9f26c7d3480ae5461d297d8a126) *(build-system)* Add zoi-mini binary and integrate into build

### 🧪 Testing

- [`43d0a6b7`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/43d0a6b7b100f70752ff33e5d6b4b85c3171cb1e) *(pkg)* Add tests for package resolution and mini-resolver

## [Prod. Release 1.12.1] - 2026-05-07

### 🔒 Security

- [`e1f1c576`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e1f1c576338621bc4eb7a9e27f2d530d942c31fe) *(registry)* Revert changes on signature verification failure

### 🩹 Bug Fixes

- [`63f20fed`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/63f20fed1d63e0360065adc482307c5e80482d25) *(dependencies)* Isolate mutex guard acquisition

## [Prod. Release 1.12.0] - 2026-05-07

### ♻️ Refactor

- [`7237a172`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/7237a1728d4bbbd5015fef9802290c1ce3426608) *(sync)* Refine command output and remove auto shell setup

### ✨ Features

- [`7c1bc785`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/7c1bc78588af14a47a98554433e461196787649b) *(pkg)* Improve installation robustness and registry sync speed

### 🛡️ Dependencies

- [`5eaaa06e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/5eaaa06e3382e819606dd308d7f136cdc0b09e26) Update Cargo dependencies

## [Prod. Release 1.11.0] - 2026-04-25

### ✨ Features

- [`3762b020`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/3762b020010bc5a7fc14ccc9974720f7fb859590) *(pkg)* Add reproducible installs, transaction inspection, and mirror support
- [`b7cabb92`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b7cabb92df28273f8781954aa5af85eb763e3b24) *(pkg)* Add interactive selection for installed packages

### 🧹 Cleanup

- [`b2184449`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b218444913f523a63cd36db489a452768b69ac76) *(clippy)* Address collapsible_match warnings

### 🩹 Bug Fixes

- [`0d8f8dcf`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0d8f8dcffce128bd240f8e289c27571743c65077) *(pkg)* Harden extension lifecycle and runtime state

## [Prod. Release 1.10.0] - 2026-04-02

### ✨ Features

- [`72383ab0`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/72383ab07f81dba73a6846f8c0006f665fcd2d26) *(ux)* Standardize install/uninstall/update preflight, explain, and bump to 1.10.0
- [`3b012639`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/3b0126397ffe7d0dfaa6b2c517ca75d1d83ff3d0) *(migrate)* Convert Scoop manifests to full pkg.lua scaffolds
- [`fdaff7a7`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/fdaff7a7cf872159a82794006b4f636fe8029999) *(install)* Enforce frozen lockfile and audit chain verification
- [`c4286f94`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/c4286f943e671e46364192b9fc32e4c63ec509cc) *(lib)* Enhance public API with typed options and resolution
- [`e85976e3`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e85976e384fd2b2bfc4c64143bd380ded8aec441) *(config)* Add parallel jobs unoverridable policy

### 🎯 UX

- [`13757b5f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/13757b5f1346c8b364e726f949b4974292ba26b4) *(install)* Add --verbose flag for package origins and preflight info

### 🔒 Security

- [`34b251bb`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/34b251bb3b28c7e76ecc319b53ca99e134a447a2) *(policy)* Enforce allow/deny and license rules

### 🩹 Bug Fixes

- [`415ed63f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/415ed63fa28b93290701dcb9d1f6a2dd2cd63cc9) *(resolver)* Improve local package and channel resolution

## [Prod. Release 1.9.4] - 2026-03-28

### 🎯 UX

- [`8456e3b5`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8456e3b54f97a19be34d1edaec1a22f543fea669) Thingy thing

### 🩹 Bug Fixes

- [`8dadb430`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8dadb430fe035b5238afddc9a8086617baaf0c9a) *(pkg)* Improve file download and symlink handling

## [Prod. Release 1.9.3] - 2026-03-23

### ⚡ Performance

- [`96429f2a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/96429f2ac75238bda6a80ea5c8231c8da4b79dfe) *(pkg-resolver)* Optimize dependency resolution with caching

### ✨ Features

- [`e1cce9d1`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e1cce9d1815ba7565250131be7a09cdcbf721854) *(pkg)* Enhance system info and project handlers
- [`f3c18166`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f3c1816648b9b37fda49051f2933e5840e97b404) *(plugin)* Add extended Lua plugin APIs and project install hook
- [`ab830cda`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ab830cdab6f8a0ced2d54e50247ba609ae1e6e8e) *(list)* Add option to show outdated packages
- [`1212f579`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1212f579cfdb7242a8ea5adfe38f3fb6d1cc5de9) *(pkg)* Add platform filtering for package builds

## [Prod. Release 1.9.2] - 2026-03-21

### ✨ Features

- [`a7b0cb0d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a7b0cb0d84e85684ad9f6fed7529902a1465e5f8) *(docker)* Enable GPG signing for Docker builds

## [Prod. Release 1.9.1] - 2026-03-21

### ✨ Features

- [`90fab8eb`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/90fab8ebdddfcbb46e953ae86825a698080b9ae8) *(security)* Implement sub-package advisories and enforcement policy

## [Prod. Release 1.9.0] - 2026-03-21

### 🔒 Security

- [`e1f69d65`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e1f69d65af4e0e0420939a2f62bde5ea6dd24f5b) *(audit)* Add security auditing command and vulnerability checks

## [Prod. Release 1.8.8] - 2026-03-21

### ✨ Features

- [`31ac13f0`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/31ac13f0651b51ba60680bf3c64100343ddfa692) *(pm)* Add dynamic sudo handling for package managers

## [Prod. Release 1.8.7] - 2026-03-20

### ✨ Features

- [`dde1d5ea`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/dde1d5ea26b1c4e90715eb549f0dade0879e3dd9) *(build)* Make build type optional

## [Prod. Release 1.8.6] - 2026-03-19

### ✨ Features

- [`c1f19fe0`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/c1f19fe01c6ad3a44863d689896029367bfe7453) *(package)* Add Docker build method

### 🩹 Bug Fixes

- [`41bcf584`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/41bcf5842ffcc985e644c9fad1923aab9b8b5b02) Bug from tests

## [Prod. Release 1.8.5] - 2026-03-18

### ✨ Features

- [`963fa189`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/963fa1897ea0890212f0cd4745bba28f9350238f) *(pkg)* Add automatic build dependency installation

### 🧪 Testing

- [`a0ee82eb`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a0ee82eb2a2225e0886b1440bb3752ef36d9266c) *(service)* Improve service management testability
- [`90c5353b`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/90c5353bcea6edc1532535c1c9143cf0efde8632) *(pkg)* Add comprehensive test suite for package modules

## [Prod. Release 1.8.4] - 2026-03-08

### ♻️ Refactor

- [`cfb5b4c2`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/cfb5b4c260e535862c93f1ca9d5fe8b1e7946975) Better code i think
- [`17d59d32`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/17d59d3233b224f78e705cd2f6ecb72d26595c46) Centralize HTTP client and optimize registry sync

### ✨ Features

- [`8ceeb3e6`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8ceeb3e6e001f444655e851147f509846fe21c1e) *(install)* Allow specifying exact package versions for install
- [`ce3ae5fc`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ce3ae5fcb0e08d6fb10e127f4ca4af3807ab512b) *(resolver)* Implement semantic version range parsing

### 🩹 Bug Fixes

- [`d5ace88e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/d5ace88e4c813caded07b46e475ef91e0bc4ea7c) *(pkg)* Correct dependency version ranges and install scope

## [Prod. Release 1.8.3] - 2026-03-07

### ♻️ Refactor

- [`d8d9664d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/d8d9664d86b1848d7193338731d445d0c3a28788) *(http)* Centralize HTTP client creation and add user agent

### ✨ Features

- [`b2eaaf91`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b2eaaf912dfc2ab50a57e0ef152b3ee5343402ef) *(install)* Add --build flag to force source compilation
- [`5ca22088`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/5ca220886542a3c4a104348b188862707d3aac7d) *(install)* Skip already installed packages during installation
- [`3833356c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/3833356c8b26e1061ef820b5bca5b6db7c40be97) *(lua)* Allow functions to resolve paths relative to BUILD_DIR

### 🩹 Bug Fixes

- [`daaac2cd`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/daaac2cd01417722352a6183f8bcaaa2a68de2de) *(install)* Make --repo flag work

## [Prod. Release 1.8.2] - 2026-03-06

### ♻️ Refactor

- [`6ff3d7d1`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6ff3d7d1fe2718819f2350ee504228ba8fac3bb0) *(db)* Consolidate package updates to local database

## [Prod. Release 1.8.1] - 2026-03-06

### ⚡ Performance

- [`45f92026`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/45f920266b1b1c7eecfcf5887c394c62c13f6df3) *(hashing)* Stream data directly to hashers

## [Prod. Release 1.8.0] - 2026-03-05

### ♻️ Refactor

- [`ea340793`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ea3407936682f2728d93297c12a1d6bf0c4f9e49) *(doctor)* Integrate external tool checks into doctor command
- [`83a4527f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/83a4527fc9d7c92e857708ac44e5942c84278ffb) *(pkg)* Refactor package resolution and installation flow

### ✨ Features

- [`90d072bb`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/90d072bba65b83d8114cc4585edeb2a08708e637) *(pgp)* Allow non-interactive GPG signing with passphrase
- [`f3b6e84a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f3b6e84a0e3ff3115a83ff41c1a4f3b89fb22184) *(completion)* Provide package descriptions for shell completion
- [`a6a41cf7`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a6a41cf7c00a79beef59c376b6055a3c677994f5) *(plugin, config)* Add shim version hook and configure rollback default
- [`b2886049`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b288604923e2b80e1460bed9cd5cd4a05ff5c3e1) *(telemetry)* Enhance data collection and status output
- [`65cfb770`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/65cfb770b4f346bf4aeb280355f6d020c562ef56) *(pkg)* Persist and optimize dependency resolution
- [`62fe854c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/62fe854c1a6807547dba3040e5dd4b2c4d6bace3) *(system)* Extend declarative configuration with advanced options
- [`b95f5ced`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b95f5ced49693a042058123d5a46d51eb57fb3d9) *(system)* Add declarative system configuration
- [`5fea727b`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/5fea727b10e1ebb700fa88ca6f2e45517d72d26a) *(pkg)* Implement package shim system
- [`779b9d4c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/779b9d4c833a80c527126ebc2b2e63aa6780cb70) *(pkg)* Add file system build operations to Lua scripts

### 🩹 Bug Fixes

- [`97034c5b`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/97034c5b7ecada83b37337a5258533b498a4430f) *(pkg)* Refine package path handling and dependency resolution
- [`ba52a509`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ba52a509db8f05aa6e7e374fa03123d60262f267) PGP

## [Prod. Release 1.7.0] - 2026-02-27

### ✨ Features

- [`693e3da1`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/693e3da114ea520d138b5e9f5d55bc2659bd528d) *(completions)* Add dynamic package name listing
- [`67682ccc`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/67682ccc71789c36612e25b031b4c3c7c1dd61d3) Add zig package manager
- [`82f98728`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/82f9872823676ddcc5ebcdbf49a85b7853d6ec45) *(cli)* Add 'provides' command to find packages
- [`29c04abc`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/29c04abc52011c995fb1975a1f422d4408e2ea7f) *(service)* Add package background service management
- [`69c036a0`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/69c036a0c5b4a863f0f84f3834b1fef35c17fed7) *(cmd)* Add dependency tree visualization command
- [`30e7eb00`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/30e7eb00265ed874724996e2894df2de7b18ed46) *(cli)* Add project development shell command
- [`f0c13153`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f0c131534dde41ec1dfe53502cb829806ad7a8f7) *(shell)* Add ephemeral shell for temporary environments
- [`0c8247b3`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0c8247b3057baf089f82bdc38c5edbe452b9da20) *(cmd)* Add dry-run flag for install and update
- [`49e07304`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/49e07304efeb21f1dab541ca05e13569662dddde) *(search)* Add global file search
- [`817d2c8a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/817d2c8aa8c7fed0458857013fff2f7d8bbb59e5) *(uninstall)* Add recursive uninstall for orphaned dependencies
- [`aca4f59c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/aca4f59c13b77e02073f88993f20bae8794e8f12) *(list)* Add --foreign flag to list packages
- [`e9ab60d1`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e9ab60d1304876c266bb7702dfceae9509be3fa3) *(resolver)* Integrate PubGrub for robust dependency resolution
- [`96da55a1`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/96da55a1d6bf5d6c76454abe085b25a5fdbad53a) *(hooks)* Implement global hook system
- [`19e6bf8a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/19e6bf8a143f9a0b30e5599744ece7ba90157a0d) *(pkg)* Add downgrade command
- [`0ad99763`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0ad99763e897cfd9e6e21d8e0614b9021ece2bc8) *(pkg)* Add global offline mode and cache commands
- [`ae534b6f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ae534b6f0b3566478c912cef1a527a5236ff6773) *(pkg)* Add 'mark' command to modify package installation reason
- [`e37640de`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e37640deb40b5be6bcb885ad967fe046a4c546b0) *(db)* Enhance package tracking with sub-packages and scope
- [`c7d582a5`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/c7d582a5838d59307edba7c509943f4fcb56848f) *(pkg)* Add SQLite database for package metadata
- [`2bd86b03`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/2bd86b031823b610daa390c1dc57414ad2487cb9) *(sysroot)* Add option to define alternative root directory

### 🎯 UX

- [`c8a5a6fc`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/c8a5a6fca96932dcc542115aea37e7ff2c0791d8) *(pkg)* Enhance multiple package selection with table display
- [`f190426e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f190426e5c9e3be19726d5be15b9c18723469715) *(cli)* Suppress zero size output in install and update commands

### 🔧 Configuration

- [`4edfde40`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/4edfde40f6327a26d402c00a1f6dad811971e54d) Add configurable offline mode and package directories

## [Prod. Release 1.6.0] - 2026-02-26

### ♻️ Refactor

- [`fae24d79`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/fae24d794b97ed3931d08bc4531b154cc839576e) *(symlink)* Centralize symlink creation logic

### ✨ Features

- [`2e6c8f09`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/2e6c8f095a2d6b9ef116fae942f24fe8d8df247f) *(pgp)* Add support for builtin PGP keys
- [`e6c30e92`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e6c30e92a5dc51bf9f9fbaa97377e25f274f1a41) *(security)* Implement PGP signature verification for registries
- [`94ddc3d9`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/94ddc3d9c223cc0d1363ce4c5336f2d46e2eee8e) *(doctor)* Add orphaned package detection
- [`9e8ea866`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/9e8ea866868a2609efff78a0e7c7923dee39c878) *(audit)* Add package operation audit log and history command
- [`b19c5f60`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b19c5f601be5978c1950646dd1d2bc5646c9f621) *(search)* Add interactive TUI and result sorting
- [`4416e80f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/4416e80f224d706b6f676d8a873b551f70efc63b) *(plugin)* Introduce Lua-based plugin system
- [`dc9c2fa1`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/dc9c2fa1ed4d3bfeb726a78996b33c2733b5cb7c) *(pkg)* Expose package directory paths to Lua
- [`cb4b4213`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/cb4b4213f0cb3fea9e604ec1c159b47bdc954120) *(lock)* Implement advisory file locking
- [`8bd581c2`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8bd581c269832172c7c020cdbba766d44be2a6d9) *(archive)* Add support for 7z, RAR, and DEB archive extraction
- [`951812b5`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/951812b5d9728f61661954e1e8efb9b22d9eb8ec) *(pgp)* Add key validation and status display

### 🛡️ Dependencies

- [`a9ac743e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a9ac743e33b2fc36dc250c59d7cfe3129fa997e8) Update Cargo dependencies

## [Prod. Release 1.5.0] - 2026-02-22

### ♻️ Refactor

- [`6ab1bdb9`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6ab1bdb95f5f45feed65caaa1dbd45db256515c5) *(shell)* Abstract shell command execution

### ⚡ Performance

- [`ecbf459e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ecbf459e82c6965587336071acc1f5fde0b2a558) *(sync)* Synchronize multiple registries in parallel

### ✨ Features

- [`9c944f32`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/9c944f321643f60001260b039c0b3eb9e2b30973) *(cli)* Add dry-run option to autoremove and clean commands
- [`e6fe6546`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e6fe6546a3c25a5fc71194597fc292f320dc3884) *(doctor)* Add package and PGP health checks
- [`006a970f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/006a970f88bb401deeba7c107f44aab048ab825a) *(lua/utils)* Add file system and archive utilities
- [`52844e54`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/52844e54f72fa1f5052fa77f9928e8f7994ada33) *(cli)* Add --registry filter to list and search
- [`79afbc1e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/79afbc1ef7229d4517afe1a58e9040fb66664b23) *(install)* Enhance file conflict detection
- [`c11557af`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/c11557af3c2e9f90e55e68bdd24776d444569c9e) *(pkg)* Allow configuring max package resolution depth
- [`5ef69669`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/5ef69669d81a4a9461cbc33ce23b1cdcc33f83b5) *(uninstall)* Add 'yes' flag for non-interactive mode
- [`d44781cc`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/d44781ccb6d1a90ba91e5c82dcf425e317eba0ff) *(pkg)* Add Arch User Repository (AUR) support

### 🎯 UX

- [`e71e7116`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e71e7116e5438d7fa5fd4f97f1094721f6990358) *(install)* Enhance install command with detailed progress and summary

### 🩹 Bug Fixes

- [`e5491c15`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e5491c15c7e4991ca24370703709341ff381cc3b) *(sync)* Dynamically determine remote default branch

## [Prod. Release 1.4.0] - 2025-11-11

### ♻️ Refactor

- [`fc7c91b6`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/fc7c91b6f35d602465e704781a06981e127b3719) *(cli)* Standardize error handling
- [`7eaba87e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/7eaba87e7d5dd3f4d276bc37409bfaaba01533af) Use `unwrap_or_default` instead of `unwrap_or()`
- [`b7ac1f98`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b7ac1f98c0885c64f1e16db21d44c98f50ead451) *(pkg)* Decouple package download from installation
- [`866027d6`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/866027d66ecd4073d7798cbf3779390b3c3445d3) More good code
- [`b6162b26`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b6162b261c6c176b309991fecc3f7eaa53fd1ca0) Better code i hope
- [`c8d46e73`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/c8d46e7352677d59f6a19c4a16d21d48b9d71664) Merge 'setup' command into 'shell'

### ✨ Features

- [`33522fde`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/33522fde7c9cb8526561fef844758d853a4d572d) *(pkg)* Enhance package resolution and parsing

## [Prod. Release 1.3.1] - 2025-10-31

### ✨ Features

- [`d175dbe4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/d175dbe41a26a3baa7632597a53e3a453edb9267) *(pkg)* Add support for typed build dependencies

## [Prod. Release 1.3.0] - 2025-10-29

### ✨ Features

- [`93eec4d0`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/93eec4d0519e68c889efd0a4d45476a8e0d6b501) *(pkg)* Implement package replacement, provides, and backup
- [`629401d6`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/629401d63d10e8cfaa29246a2879ee8aa7a31882) *(Lua)* Add support for  in package sources
- [`1f803d30`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1f803d300901e997f63de1ead4b28607818ec087) *(pkg)* Refine package management UI

### 🔒 Security

- [`292e1055`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/292e105556d7cdf89d013f5d7607d53cb9b8c384) Closes #26 Docker image vulnerability

## [Prod. Release 1.2.2] - 2025-10-27

### ✨ Features

- [`0c1bbdb5`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0c1bbdb5bbbe512ba6f8a5365bb908763368d144) *(pkg)* Add 'output_dir' flag to 'cmd'
- [`e3761bf1`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e3761bf1870ff9bf2b9f285e23542929f67846c9) Remove 'build_date' from 'manifest.yaml' in 'pkg.tar.zst'

## [Prod. Release 1.2.1] - 2025-10-26

### ✨ Features

- [`68ba7531`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/68ba753115e0edc672f84bae176aa59e9f80c562) *(db)* Implement package database write protection
- [`2f51a4eb`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/2f51a4eb488f16db665aef1fb560adf7b7720227) Remove 'sync' from 'update' command
- [`8ca49d37`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8ca49d37eef66c3cedffb25c173f4194edb99b73) *(package)* Implement test command and build integration

### 🩹 Bug Fixes

- [`689097f6`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/689097f60a2482ddb41d59d1f321f34b98b52db3) *(upgrade)* Retain temporary directory for fallback upgrade

## [Prod. Release 1.2.0] - 2025-10-25

### ✨ Features

- [`14299154`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/14299154c447d90d27d61e8ccbf45a77c49cb033) *(deps)* Support sub-package dependencies
- [`0316d725`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0316d7250f364682611536abb91aa664fbe60ac2) *(pkg)* Implement uninstall sub-package logic
- [`d727b611`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/d727b61196c848f0edf9028c65a0505c0e45bb67) *(pkg)* Implement support for split packages
- [`5539f72b`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/5539f72b449083a168eed9112870de25252f8a01) *(project)* Introduce new zoi.lock format and verification
- [`c4ad6c23`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/c4ad6c23f916d97d2b1bf62e25854b5d8bcfd040) Add Unkown license custom warning message
- [`38d05fa6`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/38d05fa62066edf60e28f57fb17e65bc43ba1edf) *(doctor)* Add doctor command for system diagnostics
- [`fea027b8`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/fea027b861563b5d8a798e7856232d395d90df6c) Better error messages
- [`422dc3a2`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/422dc3a24d028bfe5a933682f449c89882f5cb1c) *(pkg-policy)* Implement package installation policy

### 🧹 Cleanup

- [`b565356c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b565356cfcba0dac6d30593aa485dbea8e8f32f1) *(build)* Remove redundant completion and man page binaries

## [Prod. Release 1.1.1] - 2025-10-22

### ✨ Features

- [`92ece8b2`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/92ece8b2c94befee78e2b76798edd784927cecc8) *(install)* Implement file conflict detection and auto-overwrite

### 🩹 Bug Fixes

- [`9d1afa6e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/9d1afa6eb2fa5a70051ee3de125f9af34609fb61) *(uninstall)* Resolve path placeholders

## [Prod. Release 1.1.0] - 2025-10-21

### ✨ Features

- [`f9a8fec0`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f9a8fec0585634fcdca0530a7358cfa62bec252c) *(install)* Add option to specify package build type
- [`212aaf34`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/212aaf34d69969b6ca8e00ba654a2674ef466f62) *(registry)* Enable extensions to manage registries

## [Prod. Release 1.0.0] - 2025-10-21

### ♻️ Refactor

- [`36698b47`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/36698b470ad558d7e171924ddd9bef492a6e514f) Remove 'zoi build' command
- [`2d50aa80`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/2d50aa8026d3fbf0360e054cf33332551bc0e45e) Remove patch upgrades and generation
- [`b4439f5b`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b4439f5b0e1966f6c8240157be807bc48525face) *(rollback)* Improve package resolution logic
- [`5839426a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/5839426a0459933063b0f7467d3a347edaa395ac) *(pkg)* Centralize package name resolution
- [`abe098d0`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/abe098d04b203a110fc257fe08af5903d10005d1) *(project)* Use anyhow for error management
- [`55db2436`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/55db2436d338eac85cc6df426ea1b38725c140d7) *(pkg/build)* Use anyhow for error handling
- [`6d64c990`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6d64c990968209d350c618f53a176f7cf43412df) Establish core utilities and package configuration
- [`70d7a488`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/70d7a4881946b9434a60e8f1a0e2526697fe58ce) *(lib)* Simplify package management library API
- [`f6894ee4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f6894ee45e09000c499d3e28d0b630be5173a71d) *(pkg)* Move update logic and enhance version cleanup
- [`a07a5d8d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a07a5d8df090d1d82babb470bb775aa72c05ea75) Remove Library, Config and Service package type
- [`139e35f0`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/139e35f00e243dbab69b662be3bf0e88fe37137d) *(pkg)* Revamp package definitions and build lifecycle
- [`45f69926`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/45f699269bb311fe67a90b05b4f2d048bf988569) *(pkg)* Streamline package lifecycle operations
- [`9f30abc4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/9f30abc4fc421e5963dcf6ae05fe5e4a6dabd02e) *(pkg)* Enhance package execution and extension handling
- [`221223bc`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/221223bc598e49752521671813a48044ca3a677c) *(pkg)* Improve package pinning logic
- [`38b40ccf`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/38b40ccff33040dc422883abef304d376d8d21b4) *(pkg)* Enhance dependency resolution and autoremoval
- [`443384ad`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/443384adedbb78611ca6e9b128fcea808defa451) *(install)* Implement version-aware package installation
- [`17a3a952`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/17a3a9523ce921aeee762b743b18b5e49c0da83d) *(cmd)* Standardize CLI command definitions and package resolution
- [`a71c7051`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a71c70512599f539be3f6e9b78cb59b38106a77e) *(core)* Overhaul package module and type definitions
- [`48b8b351`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/48b8b35147623f3560c9affc6b79038d0cefb983) *(install)* Modularize package installation logic
- [`284d987a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/284d987a2a2b2d7d452e26d9dfcf15d620559b55) *(cmd)* Handle optional repo name for warnings
- [`8c30ac0d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8c30ac0d1aaa2e8cd2b29e31611d628917ad70fd) *(pkg)* Revamp repository configuration and sync
- [`668748eb`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/668748ebee99c307405d45cf74787ff96cafeb87) *(pkg)* Improve package retrieval with repo filters
- [`8a513316`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8a513316cea05da7d696c87e730d5f3469d6136c) *(utils)* Refactor PATH environment variable check
- [`321755ee`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/321755ee6861b9e42782039b46b5f05e636c44cc) Rename Zoi-Pkgs to Zoidberg
- [`8ba3abbf`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8ba3abbf9d99b463f386532ab776c36d062d1016) *(pkg)* Pass resolved version to Lua parser

### ✨ Features

- [`635bf5c0`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/635bf5c0c49915cf6305ce6d2f1a1f4910066b79) *(lua)* Run cmd_util commands in build directory
- [`3c18dd00`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/3c18dd005cf3d16be5b75a561bc16604a6cbc2a4) *(telemetry)* Add registry handle to events
- [`59b82ca7`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/59b82ca7ccea30a96e698b19d53b1e66af63473c) *(pkg)* Implement transaction system for package operations
- [`01aa7db8`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/01aa7db8a5825433736e821d253a6cdc8333be61) *(pkg)* Allow explicit version for package build and install
- [`7fb83022`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/7fb83022d774534c902b44d2e94fb0815e9aafa9) *(uninstall)* Add scope options for uninstall command
- [`f2581b16`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f2581b16862d6710b8d2571c8c950099bccf323e) *(install)* Implement parallel package installation
- [`9dce2854`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/9dce2854cbcc74fc5297125a43c7ee40cff32d2c) *(create)* Revamp app creation with package templates
- [`6a84edf9`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6a84edf9f1d080107863b12bcdf89515630843c4) *(install)* Add multi-progress bars for parallel operations
- [`45a073d4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/45a073d48566b9c71a1475b7ac50d38c118c3d4b) *(cmd)* Improve package CLI commands and error handling
- [`3c3193dc`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/3c3193dcb251da15ac52d49bca133ecc1eb09089) *(cli)* Add CLI commands for package state and queries
- [`fdeb0ff2`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/fdeb0ff2ba3296618f5c07f9abdd7d4ff714b756) *(pkg)* Implement package rollback system
- [`e20d7d94`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e20d7d94407ee2cc1717a5948fa690af4479ae37) *(extension)* Introduce package extension management
- [`afaeee60`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/afaeee60657e47369646abdebbe2e50c676e4dee) *(pgp)* Integrate PGP for package verification
- [`febdbd33`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/febdbd33e6dc917d0e762dbc1aaa9e8ff9b83f6f) *(pkg)* Add package lifecycle management operations
- [`f4652533`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f4652533c4713652873e27bfea3efff5304ae5fb) Implement robust package installation and execution flow
- [`b39f3262`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b39f3262212aee1fd6a64d4717c0259e518f6c14) *(pkg)* Add package recording and robust error handling
- [`f8136770`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f81367707d3d503bd3194c4f836b46107213a611) *(install)* Add --save option for project packages
- [`03b76013`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/03b7601307cc5bf4447c33ce44a9ea85b06a9555) *(hooks)* Add package lifecycle hooks
- [`463dca87`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/463dca87f62d3a53427b6c063252673d0defa6cf) *(cli)* Add 'owner' and 'files' commands
- [`2606f316`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/2606f3163ed2da6a4040b52988abdef73b852ece) *(pkg)* Implement global lock and atomic package installation
- [`9ec68d68`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/9ec68d684fe38180ce0d7aacbed7eead4fecd46a) *(build)* Add PGP signing for packages
- [`3924ffbf`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/3924ffbf8d956789620731433661bfa2ae8b7a7b) *(install)* Add project scope and CLI flags
- [`80533e27`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/80533e27b6df23092a934ddf13e05455c8fe9306) *(config)* Implement layered configuration system
- [`8f1b8715`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8f1b8715557286bcbd2bb06eded6e43a5587dcba) *(ext)* Implement PGP key management to extensions
- [`a5a9a873`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a5a9a873a3d2dc90e10b840493f38d1841d62dbf) *(about)* Add packager information to about command
- [`676f8829`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/676f88291bd9ec91b2620d98f990eb59953decc0) *(lockfile)* Implement zoi.lock for package integrity
- [`b4d9b6e2`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b4d9b6e21bc9dd265a38ffd5d1b6285405956558) *(pkg)* Introduce project-local package scope
- [`dd1136e4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/dd1136e419c51086f9935106b369bc0a9cdd8de4) *(lua/utils)* Add find and enhance extract utilities
- [`ab97cc2c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ab97cc2cbf8dcaf975af62687ffa671113b96999) *(lua)* Add utility to extract various archive formats
- [`fc751a2c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/fc751a2c6aaecaeec604d5e3db052206cdf2481a) *(security)* Add PGP signature verification and MD5 hashing to Lua
- [`107d67a9`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/107d67a96f090b1978509e1dcca9ac38150c060d) *(pgp)* Add command to verify detached signatures
- [`5fa1c4d7`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/5fa1c4d7d7be2d6214692cf4590fcefcbc7d31b4) *(lua)* Add advanced Git API and file import to Lua
- [`c1ac66fe`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/c1ac66fe8b5babcf97ab2d8d80d8843d59c820fb) *(lua)* Introduce Lua scripting utilities
- [`1104a127`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1104a1278b7a57b5f2c273ee851c18c654af6006) *(pkg-keys)* Enhance key management for signature verification
- [`1adf51ce`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1adf51cee4ff55b99415646677bed69a99c3214b) *(about)* Show PostHog and Registry configuration
- [`a351bb01`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a351bb014e6a65f9b3e066a66376002af7dc95e0) *(pgp)* Add command to show stored public key
- [`57e305af`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/57e305af82b714ebd621b09410d19caa8f07801c) *(upgrade)* Display changelog link after successful upgrade
- [`0cdde5d2`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0cdde5d282f60a0ec5c5bf605c196da5a26b4afe) *(pkg)* Enhance repository filtering and display
- [`ab965d48`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ab965d48a740ab81e09fee38e793e979151a75ea) *(man)* Generate man pages for subcommands
- [`71dd9b5e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/71dd9b5edfe4c693918c177e9479f9330df9080c) *(pkg)* Refine build command mapping for OS platforms
- [`48ddbda7`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/48ddbda769e75040967e9bff16c583b9feb70f72) *(meta)* Add meta command to generate resolved package JSON
- [`54868fb3`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/54868fb33f8a3e28efc8edc678a3464429f85508) *(resolve)* Add support for direct git package sources
- [`8ba54fe2`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8ba54fe2fd2642983cbe46d2bf091aac1fc5150e) *(pkg)* Enhance package resolution and initial config
- [`0f0f785c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0f0f785c069a5464e11e37b4c8d40f7f2b7854cd) *(registry)* Display descriptions and refine repo resolution
- [`debcee71`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/debcee71164faf6c814cbe64630b46c27f76195a) *(cli)* Add helper command
- [`b62f8e5b`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b62f8e5b30626a8bbc4e9b6d7757b7984873036e) *(registry)* Implement support for multiple package registries
- [`05e4c769`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/05e4c769ed9e3a55364f4adf9fc25b1d14e1a778) *(pkg)* Enhance package installation with PGP verification
- [`7e632235`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/7e6322355c275ecf6ff8117495f6f98cc9cb72d7) *(upgrade)* Warn when self-upgrading package manager installations
- [`913f490d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/913f490df3c2288d49c18ee1bdc98c5f1be9a3f2) *(install)* Implement installer package method and uninstall
- [`b72d42a4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b72d42a466c9166b2a4efae8944c68cbcde24120) *(install)* Prevent redundant manual installs after binary installation
- [`684fa7b4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/684fa7b4693def57c99d3f480fffa6bab4713a71) *(meta)* Add version argument for metadata generation
- [`dc712627`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/dc7126278f8dbcc6e13e19ae79d4cf871e41960c) *(cli)* Add hidden command to print man page
- [`ca3cdceb`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ca3cdceb5c249a97de64b38b43edfd1051165524) *(packaging)* Add man page generation to package builds
- [`a28e17ea`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a28e17ea671545484d3dd94b344ba3914b8713dd) *(lua)* Add fetch utility for making web requests
- [`299133e2`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/299133e21c1492370f521c16dbc9298efc97fd19) *(pkg/package)* Expand platform resolution for architecture inference

### ➡️ Migrations

- [`baeaa064`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/baeaa0647940f29f288706c42989e33850ba76d2) *(lockfile)* Introduce custom package lockfile

### 🎯 UX

- [`1569c7ad`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1569c7ad24b58cb97e858c3509a467eb302f55f2) *(pgp)* Add 'rm' alias for remove command

### 🏗️ Structure

- [`3b8a36e5`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/3b8a36e5f45b73240e2c6a89bd3e7bd7c9f985d8) *(scripts)* Rename build directory to scripts

### 🔒 Security

- [`1d0278bf`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1d0278bfa24d1f80351163eef607666aebe018e1) *(reporting)* Update vulnerability reporting guidelines

### 🔧 Configuration

- [`681fdfd9`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/681fdfd9a85020ae74e2aefcacbb5e9c60afd3e8) *(registry)* Use build-time configurable default registry
- [`6c249336`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6c24933698201485f633cfd3f264b46dce60c361) *(Cargo)* Specify minimum Rust version

### 🛠️ Build

- [`bd4b233c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/bd4b233cdaf6862fcb82c621bd9ba299fe292663) *(build)* Refactor environment variable loading
- [`c62da5d8`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/c62da5d8d4160319391379f5edd8e0a5f5af5e5f) Update zoi.yaml
- [`5d96b384`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/5d96b384715b42c7aa6a9d83a305e9a07b68bb08) *(cargo)* Gate utility binaries behind 'tools' feature
- [`7f9d4c21`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/7f9d4c213f300500f508ac9d58e753de6766e737) Update Cargo dependencies and minimum Rust version to 1.88.0
- [`04e4d472`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/04e4d472d8366a76f8c07c21dfee6516ceaf5d68) Add '--bin zoi' to build scripts
- [`eccc1fb4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/eccc1fb4afcfba257cc374e706ecc57592798ed9) *(tools)* Add CLI completion and man page generation
- [`a5f3349d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a5f3349d2035dae150b3527a4ac3fb2d0f5c2fd8) Add 'build' make command
- [`4d47e920`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/4d47e92004b8b82d317fd881084d21c4295b1307) Add 'help' make command
- [`efe7eadf`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/efe7eadf8995d71a4692fb88a3f2f9dec476e17a) *(setup)* Consolidate shell configuration
- [`4fb8264b`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/4fb8264bed24b1d09c035315cdc5c401c1de6e9b) Remove FreeBSD/OpenBSD support
- [`1ac145c6`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1ac145c6ff44c680278a3e3463c891afde6e1c9d) Update build scripts

### 🛡️ Dependencies

- [`1b1b6330`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1b1b63306005a11e636420cf5b460d14f122f424) Update Cargo dependencies
- [`ed8e0fef`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ed8e0fefbafd15f52774beca2b015b4cc6409ec5) Add rayon parallel iteration library
- [`4ae18d43`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/4ae18d43da3e4036097054e7c0fb45d8f043fb57) *(cargo)* Remove unused cyclonedx-bom and purl crates

### 🩹 Bug Fixes

- [`29af2162`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/29af21628ce8672a6314284fdf271a67080c6c21) *(sync)* Use compiled-in default registry when unset
- [`0d48ba37`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0d48ba37fd0bfac80bdb98e813542654156982a2) Remove installed_at for zoi.lock
- [`0c6c33d4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0c6c33d4c842c7ae5df2564d86896553316be4cb) *(pkg)* Improve uninstall error handling and messages
- [`fe2203ce`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/fe2203ce5d53e9905fa84b5bc4378441951f1bdb) *(pkg)* Remove symlinks before package directory during uninstall
- [`0a55255f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0a55255fe3c6260dae41a2a76ffc308b4aa4f97a) *(install)* Prevent duplicate package installations
- [`613d1fef`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/613d1fef70201c9753fd63ec40d651cd72fe2883) Tests in lib.rs
- [`0277450f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0277450f159b9990c960d8e9c5d766293e083b05) *(pkg)* Ensure symlinks are removed on uninstall
- [`4b80db7b`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/4b80db7bb7b7ac4600515e48a163772a98a49f1c) *(packaging)* Use GitLab project ID for release fetching
- [`69f57ae4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/69f57ae4399d28ccb7f01b6f17e45ede2862f775) *(update)* Correct package resolution for update command
- [`05f83753`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/05f83753b33560ef539a2b53952d3ac65f3dfea6) *(path)* Correct PATH verification for custom definitions
- [`b707a1e2`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b707a1e211f5f91284e6e5b183ede7c57fdde256) *(pkg)* Prevent resolution of nested packages

## [Prod. Beta 5.0.5] - 2025-09-09

### ➡️ Migrations

- [`04ef5ff9`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/04ef5ff9062878a6b90afce7626eb0d0452f78d2) *(pkg)* Use Lua for package definitions

## [Prod. Beta 5.0.4] - 2025-09-09

### ✨ Features

- [`6b6da5f8`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6b6da5f8e3ebe65d3156e52b15f84f46de120a78) *(package)* Add custom file staging and installation

## [Prod. Beta 5.0.3] - 2025-09-09

### ✨ Features

- [`097ba5b7`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/097ba5b7b815d2dddf9ecc5d8c312d2b27b52526) *(package)* Add Docker build support for source packages

## [Prod. Beta 5.0.0] - 2025-09-09

### ♻️ Refactor

- [`801f012e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/801f012e85b8b0686ce55354ac6b76fa16b8ce08) *(pkg)* Simplify archive filename and URL template
- [`1b73e15e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1b73e15e40ca12cb66ddba75508f3aa754ea6e03) *(cmd)* Adapt modules to new package resolution signature
- [`37627a47`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/37627a479dfd3c8405393e9efe97e3ae9e52202a) *(pkg)* Remove dynamic variable replacements
- [`111be67b`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/111be67b7a7e734b9d26e6cfbca6c7d4367ac8ff) *(cli)* Restructure update command arguments and improve help output

### ✨ Features

- [`eeb9ba59`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/eeb9ba59a9e45596de10eb43d03d2c44adf8ed99) *(meta)* Allow specifying installation type for meta generation
- [`7050c0f2`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/7050c0f238e0b0cd34a7854a3e5c60ee22bb49b4) *(api)* Expose core functionality as public library API
- [`4a296a88`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/4a296a88c65daa5022762518aaddd66c045b2dd7) *(package)* Add source installation support
- [`3d06c4fe`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/3d06c4fe67b8b00659593d038883262a8841bd0e) *(pgp)* Add command to search PGP keys
- [`327c8a58`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/327c8a5844c4615c45caadda564114fc99673615) *(package)* Add multi-platform build capability
- [`b66e4c28`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b66e4c28472c1e356ccc9d0429e2a1ac84c29729) *(pkg)* Support direct package names in repo installs
- [`5d87b9fc`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/5d87b9fcc0d9fb86a74a5753f228b8d316777a0c) *(install)* Add support for installing from git repositories
- [`f4076bb4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f4076bb4f5fd3ba14fcf9108cbde24af0baa4db4) *(pkg)* Add package installation scope
- [`0e8fb6b5`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0e8fb6b585ce96a70e8e1f95e12623f2e185cfc8) *(pkg)* Implement meta-build-install and update package resolution
- [`010abef9`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/010abef9c7ff7cf9e7de8a98a455f942802d023e) *(pkg)* Implement pre-built package installation from repos
- [`bc5716e8`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/bc5716e8bd32ba621e0a596151bdd1f4de0684c9) *(pgp)* Implement PGP key import from URL and list command
- [`1e0b0f5e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1e0b0f5e0e889b5e97bdca174f0ca4af1e416bc5) *(pgp)* Add PGP key management
- [`196f8491`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/196f849181aa8357dc47d394db32ce9d0b59bb32) *(package)* Add package install command
- [`0ded3387`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0ded33870ce00a893cb00f2f02d7dc94500d6c4f) *(package)* Add CLI commands for package creation
- [`cd787258`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/cd787258a7beac90fdbda33ba7b74115a55a665c) *(pkg)* Support structured package database
- [`034a59bb`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/034a59bbb8a7aa55f48b8291a67bde2dd1c70fbe) *(extension)* Allow extensions to manage project configuration file
- [`da25b120`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/da25b12029e2b369aa7c5c560b5d2332d5ba5adc) *(script-handler)* Implement script package uninstallation
- [`87aa8f1a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/87aa8f1a16f8b33cab881eb645f9f8305254189f) *(man)* Enhance man command with local caching and raw display
- [`0635e245`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0635e2458f578ee5c063b278c99d840b674bf4dc) *(dev-setup)* Implement comprehensive testing and formatting
- [`6f92aa09`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6f92aa09fa439f3136efa847333e6ea8f4af029d) *(cli)* Add new 'man' command for package manuals
- [`ab1c74ad`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ab1c74ad13ecdec1e29f5b7668b5f97e3ff5d149) *(script)* Add support for script package type
- [`1cb7807f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1cb7807fb7932b10b9b8b55cbdf7032b393da1e1) *(show)* Display package installation status
- [`7e830725`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/7e8307258e7e18f0f4cc3605825ed6410dd6e153) *(pkg)* Implement interactive package selection
- [`dd97d915`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/dd97d915dba6444f77f89dcb2c5f664967be0edb) *(install)* Add --all-optional flag to install command
- [`dd8d0769`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/dd8d0769697380fa8eb00091061f3ac5b6edef9a) *(show)* Add license verification
- [`9150ca86`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/9150ca86a17cca0785bc5be65a4c84633dc32d93) *(cli)* Enhance package completions and auto-setup
- [`2f068377`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/2f068377f450075150b0dc47af708df3fe7b38b1) *(config)* Allow platform-specific commands and environment variables
- [`fcd1c770`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/fcd1c77012f2bbe8e985c80717c6e5d75f7d0b97) *(sync)* Add --no-pm flag to skip package manager checks
- [`6afa42d9`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6afa42d96ac9986d0e33c01c7aa7e93259d01854) *(sync)* Add fallback mirrors for package database
- [`1ec76c7c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1ec76c7cce917529bce611eb99a3278ebf373a3f) *(gemini)* Add AI flow for GitLab operations

### ➡️ Migrations

- [`d91a2c1d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/d91a2c1d42d5d20239d3444b7a68417d8bde0882) *(pkg-format)* Switch to Lua for package definitions
- [`b41dee2f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b41dee2fed84c12a022affd0e496174751919cce) *(parser)* Transition to Lua package definitions
- [`86ec8059`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/86ec80590a6cf0209608c5b0e6c28667f543af6d) *(scripts)* Migrate install scripts to zillowe.pages.dev

### 🎨 Styling

- [`3006624b`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/3006624b900fa721f9834fad62f6a01a882b0727) *(cli)* Add custom colors and styling to CLI output

### 🎯 UX

- [`8c710a28`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8c710a2862cec385ed593a9066bd1bcd4d6eb8ba) *(cmd)* Condense repository names in list and search output
- [`dfb20d07`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/dfb20d073a3d701ea674d672f09ee28577f41c9d) *(cli)* Add package name suggestions to CLI arguments

### 🔒 Security

- [`51a6afce`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/51a6afce1372170f6d3d942d71bd4884ab0a115b) *(package)* Implement PGP signature verification
- [`d82c9288`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/d82c928846d88e86a5fdf8efcfd7771edd4cea76) *(install)* Implement GPG signature verification

### 🔧 Configuration

- [`1201c6c6`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1201c6c627d20e14914243f098e9dd96efe686c7) *(about)* Add contact email to about command
- [`ac17064d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ac17064dd3ed4e19b2f145f4c598baeebdf8eaa5) *(gemini)* Set up client credentials

### 🛠️ Build

- [`125217f1`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/125217f19ac130cbc4953c329af70827f400994f) *(sync)* Load sync fallbacks from repo.yaml
- [`c4111789`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/c4111789c0257d0b2c679f7f3e38b1f84bb5c609) *(docker)* Add Docker build configuration

### 🛡️ Dependencies

- [`8574774a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8574774a8f3e081034a38511c3dcb384feb46aa2) *(cargo)* Add mlua crate

### 🧹 Cleanup

- [`6711b6c1`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6711b6c1e7b7fa0aff55ace362ca78f783197cd4) *(cli)* Remove interactive package creation command

### 🩹 Bug Fixes

- [`0cd31458`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0cd31458375cb3646cf577899b18dc0d5ecb681d) *(build)* Correct checksum mismatch error message formatting
- [`fdac7ea7`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/fdac7ea7d9985a3c889dc9b48a60b9899771b7cf) *(windows)* Initialize colored crate output

## [Prod. Beta 4.3.7] - 2025-08-20

### ♻️ Refactor

- [`6d1bdb13`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6d1bdb13fad8449e7caec360d8eb7ca384e13aff) *(dependencies)* Remove pre-installation conflict checks
- [`1320ddaf`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1320ddafae636af04622d09f889fea780f8c48bb) Enhance package resolution and CLI output

### ✨ Features

- [`a7c6b123`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a7c6b1237edd8326b754f70cdad993961d94366e) *(pkg)* Add package update command
- [`6333d541`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6333d5415ecd0e9370b5e60be3358094789cbad8) *(exec)* Execute commands via shell

### 🧹 Cleanup

- [`4ffbdc73`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/4ffbdc733a46085102ba864d01fdddfb3889bb06) *(pkg)* Remove external command conflict check

## [Prod. Beta 4.3.6] - 2025-08-19

### ♻️ Refactor

- [`2e807772`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/2e807772f5fa32454a6d95b6a119dd1917a49873) *(pkg-resolve)* Remove alt source caching and improve download reliability
- [`b4b75eb4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b4b75eb473f4736e5ba3a9266da2cff2aac5b431) *(path)* Refine PATH check output logic

### ✨ Features

- [`4288c783`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/4288c783423dc3d906435cd328ef674326c06dcc) *(install)* Add license validation to packages

### ➡️ Migrations

- [`d2f82770`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/d2f827708397a55c5a9da0207fd34b02d12da823) *(sbom)* Migrate package recording to CycloneDX SBOM

## [Prod. Beta 4.3.5] - 2025-08-18

### ✨ Features

- [`b0349dbb`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b0349dbbd782df4010edfaa0726f88e29190fcbc) *(shell)* Add Elvish shell path setup
- [`77a03dd6`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/77a03dd68120ec8f95d8e2297cebada215668b36) *(shell)* Add setup command to configure shell PATH

### 🎯 UX

- [`4e0f797a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/4e0f797a228b3e0001fdacc04c2926f2f5ea9525) *(path)* Enhance PATH warning for better user guidance

## [Prod. Beta 4.3.4] - 2025-08-18

### ✨ Features

- [`87421dc9`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/87421dc965985ae0370a9983fa8ea80686acdc02) *(install)* Add package recording and lockfile installation

## [Prod. Beta 4.3.3] - 2025-08-17

### ✨ Features

- [`cf91a8c7`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/cf91a8c758b6014b91350dfcbffc53d3be0d39bc) *(pkg)* Add sharable install manifests

## [Prod. Beta 4.3.2] - 2025-08-17

### ♻️ Refactor

- [`6d8a80f1`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6d8a80f1bf1f6f0a6ad0c4d461c1d94e64b502f8) *(deps)* Specify optional dependency type in installation output
- [`160fedb7`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/160fedb7a74b74188419d89205e1c1c3babad6dd) *(upgrade)* Streamline patch upgrade by using current executable

### ✨ Features

- [`615379e0`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/615379e09a2b36a29619ca967cea2ed4d759ca42) *(service)* Add Docker Compose support
- [`e7f5ffd1`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e7f5ffd16a8f79eac891ce1c6ac69fb037122b0a) *(about)* Include documentation URL in output

## [Prod. Beta 4.3.1] - 2025-08-16

### ♻️ Refactor

- [`a13e59db`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a13e59dba75349eacbc9e58e6f22160eb9ab6bf5) Address Clippy warnings across codebase

### ✨ Features

- [`d54cea42`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/d54cea42b30eb7377d545b9026b685df7684b0e7) *(pkg)* Prompt user with important package updates
- [`82829a58`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/82829a584cddd279b574274f1fce52b0a2bb3085) *(pkg)* Add library package type and pkg-config command
- [`50711805`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/507118050cb5246996650574686e8df1134b2755) *(pkg)* Add rollback command and functionality
- [`167b83da`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/167b83da9681261cd1829bc70aa758a01f2a78f3) *(extension)* Add extension management commands
- [`9f07d860`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/9f07d8605294e0f32451abaf72b98ea4cb95dec1) *(config)* Manage external git repositories

### 🛠️ Build

- [`e1386a0e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e1386a0e611efdc0a37cf31c7ea91c8ebb88eb55) Add dedicated lint command

## [Prod. Beta 4.3.0] - 2025-08-15

### ♻️ Refactor

- [`6250a894`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6250a894c45ee2c1556d3193a000997d51910057) *(pkg)* Improve source install binary linking

### ✨ Features

- [`5bdd7299`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/5bdd7299889ba1788f7e1a3255c54f0ec62fb8ca) *(search)* Paginate search command output
- [`bff37e81`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/bff37e8197fc4c8900ff9a09a564df632e038c77) *(git)* Add Codeberg support for latest tag resolution
- [`418b66b1`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/418b66b14698c342cad495c9f75fc9b985c6c9df) *(pkg)* Add {git} placeholder to package install URLs
- [`44f2e83f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/44f2e83f83b7296e29f6a0b46b62182538f32d61) *(show)* Add specific binary types to package info
- [`29daac2d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/29daac2d727d01676a5b160fb0dca03263be698d) *(pkg)* Allow {git} placeholder in install URLs
- [`96392f27`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/96392f2749d9a1f2e350c3908eeb242fdbf74951) *(install)* Implement binary package installation
- [`6bb434cd`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6bb434cdeb9657f2dde9e30fcdc3f42cb5574ea0) *(upgrade)* Allow specifying tag or branch for upgrade
- [`c70a2434`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/c70a2434b6d10bbd9d70a0b05cf986814ca938ac) *(shell)* Add shell command for completion management

### 🛠️ Build

- [`fc55c7b2`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/fc55c7b23029973c517736ef9bf8d602e4002311) *(release)* Add notes script to CI artifacts

### 🩹 Bug Fixes

- [`e30508bb`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e30508bb0d3e8c03231c3f6724b6617b1de64761) *(ci)* Fixing CI add bash
- [`f89200dd`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f89200dd7416982604c0a358f2cbb08091467360) *(pkg)* Conditionally compile symlink calls for Unix

## [Prod. Beta 4.2.3] - 2025-08-13

### ✨ Features

- [`844fe05b`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/844fe05bbb29a993113afecdc42c359fe9f089c9) *(pkg)* Resolve package versions from Git release tags

## [Prod. Beta 4.2.2] - 2025-08-13

### ✨ Features

- [`6bd31f34`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6bd31f34394e648df508c4380ccd60ea2fddc801) *(upgrade)* Add full and force options

## [Prod. Beta 4.2.1] - 2025-08-13

### 🎯 UX

- [`78394709`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/78394709fe3a0519dbf13cf2b7de8da87c730cd0) *(cli)* Improve auto-completion for source arguments

### 🩹 Bug Fixes

- [`2ee338d8`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/2ee338d8694bcebe288b2fce51fc4ac76eed5e7f) *(dependencies)* Fix parsing for package names starting with '@'

## [Prod. Beta 4.2.0] - 2025-08-12

### ♻️ Refactor

- [`aa105aba`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/aa105aba688e0cb6e7d040621bf7bb1b2f421462) Update Config.toml

### ✨ Features

- [`0f1afd74`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0f1afd7435e2ce2af26b0ac8233770d64ffcbb64) *(sync)* Add registry management for package database
- [`0d03f3e4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0d03f3e487acc7087b5e2bde9b38438a51a3119f) *(pkg)* Allow nested paths for git package sources
- [`98ad7894`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/98ad789436bd512052de20332b23f0aa11106ccb) *(pkg)* Improve conflict detection

### 🏗️ Structure

- [`726f9739`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/726f973902a5f62a9f4087bb858ec6fb4ec53f8c) *(core)* Rename package and restructure as library

## [Prod. Beta 4.1.3] - 2025-08-12

### 🛠️ Build

- [`820de5cc`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/820de5cc9719372e6eb3d5d90b79fd373df5d760) *(pkg)* Enhance dependency resolution robustness

## [Prod. Beta 4.1.2] - 2025-08-11

### ✨ Features

- [`e73211e1`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e73211e1fc8c2c6c962c3440bfa19233640200ca) *(cmd)* Pass arguments to custom commands
- [`d802d499`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/d802d4996d93ee9c6193fb29ff477ecf05b06af6) *(cmd)* Add interactive package file creation command
- [`4b896345`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/4b896345e9bbf8d06afe8a37f45f170e6cbf7a53) *(schema)* Add JSON schema for pkg.yaml validation

### 🔧 Configuration

- [`a8dc9098`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a8dc9098741343721659a018aa9f2c38c79eafcc) *(pkg-config)* Define Zoi package configuration schema

## [Prod. Beta 4.1.1] - 2025-08-11

### ✨ Features

- [`4c6aed5e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/4c6aed5e8fef6b1bae4ba28614ba84175fb57868) *(create)* Add pre-creation check for existing app directory
- [`6980e114`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6980e114a69cbc09f3cc0604642f53a07f4f3093) *(cmd)* Add 'create' command for application packages

## [Prod. Beta 4.1.0] - 2025-08-11

### ✨ Features

- [`68d78dac`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/68d78dac619644af7a5bec263a1e49d1aa038ca0) *(pkg)* Add conflict detection for Zoi packages

### 🛡️ Dependencies

- [`e36051d6`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e36051d6d64c12400c9933fb3567d5f1471f00bc) Update

## [Prod. Beta 4.0.4] - 2025-08-09

### ✨ Features

- [`a551b128`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a551b12852025a4bd4cd764db0fd73ed1f657c8a) *(pkg)* Add script and Volta package manager support
- [`e55478a2`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e55478a262943d534238feda82e24cd5623c34e2) *(deps)* Add support for dependency versioning

## [Prod. Beta 4.0.3] - 2025-08-09

### ♻️ Refactor

- [`5241953e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/5241953efd4a3cda368ccb1404566e4297e45939) *(cli)* Enhance input parsing and error handling

## [Prod. Beta 4.0.2] - 2025-08-09

### ✨ Features

- [`6e4aaa74`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6e4aaa747cd1698e420bea562c14c7a8408d7da4) *(pkg)* Add readme field to package type
- [`a7183c71`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a7183c71ca254eb6fa2ca5b34585d02647cc5791) *(telemetry)* Include package version

## [Prod. Beta 4.0.1] - 2025-08-09

### 🛠️ Build

- [`2121be1f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/2121be1f21a82e09a7aaaed0ebf68056ba7fd18f) *(build)* Use dotenvy for environment variable loading

## [Prod. Beta 4.0.0] - 2025-08-09

### ✨ Features

- [`24d981ee`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/24d981eec12f73ae40d2559e6d98946f93d294d0) *(telemetry)* Add opt-in usage analytics
- [`6158f050`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6158f0504e9ce88ac97501990f37be4b1bb00583) *(install)* Add tag and branch options for source installs
- [`b9c0673c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b9c0673c438c2222458cbe50097797d7f111c3a5) Introduce package tags and improve network resilience

### 📈 Tracking

- [`66627608`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/666276087fc6e3ff9e95145767a1a860a588a9db) *(telemetry)* Track clone, exec, and uninstall commands

### 🔒 Security

- [`8b6b9a20`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8b6b9a2025f8250b90c41b17b3d6145ba09b5434) *(pkg)* Warn on insecure HTTP downloads

### 🛡️ Dependencies

- [`ef787b10`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ef787b101e68b981ebc976d9fb5f7137f1a881d7) *(cargo)* Update and clean up dependencies

## [Prod. Beta 3.8.2] - 2025-08-08

### ✨ Features

- [`e288a0f9`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e288a0f9451cdcf952be0af13298060aa3b897ce) Add support for windows-arm64 binaries

## [Prod. Beta 3.8.0] - 2025-08-08

### ♻️ Refactor

- [`256c59d3`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/256c59d3ddde30b062337b6c40b667c23617295b) *(build)* Improve binary patch generation and application

### ✨ Features

- [`8329d885`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8329d8851f43799439cf7495ef91913367bf659b) *(deps)* Expand supported package managers and document dependencies
- [`18ec0c0e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/18ec0c0e11ba2950d841cd1d6781074ae087d44d) *(repo)* Add git subcommands and command aliases
- [`9850b598`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/9850b59875ac66e27cbebddc7a8c3fe6853b7db7) *(deps)* Enhance dependency schema with selectable options

### 🎯 UX

- [`7dcb6198`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/7dcb619882e3eb7392743c627a994b2127629309) *(dependencies)* Enhance dependency output format

## [Prod. Beta 3.7.2] - 2025-08-07

### 🛠️ Build

- [`6c30ae72`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6c30ae72f142793bc6192b5b4e76e5a941f4b872) *(upgrade)* Adjust patch upgrade strategy for archives

## [Prod. Beta 3.6.0] - 2025-08-07

### ♻️ Refactor

- [`f6d350fa`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f6d350faef608d4469ced260c694e74e4da80b0e) *(pkg)* Migrate GPG signature verification

### ✨ Features

- [`d878267e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/d878267e1e85e88842958443e6c0981ca00898ac) *(security)* Add GPG key fingerprint support

## [Prod. Beta 3.5.0] - 2025-08-06

### ♻️ Refactor

- [`6896d228`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6896d2282cf6aeab7ffff874fcd8d46da00c3d29) Move from 'sh' and 'cmd' to 'bash' and 'pwsh'

### 🔒 Security

- [`afbe43cd`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/afbe43cdd5555ff23522261d0e0b172c28c92374) *(pkg)* Implement GPG signature verification for package artifacts

## [Prod. Beta 3.4.2] - 2025-08-06

### ✨ Features

- [`51f9bf9c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/51f9bf9c7c13e46513a59dbce028b16c072c213f) *(pkg)* Add pre-installation conflict detection
- [`7b88c07d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/7b88c07d48a81c154595b28b340149fd2a0d9f55) *(pkg)* Improve dependency handling and uninstallation

## [Prod. Beta 3.4.1] - 2025-08-05

### 🩹 Bug Fixes

- [`7e5607e7`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/7e5607e775657887f16bfb3f1e369db63ca07a03) *(upgrade)* Standardize version parsing for releases

## [Prod. Beta 3.4.0] - 2025-08-05

### ✨ Features

- [`779c0960`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/779c0960c7d25ee44ad1e7b06bb0320431d83762) Enhance package management and CLI command capabilities
- [`f42f5ed0`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f42f5ed0456b69be1c178f412e3c5b13d07d5b3e) *(install)* Enable multi-package installation
- [`5c71aada`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/5c71aadaf76c6840274b1d7a13229dbafa9735f2) *(sync)* Add external Git repository synchronization

## [Prod. Beta 3.3.2] - 2025-08-04

### 🩹 Bug Fixes

- [`29d377a1`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/29d377a1d56c84d547b47b5285661be5232afa7c) *(patch)* Refine binary patch handling

## [Prod. Beta 3.3.1] - 2025-08-03

### ✨ Features

- [`04041b2c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/04041b2c06551de44b5a286da0306c53b2eab54d) *(pkg)* Enhance package installation and resolution

## [Prod. Beta 3.3.0] - 2025-08-03

### ✨ Features

- [`22b53bc8`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/22b53bc89f48ca27032d9942e698c76939c08235) *(repo)* Allow adding git repos as package sources
- [`813eab27`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/813eab27f00f41d0cacd0fb8bd5f1fe6a6b44f16) Add optional dependency resolution and CLI aliases

## [Prod. Beta 3.2.7] - 2025-08-02

### ✨ Features

- [`e6534b44`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/e6534b44c6c253863ddd8d354cfeba1f51a70f44) *(pkg)* Add MacPorts and Conda package manager support

## [Prod. Beta 3.2.5] - 2025-07-31

### ♻️ Refactor

- [`884bd7b7`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/884bd7b763ab5a1ff86a34b6b5c545f0f78dec70) *(upgrade)* Use 'no_' methods for HTTP compression

## [Prod. Beta 3.2.3] - 2025-07-31

### ✨ Features

- [`947df91d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/947df91d293854d39ad1f2c3607657698aad9db9) *(upgrade)* Display download progress for patches

## [Prod. Beta 3.2.2] - 2025-07-31

### ✨ Features

- [`2a78ba09`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/2a78ba094da4b428a8e1bc2ec674189909846c21) *(pkg)* Add support for more dependency managers

## [Prod. Beta 3.2.0] - 2025-07-30

### ✨ Features

- [`b33bc9ce`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b33bc9ce310d1d497ff4d227f4749019037310c6) Introduce service and config package types

## [Prod. Beta 3.1.9] - 2025-07-30

### ♻️ Refactor

- [`385f9fe5`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/385f9fe597ce55d97ff26fd521a51318d40d956b) *(pkg)* Update remote version resolution

## [Prod. Beta 3.0.0] - 2025-07-29

### ♻️ Refactor

- [`c25d11d7`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/c25d11d7397756e23c9f44fc90735f77ae798f8d) *(cmd)* Upgrade command
- [`1feb4535`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1feb4535af5cc74f0f0309b0037c3812ac77b946) *(upgrade)* Enhance version comparison for self-update

### ✨ Features

- [`60d7270b`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/60d7270bda32bf9cf4b4b214aed942afaa16f8d4) *(upgrade)* Implement patch-based self-update
- [`f41df585`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f41df5855286206637786b649bcbc1fc976e2dfc) *(pkg, cmd)* Implement flexible versioning and installation scopes
- [`fe2668bd`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/fe2668bd45be0134ea0fc48116e5728717c63c9b) *(pkg)* Add package collections and alt source resolution
- [`dbff8ee6`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/dbff8ee65e08987eea77f1f46e06a19805858c23) Enhance output and add repository warnings
- [`1061627e`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1061627e41b860f6a746c06cee98eed38e64ad40) *(platform)* Add FreeBSD and OpenBSD support

### 🩹 Bug Fixes

- [`1ef755bb`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1ef755bb9154574e22c934dd816e5c0ae118700f) Robustify environment PATH variable detection

## [Prod. Beta 2.5.6] - 2025-07-26

### ♻️ Refactor

- [`485d9842`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/485d9842e685e0ab21f737abea50f91df56f70ae) *(install)* Use 'macos' for Darwin OS

### ✨ Features

- [`c7e564ef`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/c7e564efd7c6d6402b49e037f3929c97c7cd1d8a) *(env)* Automate PATH setup and verification
- [`8b1f8097`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8b1f8097592f86e4edafd8def9c8a3efde01bdf1) *(installer)* Add shell completions to install scripts

## [Prod. Beta 2.5.2] - 2025-07-25

### ✨ Features

- [`903b27bf`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/903b27bfd100049436cfd85356225a47eed5cfe8) *(cli)* Implement global non-interactive mode
- [`957f85a7`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/957f85a77e941f1624260b0c3812cd222166edba) *(sync)* Improve package database sync

## [Prod. Beta 2.5.1] - 2025-07-24

### ✨ Features

- [`f6b4943d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f6b4943d67535b1ff407c053aadab94d07230c53) Allow repository filtering for list and search commands
- [`4c4911e0`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/4c4911e0af559da85b5fe654b496ebc207d2aae4) *(distribution)* Add Scoop package manager support

### 🩹 Bug Fixes

- [`b2448f37`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b2448f370a3d69b35ca558ddec1cff4c4bd68057) Version.json

## [Prod. Beta 2.5.0] - 2025-07-23

### ✨ Features

- [`8c531a16`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/8c531a16ee96b8765dfd49b2c9e7343fa1829cd5) *(pkg)* Add interactive install and compressed binary method
- [`7b9082b7`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/7b9082b7add9e889a1aea61f1da70bd683d95dde) *(packaging)* Add AUR and Homebrew support

## [Prod. Beta 2.4.0] - 2025-07-22

### ♻️ Refactor

- [`73baf123`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/73baf123bbefb2e0442f44db0cf7dba3e78920fd) *(repo)* Extract repository listing into helper

## [Prod. Beta 2.3.0] - 2025-07-21

### ✨ Features

- [`6a55c856`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6a55c856e152653507cf609d65bffc1de5407d8d) *(repo)* Add repository management commands
- [`17aa1483`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/17aa1483ba7c833e47811cb5a3e4c4183381f780) *(pkg)* Implement new package management commands
- [`ec8ed0cb`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ec8ed0cb75abf004d88fbf9925be271a59f3105d) *(version)* Add version management script

### 🎨 Styling

- [`4b093af3`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/4b093af37259541692da38061b0cb2bb060790a0) *(rustfmt)* Enforce consistent code style

### 💼 Other

- [`1a4fd60c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/1a4fd60c66a019f7acfca32e5ca7a34c25db1afb) Automated CI fixes for 'main'

## [Prod. Beta 2.1.1] - 2025-07-18

### ✨ Features

- [`6dc009ff`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6dc009ff93c15381eb4365ec389c15703d9ec8b2) Implement Zoi self-upgrade command

## [Prod. Beta 2.0.0] - 2025-07-16

### ♻️ Refactor

- [`83d08053`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/83d08053add1a36c677faa765d84b7e86160729e) Rust rewrite
- [`ce57e5c8`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/ce57e5c85738e73aee46a843c06a4c26aa75d559) Change GitLab repo namespace

### ✨ Features

- [`3c261462`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/3c2614626cc5724061991c906a275bc8cacd6b7c) *(pkg)* Add signing
- [`f74dfc0d`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f74dfc0d33f1e88a15774fb5aab3d2eb8b8383a0) *(pkg)* Add 'pkg doctor' command
- [`bcacf88f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/bcacf88fb71fc49aa123df1f273b39744233c969) *(pkg)* Add 'pkg update' command
- [`0b412809`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/0b4128097dbf6f3250b70f5d46e0d3f9f3d9d163) *(pkg)* Add install dependencies logic
- [`c6e6aeef`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/c6e6aeef8f03e606f14356185bfa7e388ceb7002) New package 'search' and 'list' commands

### 🧪 Testing

- [`9b0e49e3`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/9b0e49e39b1e26cef348cc18a9d526c06d1b22f4) Testing Codeberg's CI/CD

## [Dev. Pre-Alpha 2.4.0] - 2025-04-27

## [Dev. Beta 1.2.1]

### ♻️ Refactor

- [`f1badc5a`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/f1badc5a2fa3bddf18e57fb052e90c1b9bec49c8) *info* Source system details from configuration.

### 🩹 Bug Fixes

- [`b8b0961c`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/b8b0961c9b00b3038c2a9aaa1bf4e4b6fb94c6b6) *cmd* Removed redundant error print and unused import.

## [Dev. Beta 1.2.0]

### ✨ Features

- [`11d0f583`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/11d0f583c6ee100d16c637f1c26bcd71db700922) *env* Made command interactive.
- [`bb9f64ff`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/bb9f64ffc6aa5e2b4b9de240c9ff5051c0aaf198) *env* Implemented Go version manager.

## [Dev. Beta 1.1.0]

### ✨ Features

- [`151f2f45`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/151f2f4588e1a1a956202b69e449b7344a8fde14) *update* Added --force flag for reinstallation.
- [`cee2da83`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/cee2da8374f56648b6017cd1d4a5f7a05a366022) *set* Added interactive mode for config values.
- [`6fc339b4`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/6fc339b408a18b11c66352d5ed587b28376ccc78) *run* Allowed interactive command selection.

## [Dev. Beta 1.0.0]

### ♻️ Refactor

- [`a2b4f4a5`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/a2b4f4a5cc65802f0d54e1bca24bca1b21f84619) Moved to Cobra for command-line tool and Viper for config.

## [Dev. Alpha 3.2.0]

### ✨ Features

- [`7c35bd9f`](https://gitlab.com/zillowe/zillwen/zusty/zoi/-/commit/7c35bd9fcbaf8d772ea9fdc9641a507f3d45893b) Added update packages before installing.

## [Dev. Alpha 3.1.0]

### ✨ Features

- Added a bunch of features.

## [Dev. Alpha 3.0.0]

### ✨ Features

- Added `uninstall` subcommand to `zoi vm`.
- Allows users to remove specific installed language versions (e.g. `zoi vm uninstall go@1.20.0`).

### ♻️ Refactor

- Improved main `zoi` help message clarity.
- Refined command descriptions in the global `zoi help` output for better readability and understanding.

## [Dev. Alpha 2.0.0]

### ✨ Features

- Added version managing command.
- Added Go and Python version managing.

## [Dev. Alpha 1.0.0]

### ♻️ Refactor

- Major code rewrite and reformat.
- Moved the commands to a `commands` folder.
- Better code structure and better code overall.

## [Dev. Pre-Alpha 2.4.0]

### 🛠️ Build

- Added update command that update Zoi.
- Added build all script that build arm64/amd64 versions of linux/macos/windows.

## [Dev. Pre-Alpha 2.3.0]

### ✨ Features

- Added install command that install system packages.

## [Dev. Pre-Alpha 2.2.0]

### ✨ Features

- Added set command that set the apps url in a config file.

## [Dev. Pre-Alpha 2.1.0]

### ✨ Features

- Added check command that checks network and golang + git versions.

## [Dev. Pre-Alpha 2.0.0]

### 🛠️ Build

- Added build scripts for Linux/MacOS and Windows.
- Moved from NodeJS to Golang.

### ➡️ Migrations

- Command usage for make changed from json file to yaml.

## [Dev. Pre-Alpha 1.4.0]

- Making the structure better.

## [Dev. Pre-Alpha 1.3.0]

- Adding the ability to create apps from a local json file.

## [Dev. Pre-Alpha 1.2.0]

- Making the app fetch the frameworks and apps from the website.

## [Dev. Pre-Alpha 1.1.0]

- Another rewriting some of the files.
- Rewriting some of the files.

## [Dev. Pre-Alpha 1.0.0]

- Adding the ability to create new apps and frameworks, adding Ruby on Rails support.
- The main foundation of the project.
- Init.
