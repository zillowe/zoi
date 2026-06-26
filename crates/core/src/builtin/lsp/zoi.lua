---@meta
-- Zoi Package Definitions for lua-language-server (LSP)

---@class SystemInfo
---@field OS "linux"|"macos"|"windows" The operating system name.
---@field ARCH "amd64"|"arm64" The CPU architecture.
---@field DISTRO string? The Linux distribution ID (e.g., "ubuntu", "arch").
---@field DISTRO_VER string? The version of the distribution (e.g., "22.04", "14.1").
---@field DE string? The detected Desktop Environment (e.g., "kde", "gnome", "windows").
---@field SERVER string? The display server in use (e.g., "x11", "wayland", "quartz").
---@field KERNEL_VER string? The kernel version of the operating system.
---@field CPU string? CPU model information.
---@field GPU string? GPU model information.
---@field MANAGER string? The detected native package manager (e.g., "apt", "pacman").
SYSTEM = {}

---@class ZoiInfo
---@field VERSION string The version of the package being built.
---@field PATH { user: string, system: string } Zoi installation paths.
---@field PKG { store: string, template: string, root: string, home: string, lua: string } Package-specific absolute paths.
ZOI = {}

---@class Maintainer
---@field name string Maintainer name.
---@field email string Maintainer email.
---@field website string? Maintainer website.

---@class Author
---@field name string Author name.
---@field email string? Author email.
---@field website string? Author website.

---@class PkgMetadata
---@field name string Required. The name of the package.
---@field repo string Required. The repository tier (e.g., "core", "community").
---@field version string? The package version.
---@field revision string? The package revision (defaults to "1").
---@field versions table<string, string>? A map of channels to versions (e.g., { stable = "1.2.3" }).
---@field description string Required. A short description of the package.
---@field website string? The official website URL.
---@field git string? The source code's git repository URL.
---@field man string? A URL to the package's manual page.
---@field maintainer Maintainer Required. The package maintainer.
---@field author Author? The original software author.
---@field license string? The SPDX license identifier.
---@field bins string[]? List of binary names to link.
---@field conflicts string[]? List of conflicting packages.
---@field provides string[]? List of virtual packages provided.
---@field replaces string[]? List of packages this one replaces.
---@field backup string[]? List of config files to preserve during upgrades.
---@field types string[] Required. Supported build methods (e.g., "source", "pre-compiled").
---@field platforms string[]? Supported platforms (e.g., "linux", "macos", "windows").
---@field type "package"|"collection"|"app"|"extension"? The type of package.
---@field scope "user"|"system"|"project"? Default installation scope.
---@field sub_packages string[]? List of sub-package names.
---@field main_subs string[]? Default sub-packages to install.
---@field tags string[]? Keywords for search.
---@field readme string? URL to the package's README.
---@field rollback boolean? Whether to enable rollback for this package.
---@field installed_size integer? Expected size on disk (bytes).
---@field archive_size integer? Size of the pre-compiled archive (bytes).
PKG = {}

---@type string Absolute path to the temporary build directory.
BUILD_DIR = ""

---@type string Absolute path to the staging directory.
STAGING_DIR = ""

---@type "source"|"pre-compiled" The build method requested by the user.
BUILD_TYPE = ""

---@type string? For split packages, the name of the sub-package being processed.
SUBPKG = nil

---@class DependencyOptions
---@field name string Group name.
---@field desc string Group description.
---@field all boolean? Whether all options can be selected.
---@field depends string[] List of dependency strings.

---@class DependencyGroup
---@field required string[]? List of required dependencies.
---@field optional string[]? List of optional dependencies.
---@field options DependencyOptions[]? List of selectable dependency groups.
---@field sub_packages table<string, DependencyGroup>? Per-sub-package dependencies.

---@class TypedBuildDependencies
---@field types table<string, DependencyGroup> Map of build type to dependencies.

---@class Dependencies
---@field runtime DependencyGroup? Runtime dependencies.
---@field build (DependencyGroup|TypedBuildDependencies)? Build-time dependencies.

---@class Service
---@field run string The command to run the service.
---@field run_at_load boolean? Start on boot/login.
---@field working_dir string? Working directory.
---@field env table<string, string>? Environment variables.
---@field log_path string? stdout log path.
---@field error_log_path string? stderr log path.

---@class HookPlatformMap
---@field linux string[]? Commands for Linux.
---@field macos string[]? Commands for macOS.
---@field windows string[]? Commands for Windows.
---@field default string[]? Fallback commands.

---@alias HookCommands string[] | HookPlatformMap

---@class Hooks
---@field pre_install HookCommands?
---@field post_install HookCommands?
---@field pre_upgrade HookCommands?
---@field post_upgrade HookCommands?
---@field pre_remove HookCommands?
---@field post_remove HookCommands?

---@class UpdateNotice
---@field type "update"|"change"|"vulnerability"
---@field message string

--- Declares package static information.
---@param meta PkgMetadata
function metadata(meta) end

--- Declares package dependencies.
---@param deps Dependencies
function dependencies(deps) end

--- Declares structured update notices.
---@param notices UpdateNotice[]
function updates(notices) end

--- Declares lifecycle hooks.
---@param hooks_def Hooks
function hooks(hooks_def) end

--- Declares a background service.
---@param svc Service
function service(svc) end

--- Lifecycle: Fetch source code or binaries into BUILD_DIR.
---@param args { sub: string? }?
function prepare(args) end

--- Lifecycle: Compile and stage files into STAGING_DIR.
---@param args { sub: string? }?
function package(args) end

--- Lifecycle: Verify integrity and authenticity of downloaded files.
---@param args { sub: string? }?
---@return boolean
function verify(args) end

--- Lifecycle: Optional function to run integration tests.
---@param args { sub: string? }?
---@return boolean
function test(args) end

--- Lifecycle: Cleanup tasks outside the package store.
function uninstall() end

--- Executes a shell command within BUILD_DIR.
---@param command string
---@return string stdout, string stderr, integer exit_code
function cmd(command) end

--- Stages a file or directory for inclusion in the final package.
---@param source string Path relative to BUILD_DIR or ${pkgluadir}.
---@param destination string Destination using ${pkgstore}, ${usrroot}, etc.
function zcp(source, destination) end

--- Copies a license file to the package store (${pkgstore}/LICENSE).
---@param source string Path relative to BUILD_DIR or ${pkgluadir}.
function zlicense(source) end

--- Copies a documentation file to the package store (${pkgstore}/doc/{filename}).
---@param source string Path relative to BUILD_DIR or ${pkgluadir}.
function zdoc(source) end

--- Performs a regular expression replacement on a file within the build directory.
---@param pattern string The regex pattern to match.
---@param replacement string The string to replace the matched pattern.
---@param file string The path to the file relative to BUILD_DIR.
function zsed(pattern, replacement, file) end

--- Applies a patch file to the build directory using the 'patch' command.
---@param patch_file string The path to the patch file relative to BUILD_DIR.
---@param strip integer? The number of leading path components to strip (default is 1).
function zpatch(patch_file, strip) end

--- Creates a symbolic link in the package.
---@param target string
---@param link string
function zln(target, link) end

--- Sets permissions of a staged file or directory.
---@param path string
---@param mode integer Octal mode (e.g., 493 for 0755).
function zchmod(path, mode) end

--- Sets ownership of a staged file or directory.
---@param path string
---@param owner string|integer
---@param group string|integer
function zchown(path, owner, group) end

--- Creates a directory in the package.
---@param path string
function zmkdir(path) end

--- Removes a file/directory (used in uninstall()).
---@param path string
function zrm(path) end

--- Reads a file from the same directory as the .pkg.lua.
---@param filename string
---@return any content Parsed table for .json/.yaml/.toml, else string.
function IMPORT(filename) end

--- Executes another Lua script in the same directory.
---@param filename string
function INCLUDE(filename) end

--- Verifies a file's checksum.
---@param file_path string
---@param hash_spec string e.g., "sha256-..."
---@return boolean
function verifyHash(file_path, hash_spec) end

--- Verifies a PGP detached signature.
---@param file_path string
---@param sig_path string
---@param key_name_or_url string
---@return boolean
function verifySignature(file_path, sig_path, key_name_or_url) end

--- Adds a PGP key to Zoi's keyring.
---@param url_or_path string
---@param name string
---@return boolean
function addPgpKey(url_or_path, name) end

UTILS = {}
UTILS.FETCH = {}
--- Fetches a URL's content as a string.
---@param url string
---@return string
function UTILS.FETCH.url(url) end

---@class GithubLatestArgs
---@field repo string "owner/repo"
---@field domain string? Optional API domain.
---@field branch string? Optional branch.

UTILS.FETCH.GITHUB = { LATEST = {} }
---@param args GithubLatestArgs
---@return string
function UTILS.FETCH.GITHUB.LATEST.tag(args) end
---@param args GithubLatestArgs
---@return string
function UTILS.FETCH.GITHUB.LATEST.release(args) end
---@param args GithubLatestArgs
---@return string
function UTILS.FETCH.GITHUB.LATEST.commit(args) end

UTILS.FETCH.GITLAB = { LATEST = {} }
---@param args GithubLatestArgs
---@return string
function UTILS.FETCH.GITLAB.LATEST.tag(args) end
---@param args GithubLatestArgs
---@return string
function UTILS.FETCH.GITLAB.LATEST.release(args) end
---@param args GithubLatestArgs
---@return string
function UTILS.FETCH.GITLAB.LATEST.commit(args) end

UTILS.PARSE = {}
---@param str string
---@return table?
function UTILS.PARSE.json(str) end
---@param str string
---@return table?
function UTILS.PARSE.yaml(str) end
---@param str string
---@return table?
function UTILS.PARSE.toml(str) end
---@param content string
---@param filename string
---@return string?
function UTILS.PARSE.checksumFile(content, filename) end

--- Downloads a file from a URL to a local path.
---@param url string
---@param path string
function UTILS.FILE(url, path) end

UTILS.FS = {}
---@param path string
---@return boolean
function UTILS.FS.exists(path) end
---@param src string
---@param dest string
---@return boolean
function UTILS.FS.copy(src, dest) end
---@param src string
---@param dest string
---@return boolean
function UTILS.FS.move(src, dest) end
---@param path string
---@param mode integer
function UTILS.FS.chmod(path, mode) end

UTILS.FIND = {}
---@param dir string
---@param name string
---@return string?
function UTILS.FIND.file(dir, name) end

UTILS.ARCHIVE = {}
---@param path string
---@return string[]
function UTILS.ARCHIVE.list(path) end

--- Downloads and extracts an archive.
---@param source string URL or local path.
---@param out_dir string Subdirectory in BUILD_DIR.
function UTILS.EXTRACT(source, out_dir) end
