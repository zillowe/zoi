#!/bin/sh

# Checks a Linux distribution against the specifications programs and users
# rely on, section by section, each citing what it checks:
#
#   - freedesktop.org / systemd system files: os-release(5),
#     machine-id(5), hostname(5), machine-info(5), locale.conf(5),
#     vconsole.conf(5), localtime(5)
#   - Filesystem Hierarchy Standard 3.0, and systemd's file-hierarchy(7)
#   - freedesktop.org desktop specifications: XDG Base Directory, Icon
#     Theme, Shared MIME-info, Desktop Entry, D-Bus
#   - POSIX.1-2024 (IEEE Std 1003.1-2024, The Open Group Base
#     Specifications Issue 8): the utilities of each option group, the
#     shell's built-in utilities, a set of behaviour checks, the headers
#     (with --cc) and, through the C helper syscheck, the C library's
#     versions, options and limits
#   - common practice on mainstream distributions (Arch, Debian, Fedora):
#     accounts, system configuration, boot files and utilities
#
# It checks the running system, or an unbooted root such as the folder
# `zoi system distro build` creates (--root). With --root, nothing from the
# root is run, except a statically linked syscheck through chroot when run
# as root.
#
# Every result is one of:
#   PASS  present, valid, or working
#   FAIL  missing or invalid where the specification requires it, or most
#         programs rely on it
#   WARN  missing where the specification recommends it, or most
#         distributions provide it
#   INFO  optional, or worth knowing
#
# This is not a conformance certification (The Open Group's test suites
# run thousands of behaviour tests); it checks presence, validity and a set
# of behaviours.
#
# POSIX sh, so it runs where bash is missing. Exit status: 1 if any check
# failed, 2 on a usage error, 0 otherwise.
#
# Usage: distro-check.sh [--root DIR] [--network] [--syscheck FILE]
#                        [--cc COMPILER] [--no-color]
# `just distro-check [ARGS]` builds syscheck with clang and runs this.

ROOT=""
NETWORK=0
SYSCHECK=""
CC_CMD=""
COLOR=1
[ -t 1 ] || COLOR=0

usage() {
    cat <<'EOF'
Usage: distro-check.sh [OPTIONS]

Check a Linux distribution against freedesktop.org, FHS 3.0, POSIX.1-2024
and common practice.

Options:
  -r, --root DIR       Check the unbooted root in DIR instead of this system.
  -n, --network        Also check DNS and HTTPS (running system only).
  -s, --syscheck FILE  Run the C helper FILE for the C library checks
                       (versions, options and limits). With --root it must
                       be statically linked, and runs through chroot as root.
  -c, --cc COMPILER    Compile-test the POSIX headers with COMPILER (with
                       --root, against DIR's headers via --sysroot).
      --no-color       Do not color the output.
  -h, --help           Show this help.

Exit status: 1 if any check failed, 2 on a usage error, 0 otherwise.
EOF
}

need_arg() {
    [ "$2" -ge 2 ] || {
        echo "Error: $1 needs a value." >&2
        exit 2
    }
}

while [ $# -gt 0 ]; do
    case $1 in
    -r | --root)
        need_arg "$1" $#
        ROOT=$2
        shift
        ;;
    --root=*) ROOT=${1#--root=} ;;
    -s | --syscheck)
        need_arg "$1" $#
        SYSCHECK=$2
        shift
        ;;
    --syscheck=*) SYSCHECK=${1#--syscheck=} ;;
    -c | --cc)
        need_arg "$1" $#
        CC_CMD=$2
        shift
        ;;
    --cc=*) CC_CMD=${1#--cc=} ;;
    -n | --network) NETWORK=1 ;;
    --no-color) COLOR=0 ;;
    -h | --help)
        usage
        exit 0
        ;;
    *)
        echo "Error: unknown option '$1'." >&2
        usage >&2
        exit 2
        ;;
    esac
    shift
done

if [ -n "$ROOT" ]; then
    if [ ! -d "$ROOT" ]; then
        echo "Error: '$ROOT' is not a directory." >&2
        exit 2
    fi
    # No trailing slash, so "$ROOT/etc" is a clean path.
    ROOT=$(cd "$ROOT" && pwd)
    [ "$ROOT" = / ] && ROOT=""
fi

if [ "$COLOR" = 1 ]; then
    C_PASS=$(printf '\033[32m')
    C_FAIL=$(printf '\033[31m')
    C_WARN=$(printf '\033[33m')
    C_INFO=$(printf '\033[36m')
    C_HEAD=$(printf '\033[1m')
    C_OFF=$(printf '\033[0m')
else
    C_PASS="" C_FAIL="" C_WARN="" C_INFO="" C_HEAD="" C_OFF=""
fi

N_PASS=0 N_FAIL=0 N_WARN=0 N_INFO=0
# Per-section counts, printed in the summary.
S_NAME="" S_PASS=0 S_FAIL=0 S_WARN=0 SUMMARY=""

end_section() {
    [ -n "$S_NAME" ] || return 0
    SUMMARY="$SUMMARY$(printf '%-48s %3d pass %3d fail %3d warn' \
        "$S_NAME" "$S_PASS" "$S_FAIL" "$S_WARN")
"
}

# section NAME SPEC: starts a section; SPEC names what it checks against.
section() {
    end_section
    S_NAME=$1 S_PASS=0 S_FAIL=0 S_WARN=0
    printf '\n%s== %s ==%s\n' "$C_HEAD" "$1" "$C_OFF"
    [ -n "${2:-}" ] && printf '   %s\n' "$2"
    return 0
}

# report LEVEL MESSAGE
report() {
    case $1 in
    PASS)
        N_PASS=$((N_PASS + 1)) S_PASS=$((S_PASS + 1))
        color=$C_PASS
        ;;
    FAIL)
        N_FAIL=$((N_FAIL + 1)) S_FAIL=$((S_FAIL + 1))
        color=$C_FAIL
        ;;
    WARN)
        N_WARN=$((N_WARN + 1)) S_WARN=$((S_WARN + 1))
        color=$C_WARN
        ;;
    *)
        N_INFO=$((N_INFO + 1))
        color=$C_INFO
        ;;
    esac
    printf '%s%-4s%s  %s\n' "$color" "$1" "$C_OFF" "$2"
}

# Whether the running system is checked (not an unbooted root).
live() { [ -z "$ROOT" ]; }

# resolve PATH: follows symlinks of the last component inside ROOT, so an
# absolute link such as /etc/localtime -> /usr/share/zoneinfo/UTC is read
# relative to the checked root, not the host.
resolve() {
    r_path=$1
    r_hops=0
    while [ -L "$ROOT$r_path" ] && [ $r_hops -lt 40 ]; do
        r_target=$(readlink "$ROOT$r_path")
        case $r_target in
        /*) r_path=$r_target ;;
        *) r_path=${r_path%/*}/$r_target ;;
        esac
        r_hops=$((r_hops + 1))
    done
    printf '%s' "$r_path"
}

# exists TEST PATH: `test TEST` on PATH inside the checked root.
exists() { test "$1" "$ROOT$(resolve "$2")"; }

# The directories commands are looked up in, as on a typical PATH.
CMD_DIRS="/usr/local/sbin /usr/local/bin /usr/sbin /usr/bin /sbin /bin"

has_cmd() {
    if live; then
        command -v "$1" >/dev/null 2>&1
        return
    fi
    for d in $CMD_DIRS; do
        f=$(resolve "$d/$1")
        [ -f "$ROOT$f" ] && [ -x "$ROOT$f" ] && return 0
    done
    return 1
}

# check_cmds LEVEL LABEL CMD...: one line for the group, naming any missing
# commands.
check_cmds() {
    c_level=$1
    c_label=$2
    shift 2
    c_total=$#
    c_missing=""
    c_count=0
    for c in "$@"; do
        if ! has_cmd "$c"; then
            c_missing="$c_missing $c"
            c_count=$((c_count + 1))
        fi
    done
    if [ -z "$c_missing" ]; then
        report PASS "$c_label ($c_total/$c_total)"
    else
        report "$c_level" "$c_label ($((c_total - c_count))/$c_total), missing:$c_missing"
    fi
}

# check_any LEVEL LABEL CMD...: passes if at least one command exists.
check_any() {
    a_level=$1
    a_label=$2
    shift 2
    for c in "$@"; do
        if has_cmd "$c"; then
            report PASS "$a_label ($c)"
            return
        fi
    done
    report "$a_level" "$a_label: none of $*"
}

# check_path LEVEL TEST PATH DESCRIPTION
check_path() {
    if exists "$2" "$3"; then
        report PASS "$4 ($3)"
    else
        report "$1" "$4 missing ($3)"
    fi
}

# check_dirs LEVEL LABEL DIR...
check_dirs() {
    d_level=$1
    d_label=$2
    shift 2
    d_missing=""
    for d in "$@"; do
        exists -d "$d" && continue
        # A link into /run (such as /var/lock) only resolves on a running
        # system, where /run is mounted.
        if ! live && [ -L "$ROOT$d" ]; then
            case $(readlink "$ROOT$d") in
            /run/* | ../run/* | ../../run/*) continue ;;
            esac
        fi
        d_missing="$d_missing $d"
    done
    if [ -z "$d_missing" ]; then
        report PASS "$d_label ($#/$#)"
    else
        report "$d_level" "$d_label, missing:$d_missing"
    fi
}

# perms PATH: the `ls -ld` mode string, such as drwxrwxrwt. (POSIX has no
# portable stat(1), and only the fixed-width mode column is read.)
# shellcheck disable=SC2012
perms() { ls -ld "$ROOT$(resolve "$1")" 2>/dev/null | cut -c1-10; }

# read_file PATH: prints a file of the checked root (empty if unreadable).
read_file() { cat "$ROOT$(resolve "$1")" 2>/dev/null; }

# A key=value file (os-release(5) syntax): kv_get FILE KEY prints the value
# of KEY with one level of quotes removed.
kv_get() {
    read_file "$1" | sed -n "s/^$2=//p" | tail -n 1 |
        sed -e 's/^"\(.*\)"$/\1/' -e "s/^'\(.*\)'\$/\1/"
}

# matches STRING ERE: whether STRING matches the extended regular
# expression ERE entirely.
matches() { printf '%s\n' "$1" | grep -Eqx -- "$2"; }

if live; then
    TARGET="this system ($(uname -sr))"
else
    TARGET="$ROOT"
fi
printf '%sChecking %s%s\n' "$C_HEAD" "$TARGET" "$C_OFF"

# --- os-release(5) ----------------------------------------------------------

section "os-release" \
    "freedesktop.org os-release(5)"

OS_RELEASE=""
for f in /etc/os-release /usr/lib/os-release; do
    if exists -f "$f"; then
        OS_RELEASE=$f
        break
    fi
done

if [ -z "$OS_RELEASE" ]; then
    report FAIL "No os-release (/etc/os-release or /usr/lib/os-release)"
else
    report PASS "os-release found ($OS_RELEASE)"
    if exists -f /usr/lib/os-release; then
        report PASS "/usr/lib/os-release exists (the vendor copy)"
    else
        report WARN "/usr/lib/os-release missing; os-release(5) puts the vendor's file there"
    fi
    if [ -L "$ROOT/etc/os-release" ]; then
        report PASS "/etc/os-release links to $(readlink "$ROOT/etc/os-release")"
    elif exists -f /etc/os-release && exists -f /usr/lib/os-release; then
        report WARN "/etc/os-release is a copy; os-release(5) recommends a relative symlink to ../usr/lib/os-release"
    fi

    # Syntax: blank lines, comments, or KEY=VALUE with a shell-style value.
    bad=$(read_file "$OS_RELEASE" | grep -Ev '^[[:space:]]*(#.*)?$' |
        grep -Ev '^[A-Z][A-Z0-9_]*=("[^"]*"|'"'[^']*'"'|[^[:space:]"'"'"'\\$`]*)$')
    if [ -z "$bad" ]; then
        report PASS "Every line is a comment or KEY=VALUE assignment"
    else
        report FAIL "Lines that are not valid assignments: $(printf '%s' "$bad" | tr '\n' '|')"
    fi

    known="NAME ID ID_LIKE PRETTY_NAME CPE_NAME VARIANT VARIANT_ID VERSION
VERSION_ID VERSION_CODENAME BUILD_ID IMAGE_ID IMAGE_VERSION RELEASE_TYPE
HOME_URL DOCUMENTATION_URL SUPPORT_URL BUG_REPORT_URL PRIVACY_POLICY_URL
SUPPORT_END LOGO ANSI_COLOR VENDOR_NAME VENDOR_URL EXPERIMENT EXPERIMENT_URL
DEFAULT_HOSTNAME ARCHITECTURE SYSEXT_LEVEL CONFEXT_LEVEL SYSEXT_SCOPE
CONFEXT_SCOPE PORTABLE_PREFIXES FANCY_NAME"
    unknown=""
    for key in $(read_file "$OS_RELEASE" | sed -n 's/^\([A-Z][A-Z0-9_]*\)=.*/\1/p'); do
        case " $(printf '%s' "$known" | tr '\n' ' ') " in
        *" $key "*) ;;
        *) unknown="$unknown $key" ;;
        esac
    done
    [ -z "$unknown" ] ||
        report INFO "Keys os-release(5) does not define (allowed, for vendor use):$unknown"

    for key in NAME PRETTY_NAME; do
        if [ -n "$(kv_get "$OS_RELEASE" "$key")" ]; then
            report PASS "$key=\"$(kv_get "$OS_RELEASE" "$key")\""
        else
            report WARN "$key is not set (programs fall back to \"Linux\")"
        fi
    done

    ID=$(kv_get "$OS_RELEASE" ID)
    if [ -z "$ID" ]; then
        report WARN "ID is not set (programs fall back to \"linux\")"
    elif matches "$ID" '[a-z0-9._-]+'; then
        report PASS "ID=$ID uses only a-z, 0-9, \".\", \"_\" and \"-\""
    else
        report FAIL "ID=$ID has characters os-release(5) does not allow"
    fi
    for key in VERSION_ID VERSION_CODENAME VARIANT_ID IMAGE_ID; do
        value=$(kv_get "$OS_RELEASE" "$key")
        [ -n "$value" ] || continue
        if matches "$value" '[a-z0-9._-]+'; then
            report PASS "$key=$value is valid"
        else
            report FAIL "$key=$value has characters os-release(5) does not allow"
        fi
    done
    ID_LIKE=$(kv_get "$OS_RELEASE" ID_LIKE)
    if [ -n "$ID_LIKE" ]; then
        if matches "$ID_LIKE" '[a-z0-9._-]+( [a-z0-9._-]+)*'; then
            report PASS "ID_LIKE=\"$ID_LIKE\" is a valid list of IDs"
        else
            report FAIL "ID_LIKE=\"$ID_LIKE\" is not a space-separated list of IDs"
        fi
    fi
    if [ -n "$(kv_get "$OS_RELEASE" VERSION_ID)$(kv_get "$OS_RELEASE" BUILD_ID)" ]; then
        report PASS "VERSION_ID or BUILD_ID identifies the release"
    else
        report WARN "Neither VERSION_ID nor BUILD_ID is set"
    fi
    BUILD_ID=$(kv_get "$OS_RELEASE" BUILD_ID)
    if [ -n "$BUILD_ID" ] && ! matches "$BUILD_ID" '[^[:space:]]+'; then
        report FAIL "BUILD_ID=\"$BUILD_ID\" contains spaces"
    fi

    for key in HOME_URL DOCUMENTATION_URL SUPPORT_URL BUG_REPORT_URL \
        PRIVACY_POLICY_URL VENDOR_URL EXPERIMENT_URL; do
        value=$(kv_get "$OS_RELEASE" "$key")
        [ -n "$value" ] || continue
        if matches "$value" '(https?|mailto|tel):[^[:space:]]+'; then
            report PASS "$key is a valid URL"
        else
            report FAIL "$key=\"$value\" is not a single http(s), mailto or tel URL"
        fi
    done
    [ -n "$(kv_get "$OS_RELEASE" HOME_URL)" ] ||
        report WARN "HOME_URL is not set"
    [ -n "$(kv_get "$OS_RELEASE" BUG_REPORT_URL)" ] ||
        report INFO "BUG_REPORT_URL is not set"

    CPE=$(kv_get "$OS_RELEASE" CPE_NAME)
    if [ -n "$CPE" ]; then
        if matches "$CPE" 'cpe:(/|2\.3:)[^[:space:]]+'; then
            report PASS "CPE_NAME is a CPE name"
        else
            report FAIL "CPE_NAME=\"$CPE\" is not a CPE name (cpe:/ or cpe:2.3:)"
        fi
    fi
    END=$(kv_get "$OS_RELEASE" SUPPORT_END)
    if [ -n "$END" ]; then
        if matches "$END" '[0-9]{4}-[0-9]{2}-[0-9]{2}'; then
            report PASS "SUPPORT_END=$END is a date"
        else
            report FAIL "SUPPORT_END=\"$END\" is not YYYY-MM-DD"
        fi
    fi
    COLOR_SGR=$(kv_get "$OS_RELEASE" ANSI_COLOR)
    if [ -n "$COLOR_SGR" ]; then
        if matches "$COLOR_SGR" '[0-9]+(;[0-9]+)*'; then
            report PASS "ANSI_COLOR=\"$COLOR_SGR\" is an SGR sequence"
        else
            report FAIL "ANSI_COLOR=\"$COLOR_SGR\" is not an SGR sequence (digits and ;)"
        fi
    fi
    HOSTNAME_DEFAULT=$(kv_get "$OS_RELEASE" DEFAULT_HOSTNAME)
    if [ -n "$HOSTNAME_DEFAULT" ] &&
        ! matches "$HOSTNAME_DEFAULT" '[A-Za-z0-9]([A-Za-z0-9-]*[A-Za-z0-9])?(\.[A-Za-z0-9]([A-Za-z0-9-]*[A-Za-z0-9])?)*'; then
        report FAIL "DEFAULT_HOSTNAME=\"$HOSTNAME_DEFAULT\" is not a valid host name"
    fi

    LOGO=$(kv_get "$OS_RELEASE" LOGO)
    if [ -n "$LOGO" ]; then
        found=""
        for f in "$ROOT"/usr/share/icons/*/*/apps/"$LOGO".* \
            "$ROOT"/usr/share/icons/*/*/*/apps/"$LOGO".* \
            "$ROOT"/usr/share/pixmaps/"$LOGO".*; do
            [ -e "$f" ] && found=${f#"$ROOT"} && break
        done
        if [ -n "$found" ]; then
            report PASS "LOGO=$LOGO resolves to $found (Icon Theme lookup)"
        else
            report FAIL "LOGO=$LOGO names no icon in /usr/share/icons or /usr/share/pixmaps"
        fi
    else
        report INFO "LOGO is not set"
    fi
fi

# --- machine-id(5) and system identity files --------------------------------

section "System identity files" \
    "machine-id(5), hostname(5), machine-info(5), locale.conf(5), vconsole.conf(5), localtime(5)"

if exists -f /etc/machine-id; then
    MID=$(read_file /etc/machine-id)
    if [ -z "$MID" ]; then
        if live; then
            report FAIL "/etc/machine-id is empty on a running system"
        else
            report INFO "/etc/machine-id is empty: systemd sets it at first boot (right for images)"
        fi
    elif [ "$MID" = uninitialized ]; then
        if live; then
            report WARN "/etc/machine-id still says \"uninitialized\" (first boot did not finish)"
        else
            report INFO "/etc/machine-id is \"uninitialized\": systemd runs first-boot setup"
        fi
    elif [ "$MID" = 00000000000000000000000000000000 ]; then
        report FAIL "/etc/machine-id is all zeros, which machine-id(5) forbids"
    elif matches "$MID" '[0-9a-f]{32}'; then
        report PASS "/etc/machine-id is 32 lowercase hexadecimal characters"
        live || report INFO "The root carries a fixed machine ID; every copy of an image built from it shares it"
    else
        report FAIL "/etc/machine-id is not 32 lowercase hexadecimal characters"
    fi
else
    if live; then
        report FAIL "/etc/machine-id missing"
    else
        report INFO "/etc/machine-id missing: systemd creates it at first boot"
    fi
fi

if exists -f /etc/hostname; then
    HN=$(read_file /etc/hostname | grep -v '^#' | head -n 1)
    if [ -z "$HN" ]; then
        report WARN "/etc/hostname is empty"
    elif [ "${#HN}" -gt 64 ]; then
        report FAIL "/etc/hostname is longer than 64 characters"
    elif matches "$HN" '[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)*'; then
        report PASS "/etc/hostname \"$HN\" is a valid host name"
    elif matches "$HN" '[A-Za-z0-9]([A-Za-z0-9-]*[A-Za-z0-9])?(\.[A-Za-z0-9]([A-Za-z0-9-]*[A-Za-z0-9])?)*'; then
        report WARN "/etc/hostname \"$HN\" has capitals; hostname(5) recommends lowercase"
    else
        report FAIL "/etc/hostname \"$HN\" is not a valid host name (hostname(5))"
    fi
else
    report WARN "/etc/hostname missing"
fi

if exists -f /etc/machine-info; then
    bad=$(read_file /etc/machine-info | grep -Ev '^[[:space:]]*(#.*)?$' |
        sed -n 's/^\([A-Z_]*\)=.*/\1/p' |
        grep -Evx 'PRETTY_HOSTNAME|ICON_NAME|CHASSIS|DEPLOYMENT|LOCATION|HARDWARE_VENDOR|HARDWARE_MODEL|HARDWARE_SKU|HARDWARE_VERSION')
    if [ -z "$bad" ]; then
        report PASS "/etc/machine-info uses only machine-info(5) keys"
    else
        report WARN "/etc/machine-info has unknown keys: $(printf '%s' "$bad" | tr '\n' ' ')"
    fi
else
    report INFO "/etc/machine-info missing (optional)"
fi

locale_available() {
    l_name=$1
    l_norm=$(printf '%s' "$l_name" | sed 's/\.UTF-8$/.utf8/; s/\.utf-8$/.utf8/')
    case $l_name in C | POSIX) return 0 ;; esac
    exists -d "/usr/lib/locale/$l_name" && return 0
    exists -d "/usr/lib/locale/$l_norm" && return 0
    if live && has_cmd locale; then
        locale -a 2>/dev/null | grep -qix -e "$l_name" -e "$l_norm"
        return
    fi
    # Compiled into the archive: listed as generated in locale.gen.
    read_file /etc/locale.gen | grep -Eq "^[[:space:]]*${l_name}[[:space:]]" &&
        exists -f /usr/lib/locale/locale-archive
}

if exists -f /etc/locale.conf; then
    bad=$(read_file /etc/locale.conf | grep -Ev '^[[:space:]]*(#.*)?$' |
        sed -n 's/^\([A-Z_]*\)=.*/\1/p' |
        grep -Evx 'LANG|LANGUAGE|LC_[A-Z_]+')
    if [ -z "$bad" ]; then
        report PASS "/etc/locale.conf uses only locale.conf(5) keys"
    else
        report FAIL "/etc/locale.conf has unknown keys: $(printf '%s' "$bad" | tr '\n' ' ')"
    fi
    LANG_VALUE=$(kv_get /etc/locale.conf LANG)
    if [ -z "$LANG_VALUE" ]; then
        report WARN "/etc/locale.conf does not set LANG"
    elif locale_available "$LANG_VALUE"; then
        report PASS "LANG=$LANG_VALUE is an available locale"
    else
        report FAIL "LANG=$LANG_VALUE is not an available (generated) locale"
    fi
else
    report WARN "/etc/locale.conf missing (the system locale is C)"
fi

if exists -f /etc/vconsole.conf; then
    bad=$(read_file /etc/vconsole.conf | grep -Ev '^[[:space:]]*(#.*)?$' |
        sed -n 's/^\([A-Z_]*\)=.*/\1/p' |
        grep -Evx 'KEYMAP|KEYMAP_TOGGLE|FONT|FONT_MAP|FONT_UNIMAP|XKBLAYOUT|XKBMODEL|XKBVARIANT|XKBOPTIONS')
    if [ -z "$bad" ]; then
        report PASS "/etc/vconsole.conf uses only vconsole.conf(5) keys"
    else
        report FAIL "/etc/vconsole.conf has unknown keys: $(printf '%s' "$bad" | tr '\n' ' ')"
    fi
    KEYMAP=$(kv_get /etc/vconsole.conf KEYMAP)
    if [ -n "$KEYMAP" ]; then
        if [ -n "$(find "$ROOT/usr/share/kbd/keymaps" -name "$KEYMAP.map*" 2>/dev/null | head -n 1)" ]; then
            report PASS "KEYMAP=$KEYMAP exists in /usr/share/kbd/keymaps"
        else
            report FAIL "KEYMAP=$KEYMAP is not in /usr/share/kbd/keymaps"
        fi
    fi
else
    report INFO "/etc/vconsole.conf missing (the kernel's default keymap)"
fi

if [ -L "$ROOT/etc/localtime" ]; then
    LT=$(readlink "$ROOT/etc/localtime")
    case $LT in
    */usr/share/zoneinfo/*)
        if exists -f /etc/localtime; then
            report PASS "/etc/localtime links to $LT"
        else
            report FAIL "/etc/localtime links to $LT, which does not exist"
        fi
        ;;
    *) report WARN "/etc/localtime links outside /usr/share/zoneinfo ($LT); localtime(5) expects a link into it" ;;
    esac
elif exists -f /etc/localtime; then
    report WARN "/etc/localtime is a copy; localtime(5) expects a symlink so the zone name can be read"
else
    report WARN "/etc/localtime missing (the time zone is UTC)"
fi
check_path WARN -f /usr/share/zoneinfo/UTC "Time zone database"

# --- FHS 3.0 ----------------------------------------------------------------

section "Filesystem layout" \
    "Filesystem Hierarchy Standard 3.0, chapters 3-6 (Linux annex)"

check_dirs FAIL "Required in / (3.2, 3.15)" \
    /bin /boot /dev /etc /lib /media /mnt /opt /run /sbin /srv /tmp /usr /var
check_dirs WARN "Optional in / (3.2)" /home /root
check_dirs FAIL "Linux pseudo-filesystems (6.1)" /proc /sys
case $(uname -m) in
x86_64 | aarch64 | ppc64* | s390x | riscv64) check_dirs WARN "64-bit library directory (6.1.2)" /lib64 ;;
esac
check_dirs FAIL "Required in /etc (3.7.2)" /etc/opt
check_dirs INFO "Optional in /etc (3.7.3)" /etc/X11 /etc/sgml /etc/xml
check_dirs FAIL "Required in /usr (4.2)" \
    /usr/bin /usr/include /usr/lib /usr/local /usr/sbin /usr/share
check_dirs INFO "Optional in /usr (4.3)" /usr/games /usr/libexec /usr/src
check_dirs WARN "Required in /usr/local (4.9.2)" \
    /usr/local/bin /usr/local/etc /usr/local/games /usr/local/include \
    /usr/local/lib /usr/local/man /usr/local/sbin /usr/local/share \
    /usr/local/src
check_dirs FAIL "Required in /usr/share (4.11.2)" /usr/share/man /usr/share/misc
check_dirs INFO "Optional in /usr/share (4.11.3)" \
    /usr/share/dict /usr/share/doc /usr/share/info /usr/share/locale \
    /usr/share/nls /usr/share/terminfo /usr/share/zoneinfo
check_dirs FAIL "Required in /var (5.2)" \
    /var/cache /var/lib /var/local /var/lock /var/log /var/opt /var/run \
    /var/spool /var/tmp
check_dirs WARN "Required in /var/lib (5.8.2)" /var/lib/misc
check_dirs INFO "Optional in /var (5.3)" /var/games /var/mail /var/crash

for d in /tmp /var/tmp; do
    case $(perms "$d") in
    d????????t | d????????T) report PASS "$d is world-writable and sticky ($(perms "$d"))" ;;
    "") ;;
    *) report FAIL "$d is not world-writable and sticky ($(perms "$d"); expected drwxrwxrwt)" ;;
    esac
done
case $(perms /root) in
d???------ | d???r-x--- | d???r-----) report PASS "/root is private ($(perms /root))" ;;
"") ;;
*) report WARN "/root is readable by others ($(perms /root))" ;;
esac
for link in /var/run:/run /var/lock:/run/lock; do
    from=${link%%:*} to=${link#*:}
    if [ -L "$ROOT$from" ]; then
        report PASS "$from links to $(readlink "$ROOT$from")"
    else
        report WARN "$from is not a link to $to (FHS 3.0 5.9, 5.13)"
    fi
done
if [ -L "$ROOT/etc/mtab" ]; then
    report PASS "/etc/mtab links to $(readlink "$ROOT/etc/mtab")"
else
    report WARN "/etc/mtab is not a link to /proc/self/mounts"
fi

merged=""
for d in /bin /sbin /lib; do
    [ -L "$ROOT$d" ] && merged="$merged $d"
done
if [ "$merged" = " /bin /sbin /lib" ]; then
    report INFO "Merged /usr (/bin, /sbin and /lib link into /usr)"
elif [ -n "$merged" ]; then
    report INFO "Partly merged /usr (links:$merged)"
else
    report INFO "Split /usr (/bin, /sbin and /lib are real directories)"
fi

case $(uname -m) in
x86_64) LOADERS="/lib64/ld-linux-x86-64.so.2 /lib/ld-musl-x86_64.so.1" ;;
aarch64) LOADERS="/lib/ld-linux-aarch64.so.1 /lib/ld-musl-aarch64.so.1" ;;
*) LOADERS="" ;;
esac
if [ -n "$LOADERS" ]; then
    found=""
    for l in $LOADERS; do
        if exists -e "$l"; then
            found=$l
            break
        fi
    done
    if [ -n "$found" ]; then
        report PASS "Dynamic loader at its ABI path ($found)"
    else
        report FAIL "Dynamic loader missing (one of: $LOADERS)"
    fi
fi

# --- systemd file-hierarchy(7) ----------------------------------------------

INIT_TARGET=$(resolve /sbin/init)
case $INIT_TARGET in
*systemd*) HAS_SYSTEMD=1 ;;
*) HAS_SYSTEMD=0 ;;
esac

section "systemd hierarchy" "systemd file-hierarchy(7)"
if [ "$HAS_SYSTEMD" = 1 ]; then
    check_dirs WARN "Vendor configuration in /usr/lib" \
        /usr/lib/systemd/system /usr/lib/tmpfiles.d /usr/lib/sysusers.d \
        /usr/lib/sysctl.d /usr/lib/modules-load.d
    check_dirs WARN "Local configuration in /etc" \
        /etc/systemd/system /etc/tmpfiles.d /etc/sysctl.d
    check_dirs INFO "Presets" /usr/lib/systemd/system-preset
    check_dirs WARN "systemd state" /var/lib/systemd
else
    report INFO "Not checked: the init system is not systemd ($INIT_TARGET)"
fi

# --- freedesktop.org desktop specifications ---------------------------------

section "freedesktop.org desktop" \
    "XDG Base Directory 0.8, Icon Theme 0.13, Shared MIME-info 0.21, Desktop Entry 1.5, D-Bus"

check_path WARN -d /etc/xdg "XDG system configuration directory (XDG_CONFIG_DIRS default)"
check_path FAIL -d /usr/share "XDG system data directory (XDG_DATA_DIRS default)"
check_path INFO -d /usr/local/share "XDG local data directory (XDG_DATA_DIRS default)"
if live; then
    for var in XDG_CONFIG_HOME XDG_DATA_HOME XDG_STATE_HOME XDG_CACHE_HOME XDG_RUNTIME_DIR; do
        eval "value=\${$var:-}"
        [ -n "$value" ] || continue
        case $value in
        /*) report PASS "$var is absolute ($value)" ;;
        *) report FAIL "$var=$value is relative; the specification says to ignore it" ;;
        esac
    done
    if [ -n "${XDG_RUNTIME_DIR:-}" ]; then
        case $(perms "$XDG_RUNTIME_DIR") in
        drwx------) report PASS "XDG_RUNTIME_DIR is private (drwx------)" ;;
        *) report FAIL "XDG_RUNTIME_DIR is not mode 0700 ($(perms "$XDG_RUNTIME_DIR"))" ;;
        esac
    else
        report INFO "XDG_RUNTIME_DIR is not set (no login session, or no systemd-logind)"
    fi
fi
check_path INFO -f /usr/share/icons/hicolor/index.theme "hicolor icon theme, the Icon Theme fallback (desktops need it)"
check_path INFO -f /usr/share/mime/mime.cache "Shared MIME-info database (desktops need it)"
check_path INFO -d /usr/share/applications "Desktop Entry directory"
if [ "$HAS_SYSTEMD" = 1 ] || exists -d /usr/share/dbus-1; then
    check_path WARN -d /usr/share/dbus-1/system.d "D-Bus system bus policy directory"
    if live; then
        if [ -S /run/dbus/system_bus_socket ]; then
            report PASS "D-Bus system bus socket (/run/dbus/system_bus_socket)"
        else
            report WARN "No D-Bus system bus socket at /run/dbus/system_bus_socket"
        fi
    fi
fi

# --- Accounts and authentication --------------------------------------------

section "Accounts and authentication" "common practice; passwd(5), shadow(5), group(5)"

if read_file /etc/passwd | grep -q '^root:[^:]*:0:'; then
    report PASS "root account with UID 0 (/etc/passwd)"
else
    report FAIL "No root account with UID 0 in /etc/passwd"
fi
bad=$(read_file /etc/passwd | awk -F: 'NF != 7 && !/^#/ && NF' | head -n 3)
if [ -z "$bad" ]; then
    report PASS "Every /etc/passwd entry has 7 fields"
else
    report FAIL "/etc/passwd entries without 7 fields: $bad"
fi
dups=$(read_file /etc/passwd | cut -d: -f1 | sort | uniq -d | tr '\n' ' ')
[ -z "$dups" ] || report FAIL "Duplicate user names in /etc/passwd: $dups"
check_path FAIL -f /etc/group "Group database"
if exists -f /etc/shadow; then
    case $(perms /etc/shadow | cut -c8) in
    r) report FAIL "/etc/shadow is world-readable ($(perms /etc/shadow))" ;;
    *) report PASS "/etc/shadow is private ($(perms /etc/shadow))" ;;
    esac
else
    report WARN "/etc/shadow missing (passwords would be in /etc/passwd)"
fi
check_path INFO -f /etc/gshadow "Group password database"
if read_file /etc/passwd | grep -q '^nobody:'; then
    report PASS "nobody user"
else
    report WARN "No nobody user (used by NFS, daemons and sandboxes)"
fi

if exists -f /etc/shells; then
    bad=""
    for s in $(read_file /etc/shells | grep '^/'); do
        exists -x "$s" || bad="$bad $s"
    done
    if [ -z "$bad" ]; then
        report PASS "Every shell in /etc/shells exists"
    else
        report WARN "/etc/shells lists missing shells:$bad"
    fi
else
    report WARN "/etc/shells missing (chsh and some SSH setups use it)"
fi

check_path WARN -f /etc/login.defs "Login defaults"
check_path WARN -f /etc/nsswitch.conf "Name service switch config"
if exists -d /etc/pam.d; then
    report PASS "PAM configuration (/etc/pam.d)"
    for p in login su passwd; do
        exists -f "/etc/pam.d/$p" || report WARN "/etc/pam.d/$p missing"
    done
else
    report WARN "No PAM configuration (/etc/pam.d)"
fi
check_any WARN "Privilege tool" sudo doas run0
if has_cmd sudo; then
    check_path WARN -f /etc/sudoers "sudo configuration"
fi
for f in /etc/subuid /etc/subgid; do
    exists -f "$f" ||
        report INFO "$f missing (rootless containers need it)"
done

# --- System configuration ---------------------------------------------------

section "System configuration" "common practice; hosts(5), resolv.conf(5), services(5), protocols(5), fstab(5)"

if read_file /etc/hosts | grep -q 'localhost'; then
    report PASS "/etc/hosts names localhost"
else
    report WARN "/etc/hosts missing or lacks localhost"
fi
if exists -e /etc/resolv.conf; then
    report PASS "DNS resolver config (/etc/resolv.conf)"
elif [ -L "$ROOT/etc/resolv.conf" ]; then
    report INFO "/etc/resolv.conf links to $(readlink "$ROOT/etc/resolv.conf"), created at runtime"
else
    report WARN "/etc/resolv.conf missing"
fi
check_path WARN -f /etc/services "Service names (getservbyname)"
check_path WARN -f /etc/protocols "Protocol names (getprotobyname)"
check_path WARN -f /etc/fstab "Filesystem table"
check_path WARN -f /etc/profile "Login shell profile"
check_path INFO -d /etc/profile.d "Login shell profile snippets"

CA=""
for f in /etc/ssl/certs/ca-certificates.crt /etc/ssl/cert.pem \
    /etc/pki/tls/certs/ca-bundle.crt; do
    if exists -s "$f"; then
        CA=$f
        break
    fi
done
if [ -n "$CA" ]; then
    report PASS "CA certificate bundle ($CA)"
else
    report FAIL "No CA certificate bundle (HTTPS cannot verify servers)"
fi

# --- Boot and kernel --------------------------------------------------------

section "Boot and kernel" "common practice"

if exists -x /sbin/init || exists -x /usr/sbin/init; then
    if [ -L "$ROOT/usr/sbin/init" ]; then
        report PASS "init (/usr/sbin/init -> $(readlink "$ROOT/usr/sbin/init"))"
    else
        report PASS "init (/sbin/init)"
    fi
else
    report FAIL "No init (/sbin/init)"
fi

kernels=0
for k in "$ROOT"/boot/vmlinuz* "$ROOT"/usr/lib/modules/*/vmlinuz; do
    [ -e "$k" ] && kernels=$((kernels + 1))
done
if [ "$kernels" -gt 0 ]; then
    report PASS "Kernel images ($kernels)"
else
    report INFO "No kernel image in /boot or /usr/lib/modules (fine for containers)"
fi
module_versions=""
for m in "$ROOT"/usr/lib/modules/*/kernel; do
    [ -d "$m" ] || continue
    m=${m%/kernel}
    module_versions="$module_versions ${m##*/}"
done
if [ -n "$module_versions" ]; then
    report PASS "Kernel modules for:$module_versions"
else
    report INFO "No kernel modules in /usr/lib/modules (fine for containers)"
fi
initramfs=""
for i in "$ROOT"/boot/initramfs* "$ROOT"/boot/initrd*; do
    [ -e "$i" ] && initramfs="$initramfs ${i##*/}"
done
if [ -n "$initramfs" ]; then
    report PASS "initramfs in /boot:$initramfs"
else
    report INFO "No initramfs in /boot"
fi
check_any INFO "Bootloader tools" grub-install bootctl limine refind-install syslinux

# --- POSIX.1-2024 utilities -------------------------------------------------

section "POSIX utilities" \
    "POSIX.1-2024 (Issue 8), Shell and Utilities volume, chapter 3"

# The Issue 8 utilities by margin code: no code (every system), UP (User
# Portability), SD/CD (software and C-language development), XSI (X/Open
# System Interfaces) and UU (UUCP).
check_cmds FAIL "Base utilities everything relies on" \
    sh awk basename cat chgrp chmod chown cksum cmp cp cut date dd df diff \
    dirname du echo env expr false find grep head id kill ln ls mkdir mv \
    od printf ps pwd rm rmdir sed sleep sort tail tee test touch tr true \
    uname uniq wc xargs
check_cmds WARN "Base utilities" \
    at batch bc cal comm crontab csplit ed expand file fold gencat getconf \
    gettext iconv join locale localedef logger logname lp m4 mailx man \
    mesg mkfifo msgfmt newgrp ngettext nice nohup paste patch pathchk pax \
    pr readlink realpath renice split stty tabs time timeout tput tsort \
    tty unexpand uudecode uuencode who write xgettext
check_cmds WARN "User Portability (UP)" ex more talk vi
check_cmds INFO "Software development (SD, CD)" ar c17 ctags lex make nm strings strip yacc
check_cmds INFO "X/Open System Interfaces (XSI)" \
    admin cflow compress cxref delta fuser get ipcrm ipcs link nl prs rmdel \
    sact sccs uncompress unget unlink val what zcat
check_cmds INFO "UUCP (UU)" uucp uustat uux
if live; then
    # Built-in utilities the shell must provide (Issue 8, 1.7 and the
    # "intrinsic utilities"); only a running shell can be asked.
    missing=""
    for b in alias bg cd command fc fg getopts hash jobs kill read type \
        ulimit umask unalias wait; do
        sh -c "command -v $b" >/dev/null 2>&1 || missing="$missing $b"
    done
    if [ -z "$missing" ]; then
        report PASS "/bin/sh provides every built-in utility"
    else
        report FAIL "/bin/sh lacks built-in utilities:$missing"
    fi
fi

# --- POSIX behaviour --------------------------------------------------------

section "POSIX behaviour" \
    "POSIX.1-2024 requirements of the shell and utilities, checked by running them"

# behaves DESCRIPTION EXPECTED SCRIPT [UTILITY...]: runs SCRIPT with /bin/sh
# in the C locale and compares its output with EXPECTED. Skipped when one of
# the UTILITY commands is missing (the utilities section reports that).
behaves() {
    b_desc=$1 b_expected=$2 b_script=$3
    shift 3
    for b_cmd in "$@"; do
        if ! has_cmd "$b_cmd"; then
            report INFO "$b_desc: skipped, $b_cmd is missing"
            return
        fi
    done
    set -- "$b_desc" "$b_expected" "$b_script"
    b_out=$(LC_ALL=C sh -c "$3" 2>/dev/null)
    if [ "$b_out" = "$2" ]; then
        report PASS "$1"
    else
        report FAIL "$1 (expected \"$2\", got \"$b_out\")"
    fi
}

# The scripts are single-quoted on purpose: /bin/sh expands them.
# shellcheck disable=SC2016
if ! live; then
    report INFO "Skipped: --root runs nothing from the root"
else
    T=$(mktemp -d "${TMPDIR:-/tmp}/distro-check.XXXXXX")
    behaves "sh: arithmetic expansion" 7 'echo $((1 + 2 * 3))'
    behaves "sh: command substitution and parameter operators" "b.c a" \
        'f=a/b.c; echo "${f##*/} ${f%%/*}"'
    behaves "sh: case with patterns" yes 'case abc in a*c) echo yes;; esac'
    behaves "sh: functions, local return status" 3 'f() { return 3; }; f; echo $?'
    behaves "sh: getopts" "a:x b" 'set -- -a x -b; while getopts a:b o; do case $o in a) printf "a:%s " "$OPTARG";; b) echo b;; esac; done'
    behaves "sh: read -r keeps backslashes" 'a\b' 'printf "a\\\\b\n" | { read -r v; printf "%s" "$v"; }'
    behaves "sh: here-document" "x y" 'cat <<E
x y
E'
    behaves "sh: set -u fails on an unset variable" fail 'set -u; (echo "$undefined_variable") 2>/dev/null || echo fail'
    behaves "sh: trap on EXIT" bye 'trap "echo bye" EXIT; true'
    behaves "printf: formats and escapes" "0042|ff|a	b" 'printf "%04d|%x|%b" 42 255 "a\\tb"'
    behaves "test: string, integer and file operators" ok 'test -n a && test 2 -gt 1 && test -d / && echo ok'
    behaves "expr: arithmetic and matching" "5 3" 'echo "$(expr 2 + 3) $(expr abc : "a.*")"'
    behaves "basename and dirname" "c b /a" 'echo "$(basename /a/b/c.h .h) $(basename /a/b) $(dirname /a/b)"'
    behaves "cut: fields and characters" "b cd" 'echo "$(echo a:b:c | cut -d: -f2) $(echo abcd | cut -c3-)"'
    behaves "tr: delete and squeeze" "ac b" 'echo "$(echo abc | tr -d b) $(echo bbb | tr -s b)"'
    behaves "sed: substitution and line selection" "X2" 'printf "1\n2\n" | sed -n "2s/^/X/p"'
    behaves "awk: fields and printf" "3.50" 'echo "1 2.5" | awk "{ printf \"%.2f\", \$1 + \$2 }"'
    behaves "sort: numeric keys" "2 b|10 a" 'printf "10 a\n2 b\n" | sort -n -k1,1 | paste -sd"|" -'
    behaves "uniq -c and wc -l" "2" 'printf "a\na\nb\n" | uniq | wc -l | tr -d " "'
    behaves "grep -E counts extended matches" 2 'printf "a+\nab\naab\n" | grep -Ec "^a+b$"'
    behaves "grep -F -x matches fixed whole lines" "a+" 'printf "a+\na+b\n" | grep -Fx "a+"'
    behaves "head and tail" "2 9" 'echo "$(printf "1\n2\n3\n" | head -n 2 | tail -n 1) $(printf "8\n9\n" | tail -n 1)"'
    behaves "od -A n -t x1" "61 62" 'printf ab | od -A n -t x1 | tr -s " " | sed "s/^ //; s/ $//"'
    behaves "cksum of empty input" "4294967295 0" 'printf "" | cksum' cksum
    behaves "paste and join" "a 1 x" 'printf "a 1\n" > "'"$T"'/1"; printf "a x\n" > "'"$T"'/2"; join "'"$T"'/1" "'"$T"'/2"' join
    behaves "comm" "b" 'printf "a\nb\n" > "'"$T"'/c1"; printf "a\n" > "'"$T"'/c2"; comm -23 "'"$T"'/c1" "'"$T"'/c2"' comm
    behaves "find -name and -type" "x" 'mkdir -p "'"$T"'/f/x"; find "'"$T"'/f" -type d -name x -exec basename {} \;'
    behaves "xargs" "1 2 3" 'printf "1\n2\n3\n" | xargs echo'
    behaves "date -u with a format" "1970" 'TZ=UTC date -u -d @0 +%Y 2>/dev/null || TZ=UTC date -u -r 0 +%Y 2>/dev/null'
    behaves "ln -s, readlink and realpath" "target $T/target" 'cd "'"$T"'" && touch target && ln -s target link && echo "$(readlink link) $(realpath link)"' readlink realpath
    behaves "mkdir -p -m" "drwx------" 'mkdir -p -m 700 "'"$T"'/m/n" && ls -ld "'"$T"'/m/n" | cut -c1-10'
    behaves "umask -S" "u=rwx,g=rx,o=rx" 'umask 022; umask -S'
    behaves "diff -u marks changed lines" "-a +b" 'echo a > "'"$T"'/d1"; echo b > "'"$T"'/d2"; diff -u "'"$T"'/d1" "'"$T"'/d2" | grep -E "^[-+][ab]" | paste -sd" " -' diff
    behaves "ed edits a file from a script" "hello" 'printf "x\n" > "'"$T"'/e"; printf "1c\nhello\n.\nw\nq\n" | ed -s "'"$T"'/e" && cat "'"$T"'/e"' ed
    behaves "bc arbitrary precision" "1267650600228229401496703205376" 'echo "2^100" | bc' bc
    behaves "getconf PATH" ok 'test -n "$(getconf PATH)" && echo ok' getconf
    behaves "pathchk -p rejects non-portable names" no 'pathchk -p "a*b" 2>/dev/null && echo yes || echo no' pathchk
    behaves "locale lists the POSIX locale" ok 'locale -a | grep -qx -e C -e POSIX && echo ok' locale
    behaves "iconv converts between encodings" "c3a9" 'printf "\351" | iconv -f ISO-8859-1 -t UTF-8 | od -A n -t x1 | tr -d " \n"' iconv od
    rm -rf "$T"
fi

# --- Common Linux utilities -------------------------------------------------

section "Common Linux utilities" "common practice on Arch, Debian and Fedora"

check_cmds WARN "coreutils beyond POSIX" \
    install mktemp stat sha256sum sha512sum md5sum base64 nproc seq \
    truncate tac chroot mknod less which diff3 sdiff
check_cmds WARN "util-linux" \
    lsblk findmnt blkid fdisk sfdisk losetup mkswap swapon swapoff su \
    login agetty dmesg flock unshare nsenter hwclock wipefs column hexdump \
    lscpu setsid mount umount
check_cmds WARN "Processes (procps, psmisc)" \
    top free pgrep pkill pidof sysctl uptime watch vmstat killall
check_cmds WARN "Users and groups (shadow)" \
    useradd usermod userdel groupadd groupmod groupdel passwd chpasswd \
    chsh gpasswd
check_cmds WARN "Compression" gzip bzip2 xz zstd tar
check_cmds INFO "More compression" unzip zip 7z lz4
check_cmds WARN "Networking" ip ss ping hostname
check_any WARN "HTTP client" curl wget
check_cmds INFO "More networking" dig ssh rsync nc traceroute
check_any WARN "TLS toolkit" openssl gnutls-cli
check_any INFO "OpenPGP" gpg sq
check_cmds WARN "Kernel modules (kmod)" modprobe lsmod depmod insmod rmmod modinfo
check_cmds WARN "Filesystems" fsck mkfs mkfs.ext4 e2fsck
check_cmds INFO "More filesystems" mkfs.vfat mkfs.btrfs mkfs.xfs cryptsetup lvm
check_cmds WARN "C library tools" ldd ldconfig getent iconv
check_any WARN "Editor" vi vim nvim nano
check_any WARN "Manual page viewer" man mandoc
check_cmds INFO "Shells" bash zsh dash fish
check_cmds INFO "Developer toolchain" cc make pkg-config ld as gdb
if [ "$HAS_SYSTEMD" = 1 ]; then
    check_cmds WARN "systemd tools" \
        systemctl journalctl loginctl hostnamectl timedatectl localectl \
        busctl systemd-analyze udevadm systemd-sysusers systemd-tmpfiles
    check_cmds INFO "systemd network tools" resolvectl networkctl
fi
check_any INFO "Package manager" zoi pacman apt dnf zypper apk xbps-install emerge nix-env

# --- POSIX system interfaces ------------------------------------------------

section "POSIX system interfaces" \
    "POSIX.1-2024 Base Definitions (limits, options, headers) and System Interfaces"

if [ -n "$SYSCHECK" ]; then
    if [ ! -x "$SYSCHECK" ]; then
        report FAIL "--syscheck $SYSCHECK is not an executable file"
    else
        if live; then
            OUT=$("$SYSCHECK" 2>&1)
            STATUS=$?
        elif [ "$(id -u)" = 0 ]; then
            # Inside the root, so its own C library answers. Needs a static
            # build, since the root's loader may not match the host's.
            COPY="/tmp/distro-check-syscheck.$$"
            cp "$SYSCHECK" "$ROOT$COPY"
            OUT=$(chroot "$ROOT" "$COPY" 2>&1)
            STATUS=$?
            rm -f "$ROOT$COPY"
        else
            OUT="" STATUS=0
            report INFO "C library checks skipped: checking a --root needs root (chroot) and a static syscheck"
        fi
        if [ "$STATUS" != 0 ]; then
            report FAIL "syscheck failed (exit $STATUS): $(printf '%s' "$OUT" | head -n 3 | tr '\n' ' ')"
        elif [ -n "$OUT" ]; then
            TAB=$(printf '\t')
            printf '%s\n' "$OUT" | {
                while IFS=$TAB read -r level message; do
                    case $level in
                    PASS | FAIL | WARN | INFO) report "$level" "$message" ;;
                    esac
                done
                # The counts live in this subshell; hand them back.
                echo "$N_PASS $N_FAIL $N_WARN $N_INFO $S_PASS $S_FAIL $S_WARN" >"${TMPDIR:-/tmp}/distro-check-counts.$$"
            }
            read -r N_PASS N_FAIL N_WARN N_INFO S_PASS S_FAIL S_WARN <"${TMPDIR:-/tmp}/distro-check-counts.$$"
            rm -f "${TMPDIR:-/tmp}/distro-check-counts.$$"
        fi
    fi
elif live && has_cmd getconf; then
    # Without the C helper, getconf(1) answers the main questions.
    v=$(getconf _POSIX_VERSION 2>/dev/null || echo 0)
    if [ "${v:-0}" -ge 200809 ] 2>/dev/null; then
        report PASS "POSIX.1 version $v (getconf)"
    else
        report FAIL "POSIX.1 version ${v:-unknown}, older than POSIX.1-2008 (getconf)"
    fi
    v=$(getconf _XOPEN_VERSION 2>/dev/null || echo 0)
    if [ "${v:-0}" -ge 700 ] 2>/dev/null; then
        report PASS "X/Open (SUS) version $v (getconf)"
    else
        report WARN "X/Open (SUS) version ${v:-unknown} (getconf)"
    fi
    report INFO "Options and limits not checked: pass --syscheck (just distro-check builds it)"
else
    report INFO "C library checks skipped: pass --syscheck (just distro-check builds it)"
fi

# The headers of POSIX.1-2024 Base Definitions, chapter 13. devctl.h is new
# in Issue 8 and not in glibc yet; it is reported, not failed.
POSIX_HEADERS="aio.h arpa/inet.h assert.h complex.h cpio.h ctype.h dirent.h
dlfcn.h endian.h errno.h fcntl.h fenv.h float.h fmtmsg.h fnmatch.h ftw.h
glob.h grp.h iconv.h inttypes.h iso646.h langinfo.h libgen.h libintl.h
limits.h locale.h math.h monetary.h mqueue.h ndbm.h net/if.h netdb.h
netinet/in.h netinet/tcp.h nl_types.h poll.h pthread.h pwd.h regex.h sched.h
search.h semaphore.h setjmp.h signal.h spawn.h stdalign.h stdarg.h
stdatomic.h stdbool.h stddef.h stdint.h stdio.h stdlib.h stdnoreturn.h
string.h strings.h sys/ipc.h sys/mman.h sys/msg.h sys/resource.h sys/select.h
sys/sem.h sys/shm.h sys/socket.h sys/stat.h sys/statvfs.h sys/time.h
sys/times.h sys/types.h sys/uio.h sys/un.h sys/utsname.h sys/wait.h syslog.h
tar.h termios.h tgmath.h threads.h time.h uchar.h unistd.h utmpx.h wchar.h
wctype.h wordexp.h"

if [ -n "$CC_CMD" ]; then
    if ! live && ! exists -f /usr/include/stdio.h; then
        report INFO "Headers not checked: $ROOT has no /usr/include (install @core/base:dev)"
    else
        T=$(mktemp -d "${TMPDIR:-/tmp}/distro-check.XXXXXX")
        sysroot=""
        live || sysroot="--sysroot=$ROOT"
        missing="" count=0 n=0
        for h in $POSIX_HEADERS; do
            count=$((count + 1))
            printf '#include <%s>\nint main(void) { return 0; }\n' "$h" >"$T/h.c"
            # shellcheck disable=SC2086 # CC_CMD may carry flags
            $CC_CMD $sysroot -std=c17 -D_XOPEN_SOURCE=700 -fsyntax-only \
                "$T/h.c" 2>/dev/null || {
                missing="$missing $h"
                n=$((n + 1))
            }
        done
        rm -rf "$T"
        if [ -z "$missing" ]; then
            report PASS "POSIX headers compile ($count/$count)"
        else
            report WARN "POSIX headers ($((count - n))/$count), missing or broken:$missing"
        fi
        report INFO "devctl.h (new in POSIX.1-2024) is not checked: glibc does not provide it yet"
    fi
else
    report INFO "Headers not checked: pass --cc (just distro-check passes clang)"
fi

# --- Runtime ----------------------------------------------------------------

section "Runtime" "common practice on a running Linux system"

if ! live; then
    report INFO "Skipped: --root checks files only"
else
    PID1=$(cat /proc/1/comm 2>/dev/null)
    [ -n "$PID1" ] && report INFO "PID 1 is $PID1"

    for m in /proc /sys /dev /run; do
        if grep -q " $m " /proc/mounts 2>/dev/null; then
            report PASS "$m is mounted"
        else
            report FAIL "$m is not mounted"
        fi
    done
    for m in /dev/pts /dev/shm; do
        if grep -q " $m " /proc/mounts 2>/dev/null; then
            report PASS "$m is mounted"
        else
            report WARN "$m is not mounted"
        fi
    done
    for d in /dev/null /dev/zero /dev/urandom /dev/tty; do
        if [ -c "$d" ]; then
            report PASS "$d is a character device"
        else
            report FAIL "$d is missing or not a character device"
        fi
    done
    case $(perms /dev/null) in
    crw-rw-rw-) ;;
    *) report WARN "/dev/null is not crw-rw-rw- ($(perms /dev/null))" ;;
    esac

    if [ -f /sys/fs/cgroup/cgroup.controllers ]; then
        report PASS "cgroup v2 (unified hierarchy)"
    else
        report WARN "No cgroup v2 at /sys/fs/cgroup"
    fi
    userns=$(cat /proc/sys/user/max_user_namespaces 2>/dev/null || echo 0)
    if [ "$userns" -gt 0 ]; then
        report PASS "User namespaces enabled (max $userns)"
    else
        report INFO "User namespaces disabled (bubblewrap and rootless containers need them)"
    fi
    if grep -q '^Seccomp:' /proc/self/status 2>/dev/null; then
        report PASS "seccomp supported"
    else
        report INFO "seccomp not supported by the kernel"
    fi
    if [ -d "/usr/lib/modules/$(uname -r)" ] || [ -d "/lib/modules/$(uname -r)" ]; then
        report PASS "Modules for the running kernel ($(uname -r))"
    else
        report WARN "No modules for the running kernel $(uname -r)"
    fi
    if has_cmd getent && getent hosts localhost >/dev/null 2>&1; then
        report PASS "localhost resolves"
    else
        report WARN "localhost does not resolve (getent hosts localhost)"
    fi
    if has_cmd getent && getent passwd root >/dev/null 2>&1; then
        report PASS "NSS resolves the root account (getent passwd root)"
    else
        report FAIL "NSS does not resolve the root account"
    fi

    if [ "$PID1" = systemd ] && has_cmd systemctl; then
        state=$(systemctl is-system-running 2>/dev/null)
        case $state in
        running) report PASS "systemd state: running" ;;
        *) report WARN "systemd state: $state" ;;
        esac
        failed=$(systemctl --failed --no-legend --plain 2>/dev/null |
            awk '{print $1}' | tr '\n' ' ')
        if [ -z "$failed" ]; then
            report PASS "No failed units"
        else
            report WARN "Failed units: $failed"
        fi
        if has_cmd timedatectl; then
            case $(timedatectl show -p NTPSynchronized --value 2>/dev/null) in
            yes) report PASS "Clock is synchronized (NTP)" ;;
            *) report INFO "Clock is not synchronized (NTP)" ;;
            esac
        fi
    fi

    if [ "$NETWORK" = 1 ]; then
        if has_cmd getent && getent hosts example.org >/dev/null 2>&1; then
            report PASS "DNS resolves example.org"
        else
            report WARN "DNS cannot resolve example.org"
        fi
        if has_cmd curl; then
            if curl -sSfI --max-time 15 https://example.org >/dev/null 2>&1; then
                report PASS "HTTPS works (certificate verified)"
            else
                report WARN "HTTPS to example.org failed (network or CA bundle)"
            fi
        fi
    else
        report INFO "Network checks skipped (use --network)"
    fi
fi

# --- Summary ----------------------------------------------------------------

end_section
printf '\n%s== Summary ==%s\n' "$C_HEAD" "$C_OFF"
printf '%s' "$SUMMARY"
SCORED=$((N_PASS + N_WARN + N_FAIL))
if [ "$SCORED" -gt 0 ]; then
    SCORE=$((N_PASS * 100 / SCORED))
else
    SCORE=0
fi
printf '\n%s%d passed%s, %s%d failed%s, %s%d warnings%s, %s%d notes%s\n' \
    "$C_PASS" "$N_PASS" "$C_OFF" "$C_FAIL" "$N_FAIL" "$C_OFF" \
    "$C_WARN" "$N_WARN" "$C_OFF" "$C_INFO" "$N_INFO" "$C_OFF"
printf 'Score: %d%% (passed out of passed, failed and warnings)\n' "$SCORE"

[ "$N_FAIL" -eq 0 ]
