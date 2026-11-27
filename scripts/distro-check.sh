#!/bin/sh

# Checks how complete a Linux distribution is: the utilities, files,
# directories and runtime features that POSIX, the FHS and common practice
# on mainstream distributions (Arch, Debian, Fedora) lead users and programs
# to expect.
#
# It can check the running system, or an unbooted root such as the folder
# `zoi system distro build` creates (`--root`). With `--root`, only files are
# checked: nothing from the root is run, and the runtime section is skipped.
#
# Every result is one of:
#   PASS  present, or working
#   FAIL  missing, and most programs or users rely on it
#   WARN  missing, but expected on most distributions
#   INFO  optional, or worth knowing
#
# The script is POSIX sh so it runs even where bash is missing. It exits 1
# if any check failed, 2 on a usage error, and 0 otherwise.
#
# Usage: scripts/distro-check.sh [--root DIR] [--network] [--no-color]

ROOT=""
NETWORK=0
COLOR=1
[ -t 1 ] || COLOR=0

usage() {
    cat <<'EOF'
Usage: distro-check.sh [OPTIONS]

Check how complete a Linux distribution is: POSIX and common utilities, the
FHS layout, system files, accounts, and (on a running system) runtime
features.

Options:
  -r, --root DIR   Check the unbooted root in DIR instead of this system.
                   Only files are checked; nothing from DIR is run.
  -n, --network    Also check DNS and HTTPS (running system only).
      --no-color   Do not color the output.
  -h, --help       Show this help.

Exit status: 1 if any check failed, 2 on a usage error, 0 otherwise.
EOF
}

while [ $# -gt 0 ]; do
    case $1 in
    -r | --root)
        [ $# -ge 2 ] || {
            echo "Error: $1 needs a directory." >&2
            exit 2
        }
        ROOT=$2
        shift
        ;;
    --root=*) ROOT=${1#--root=} ;;
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

section() { printf '\n%s== %s ==%s\n' "$C_HEAD" "$1" "$C_OFF"; }

# report LEVEL MESSAGE
report() {
    case $1 in
    PASS) N_PASS=$((N_PASS + 1)) color=$C_PASS ;;
    FAIL) N_FAIL=$((N_FAIL + 1)) color=$C_FAIL ;;
    WARN) N_WARN=$((N_WARN + 1)) color=$C_WARN ;;
    *) N_INFO=$((N_INFO + 1)) color=$C_INFO ;;
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
        exists -d "$d" || d_missing="$d_missing $d"
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

if live; then
    TARGET="this system ($(uname -sr))"
else
    TARGET="$ROOT"
fi
printf '%sChecking %s%s\n' "$C_HEAD" "$TARGET" "$C_OFF"

# --- System identity --------------------------------------------------------

section "System identity"

OS_RELEASE=""
for f in /etc/os-release /usr/lib/os-release; do
    if exists -f "$f"; then
        OS_RELEASE=$f
        break
    fi
done
if [ -n "$OS_RELEASE" ]; then
    report PASS "os-release ($OS_RELEASE)"
    os_field() {
        read_file "$OS_RELEASE" | sed -n "s/^$1=//p" | head -n 1 |
            sed 's/^"\(.*\)"$/\1/'
    }
    PRETTY=$(os_field PRETTY_NAME)
    [ -n "$PRETTY" ] && report INFO "Distribution: $PRETTY"
    for field in NAME ID PRETTY_NAME; do
        if [ -n "$(os_field "$field")" ]; then
            report PASS "os-release sets $field"
        else
            report WARN "os-release lacks $field"
        fi
    done
    if [ -n "$(os_field VERSION_ID)$(os_field BUILD_ID)" ]; then
        report PASS "os-release sets VERSION_ID or BUILD_ID"
    else
        report WARN "os-release sets neither VERSION_ID nor BUILD_ID"
    fi
    for field in HOME_URL BUG_REPORT_URL ID_LIKE LOGO; do
        [ -n "$(os_field "$field")" ] ||
            report INFO "os-release lacks optional $field"
    done
    exists -e /usr/lib/os-release ||
        report WARN "/usr/lib/os-release missing (os-release(5) puts the file there)"
else
    report FAIL "os-release missing (/etc/os-release, /usr/lib/os-release)"
fi

check_path WARN -s /etc/hostname "Hostname file"
if exists -s /etc/machine-id; then
    report PASS "Machine ID (/etc/machine-id)"
elif live; then
    report WARN "/etc/machine-id missing or empty"
else
    report INFO "/etc/machine-id empty (fine: systemd creates it at first boot)"
fi

if exists -e /etc/localtime; then
    report PASS "Time zone (/etc/localtime -> $(readlink "$ROOT/etc/localtime"))"
else
    report WARN "Time zone not set (/etc/localtime)"
fi
check_path WARN -f /usr/share/zoneinfo/UTC "Time zone database"
check_path WARN -f /etc/locale.conf "System locale"

if live && has_cmd locale; then
    if locale -a 2>/dev/null | grep -qi '^c\.utf-\{0,1\}8$'; then
        report PASS "C.UTF-8 locale available"
    else
        report WARN "C.UTF-8 locale missing (locale -a)"
    fi
    if [ "$(locale charmap 2>/dev/null)" = UTF-8 ]; then
        report PASS "Current locale uses UTF-8"
    else
        report WARN "Current locale is not UTF-8 ($(locale charmap 2>/dev/null))"
    fi
elif exists -d /usr/lib/locale/C.utf8 || exists -d /usr/lib/locale/C.UTF-8; then
    report PASS "C.UTF-8 locale available"
elif exists -f /usr/lib/locale/locale-archive; then
    report INFO "Locales compiled in /usr/lib/locale/locale-archive (C.UTF-8 not checked)"
else
    report WARN "No compiled locales in /usr/lib/locale"
fi

# --- Filesystem layout (FHS) ------------------------------------------------

section "Filesystem layout (FHS 3.0)"

check_dirs FAIL "Required directories" \
    /bin /dev /etc /lib /proc /root /run /sbin /sys /tmp /usr /usr/bin \
    /usr/lib /usr/share /var
check_dirs WARN "Common directories" \
    /boot /home /media /mnt /opt /srv /usr/include /usr/local \
    /usr/local/bin /usr/sbin /usr/share/man /var/cache /var/lib /var/log \
    /var/spool /var/tmp

for d in /tmp /var/tmp; do
    case $(perms "$d") in
    d????????t | d????????T) report PASS "$d is sticky ($(perms "$d"))" ;;
    "") ;;
    *) report WARN "$d is not world-writable and sticky ($(perms "$d"); expected drwxrwxrwt)" ;;
    esac
done
case $(perms /root) in
d???------ | d???r-x--- | d???r-----) report PASS "/root is private ($(perms /root))" ;;
"") ;;
*) report WARN "/root is readable by others ($(perms /root))" ;;
esac

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

if [ -L "$ROOT/var/run" ]; then
    report PASS "/var/run links to $(readlink "$ROOT/var/run")"
else
    report WARN "/var/run is not a link to /run"
fi
if [ -L "$ROOT/etc/mtab" ]; then
    report PASS "/etc/mtab links to $(readlink "$ROOT/etc/mtab")"
else
    report WARN "/etc/mtab is not a link to /proc/self/mounts"
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
        report PASS "Dynamic loader ($found)"
    else
        report FAIL "Dynamic loader missing (one of: $LOADERS)"
    fi
fi

# --- Accounts and authentication --------------------------------------------

section "Accounts and authentication"

if read_file /etc/passwd | grep -q '^root:[^:]*:0:'; then
    report PASS "root account with UID 0 (/etc/passwd)"
else
    report FAIL "No root account with UID 0 in /etc/passwd"
fi
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
    report WARN "/etc/shells missing (chsh and some FTP/SSH setups use it)"
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

section "System configuration"

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

section "Boot and kernel"

if exists -x /sbin/init || exists -x /usr/sbin/init || exists -x /usr/lib/systemd/systemd; then
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

# --- Utilities --------------------------------------------------------------

section "Utilities"

check_cmds FAIL "Essential commands" \
    sh cat cp mv rm ln ls mkdir rmdir chmod chown touch echo printf test \
    head tail sort uniq wc cut tr grep sed awk find xargs tar gzip env id \
    uname date sleep kill ps mount umount sync basename dirname expr tee \
    true false
check_cmds WARN "POSIX utilities" \
    ar at awk basename batch bc cal cksum cmp comm crontab csplit dd df \
    diff du ed ex expand file fold fuser getconf gettext iconv ipcrm ipcs \
    join link locale logger logname m4 mailx make man mesg mkfifo more \
    newgrp nice nl nohup od paste patch pathchk pr pwd readlink realpath \
    renice split strings stty tabs time timeout tput tsort tty uname \
    unexpand unlink vi who write zcat
check_cmds INFO "Rarely installed POSIX utilities" \
    asa c17 cflow ctags cxref compress uncompress gencat lex localedef lp \
    msgfmt ngettext nm pax strip talk uucp uudecode uuencode uustat uux \
    xgettext yacc
if live; then
    # Builtins of the shell; only a running shell can be asked about them.
    missing=""
    for b in alias bg cd command fc fg getopts hash jobs read type ulimit \
        umask unalias wait; do
        sh -c "command -v $b" >/dev/null 2>&1 || missing="$missing $b"
    done
    if [ -z "$missing" ]; then
        report PASS "POSIX shell builtins in /bin/sh"
    else
        report WARN "/bin/sh lacks builtins:$missing"
    fi
fi

check_cmds WARN "File and text tools (coreutils and friends)" \
    install mktemp realpath stat sha256sum sha512sum md5sum base64 nproc \
    seq truncate tac chroot mknod less which diff3 sdiff
check_cmds WARN "util-linux" \
    lsblk findmnt blkid fdisk sfdisk losetup mkswap swapon swapoff su \
    login agetty dmesg flock unshare nsenter hwclock wipefs column hexdump \
    lscpu setsid
check_cmds WARN "Processes (procps, psmisc)" \
    top free pgrep pkill pidof sysctl uptime watch vmstat killall
check_cmds WARN "Users and groups (shadow)" \
    useradd usermod userdel groupadd groupmod groupdel passwd chpasswd \
    chsh gpasswd
check_cmds WARN "Compression" gzip bzip2 xz zstd
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
check_any INFO "Manual page viewer" man mandoc
check_cmds INFO "Shells" bash zsh dash fish
check_cmds INFO "Developer toolchain" cc make pkg-config ld as gdb

INIT_TARGET=$(resolve /sbin/init)
case $INIT_TARGET in
*systemd*)
    check_cmds WARN "systemd tools" \
        systemctl journalctl loginctl hostnamectl timedatectl localectl \
        busctl systemd-analyze udevadm
    check_cmds INFO "systemd network tools" resolvectl networkctl
    ;;
esac

check_any INFO "Package manager" zoi pacman apt dnf zypper apk xbps-install emerge nix-env

# --- Runtime ----------------------------------------------------------------

section "Runtime"

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

    if (
        set -e
        x=$((1 + 2))
        [ "$x" = 3 ]
        y=$(printf ok)
        [ "$y" = ok ]
        w=b
        case $w in b) : ;; esac
        f() { return 0; }
        f
    ) >/dev/null 2>&1 &&
        sh -c 'set -e; v=$((2*3)); [ "$v" = 6 ]; OPTIND=1; getopts a o -a;
            [ "$o" = a ]; t=$(echo x); [ "$t" = x ]' >/dev/null 2>&1; then
        report PASS "/bin/sh runs POSIX shell syntax"
    else
        report FAIL "/bin/sh fails basic POSIX shell syntax"
    fi

    if has_cmd getent && getent hosts localhost >/dev/null 2>&1; then
        report PASS "localhost resolves"
    else
        report WARN "localhost does not resolve (getent hosts localhost)"
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

section "Summary"
SCORED=$((N_PASS + N_WARN + N_FAIL))
if [ "$SCORED" -gt 0 ]; then
    SCORE=$((N_PASS * 100 / SCORED))
else
    SCORE=0
fi
printf '%s%d passed%s, %s%d failed%s, %s%d warnings%s, %s%d notes%s\n' \
    "$C_PASS" "$N_PASS" "$C_OFF" "$C_FAIL" "$N_FAIL" "$C_OFF" \
    "$C_WARN" "$N_WARN" "$C_OFF" "$C_INFO" "$N_INFO" "$C_OFF"
printf 'Score: %d%% (passed out of passed, failed and warnings)\n' "$SCORE"

[ "$N_FAIL" -eq 0 ]
