package commands

import (
	"bufio"
	"context"
	"fmt"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"runtime"
	"strings"
	"time"

	"github.com/fatih/color"
)

func getOS() string {
	switch os := runtime.GOOS; os {
	case "windows":
		return "Windows"
	case "darwin":
		return "macOS"
	case "linux":
		return "Linux"
	default:
		return os
	}
}

func getLinuxDistro() string {
	files := []string{"/etc/os-release", "/etc/lsb-release"}
	idRegex := regexp.MustCompile(`(?m)^ID=(?:["']?)(.+?)(?:["']?)$`)
	for _, file := range files {
		data, err := os.ReadFile(file)
		if err == nil {
			matches := idRegex.FindStringSubmatch(string(data))
			if len(matches) > 1 {
				return strings.ToLower(matches[1])
			}
		}
	}
	if _, err := os.Stat("/etc/arch-release"); err == nil {
		return "arch"
	}
	if _, err := os.Stat("/etc/redhat-release"); err == nil {
		return "fedora-rhel-centos"
	}
	if _, err := os.Stat("/etc/debian_version"); err == nil {
		return "debian"
	}
	if _, err := os.Stat("/etc/alpine-release"); err == nil {
		return "alpine"
	}
	return "unknown"
}

func getArch() string {
	switch arch := runtime.GOARCH; arch {
	case "amd64":
		return "x86_64"
	case "386":
		return "x86"
	case "arm64":
		return "ARM64"
	default:
		return arch
	}
}

func executeCommand(command string) error {
	var cmd *exec.Cmd
	if runtime.GOOS == "windows" {
		cmd = exec.Command("cmd", "/C", command)
	} else {
		cmd = exec.Command("sh", "-c", command)
	}
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	return cmd.Run()
}

func runCommandOutput(command string) (string, error) {
	var cmd *exec.Cmd
	if runtime.GOOS == "windows" {
		cmd = exec.Command("pwsh", "/C", command)
	} else {
		cmd = exec.Command("bash", "-c", command)
	}
	out, err := cmd.CombinedOutput()
	return string(out), err
}

func detectPackageManager() (PackageManager, error) {
	os := runtime.GOOS
	distro := getLinuxDistro()
	switch {
	case os == "windows":
		if commandExists("scoop") {
			return packageManagers["scoop"], nil
		}
		return PackageManager{}, fmt.Errorf("no supported package manager (scoop) found on Windows")
	case os == "darwin":
		if commandExists("brew") {
			return packageManagers["brew"], nil
		}
		return PackageManager{}, fmt.Errorf("no supported package manager (brew) found on macOS")
	case os == "linux":
		if distro == "arch" || distro == "cachyos" || distro == "manjaro" {
			if commandExists("pacman") {
				return packageManagers["pacman"], nil
			}
		}
		if distro == "fedora" || distro == "rhel" || distro == "centos" || distro == "fedora-rhel-centos" {
			if commandExists("dnf") {
				return packageManagers["dnf"], nil
			}
			if commandExists("yum") {
				return packageManagers["yum"], nil
			}
		}
		if distro == "alpine" {
			if commandExists("apk") {
				return packageManagers["apk"], nil
			}
		}
		if distro == "debian" || distro == "ubuntu" || distro == "kali" || distro == "raspbian" {
			if commandExists("apt-get") {
				return packageManagers["apt"], nil
			}
		}
		if commandExists("apt-get") {
			return packageManagers["apt"], nil
		}
		if commandExists("dnf") {
			return packageManagers["dnf"], nil
		}
		if commandExists("pacman") {
			return packageManagers["pacman"], nil
		}
		if commandExists("apk") {
			return packageManagers["apk"], nil
		}
		if commandExists("yum") {
			return packageManagers["yum"], nil
		}
		return PackageManager{}, fmt.Errorf("unsupported Linux distribution '%s' or no recognized package manager found", distro)
	default:
		return PackageManager{}, fmt.Errorf("unsupported operating system: %s", os)
	}
}

func commandExists(cmd string) bool {
	_, err := exec.LookPath(cmd)
	return err == nil
}

func confirmPrompt(prompt string) bool {
	yellow := color.New(color.FgYellow).SprintFunc()
	cyan := color.New(color.FgCyan).SprintFunc()
	fmt.Printf("%s %s [y/N] ", yellow("?"), cyan(prompt))
	reader := bufio.NewReader(os.Stdin)
	input, err := reader.ReadString('\n')
	if err != nil {
		return false
	}
	input = strings.TrimSpace(strings.ToLower(input))
	return input == "y" || input == "yes"
}

func checkNetwork() {
	green := color.New(color.FgGreen).SprintFunc()
	red := color.New(color.FgRed).SprintFunc()
	cyan := color.New(color.FgCyan).SprintFunc()

	fmt.Printf("  %s Checking DNS resolution (google.com)... ", cyan("▸"))
	resolver := net.Resolver{
		PreferGo: true,
		Dial: func(ctx context.Context, network, address string) (net.Conn, error) {
			d := net.Dialer{Timeout: 5 * time.Second}
			return d.DialContext(ctx, network, "8.8.8.8:53")
		},
	}
	_, err := resolver.LookupHost(context.Background(), "google.com")
	if err == nil {
		fmt.Printf("%s\n", green("✓ Working"))
	} else {
		fmt.Printf("%s (Error: %v)\n", red("✗ Failed"), err)
	}

	targetURL := "https://detectportal.firefox.com/success.txt"
	fmt.Printf("  %s Checking HTTPS connectivity (%s)... ", cyan("▸"), targetURL)
	client := http.Client{Timeout: 10 * time.Second}
	resp, err := client.Get(targetURL)
	if err == nil && resp.StatusCode == http.StatusOK {
		resp.Body.Close()
		fmt.Printf("%s\n", green("✓ Working"))
	} else if err != nil {
		fmt.Printf("%s (Error: %v)\n", red("✗ Failed"), err)
	} else {
		resp.Body.Close()
		fmt.Printf("%s (Status Code: %d)\n", red("✗ Failed"), resp.StatusCode)
	}

	fmt.Printf("  %s Testing network latency (ping 8.8.8.8)... ", cyan("▸"))
	ctx, cancel := context.WithTimeout(context.Background(), 4*time.Second)
	defer cancel()

	var cmd *exec.Cmd
	if runtime.GOOS == "windows" {
		cmd = exec.CommandContext(ctx, "ping", "-n", "1", "-w", "3000", "8.8.8.8")
	} else {
		cmd = exec.CommandContext(ctx, "ping", "-c", "1", "-W", "3", "8.8.8.8")
	}

	if err := cmd.Run(); err == nil {
		fmt.Printf("%s\n", green("✓ Responsive"))
	} else {
		if ctx.Err() == context.DeadlineExceeded {
			fmt.Printf("%s (Timeout)\n", red("✗ Timeout"))
		} else {
			fmt.Printf("%s (Error: %v)\n", red("✗ Failed/Unreachable"), err)
		}
	}
}

func extractVersion(output string) string {
	baseVersionRegex := regexp.MustCompile(`(?i)(?:v|version)?\s*(\d+\.\d+(?:\.\d+)?)`)

	matches := baseVersionRegex.FindStringSubmatch(output)
	if len(matches) > 1 {
		cleanVersion := matches[1]
		return cleanVersion
	}

	simpleVersionRegex := regexp.MustCompile(`(\d+\.\d+)`)
	matches = simpleVersionRegex.FindStringSubmatch(output)
	if len(matches) > 0 {
		return matches[0]
	}

	return ""
}

func versionMatches(want, actual string) bool {
	if want == "" || actual == "" {
		return false
	}
	return strings.HasPrefix(actual, want)
}

func checkToolDirectly(pkg string) (string, error) {
	cmd := fmt.Sprintf("%s --version", pkg)
	output, err := runCommandOutput(cmd)
	if err == nil {
		version := extractVersion(output)
		if version != "" {
			return version, nil
		}
	}

	cmd = fmt.Sprintf("%s version", pkg)
	output, err = runCommandOutput(cmd)
	if err == nil {
		version := extractVersion(output)
		if version != "" {
			return version, nil
		}
	}

	return "", fmt.Errorf("command '%s --version' or '%s version' failed or produced no parsable version output", pkg, pkg)
}

func NotFoundCommand() {
	red := color.New(color.FgRed).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()

	attemptedCmd := ""
	if len(os.Args) > 1 {
		attemptedCmd = os.Args[1]
	}

	fmt.Printf("%s Unknown command: '%s'\n", red("Error:"), yellow(attemptedCmd))
	fmt.Printf("%s Run 'zoi help' for a list of available commands.\n", yellow("Hint:"))
}

func getConfigPath() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(home, configDir, configFile), nil
}
