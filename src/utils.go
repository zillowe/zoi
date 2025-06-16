package src

import (
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"os/exec"
	"runtime"
	"strings"

	"github.com/fatih/color"
	"github.com/spf13/viper"
)

func ExecuteCommand(command string) error {
	var shell, flag string

	if runtime.GOOS == "windows" {
		shell = "powershell"
		flag = "-Command"
	} else {
		shell = "bash"
		flag = "-c"
	}

	cmd := exec.Command(shell, flag, command)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	return cmd.Run()
}

func CheckCommand(command string) bool {
	if command == "" {
		return false
	}

	var shell, flag string
	commandName := strings.Fields(command)[0]

	if runtime.GOOS == "windows" {
		shell = "powershell"
		command = fmt.Sprintf("Get-Command %s", commandName)
		flag = "-Command"
	} else {
		shell = "bash"
		command = fmt.Sprintf("command -v %s", commandName)
		flag = "-c"
	}

	cmd := exec.Command(shell, flag, command)
	cmd.Stdout = nil
	cmd.Stderr = nil

	return cmd.Run() == nil
}

func InstallPackage(pkgManager, packageName string) error {
	PrintInfo("--> Installing package: %s", packageName)
	var command string
	switch pkgManager {
	case "apt":
		command = fmt.Sprintf("sudo apt-get install -y %s", packageName)
	case "pacman":
		command = fmt.Sprintf("sudo pacman -S --noconfirm %s", packageName)
	case "scoop":
		command = fmt.Sprintf("scoop install %s", packageName)
	case "brew":
		command = fmt.Sprintf("brew install %s", packageName)
	case "yum":
		command = fmt.Sprintf("sudo yum install -y %s", packageName)
	case "dnf":
		command = fmt.Sprintf("sudo dnf install -y %s", packageName)
	case "apk":
		command = fmt.Sprintf("sudo apk add %s", packageName)
	default:
		return fmt.Errorf("unsupported or unknown package manager: %s", pkgManager)
	}
	return ExecuteCommand(command)
}

func InstallApp(installCmd string) error {
	PrintInfo("--> Installing application...")
	return ExecuteCommand(installCmd)
}

func GetSystemInfo() (string, string, string, string) {
	osName := runtime.GOOS
	arch := runtime.GOARCH
	var distro, pkgManager string

	if osName == "linux" {
		out, err := exec.Command("sh", "-c", "cat /etc/os-release").Output()
		if err == nil {
			lines := strings.Split(string(out), "\n")
			for _, line := range lines {
				if strings.HasPrefix(line, "ID=") {
					distro = strings.TrimPrefix(line, "ID=")
					distro = strings.Trim(distro, "\"")
					break
				}
			}
		}

		switch distro {
		case "arch", "manjaro", "cachyos":
			pkgManager = "pacman"
		case "debian", "ubuntu", "mint", "pop", "raspbian", "kali":
			pkgManager = "apt"
		case "alpine":
			pkgManager = "apk"
		case "fedora", "centos", "rhel", "almalinux":
			pkgManager = "dnf"
		case "opensuse", "opensuse-tumbleweed", "opensuse-leap":
			pkgManager = "zypper"
		}

		if pkgManager == "" {
			if distro != "" {
				PrintInfo("Distribution '%s' not in the known list. Probing for common package managers...", distro)
			} else {
				PrintInfo("Could not determine distribution ID. Probing for common package managers...")
			}
			managersToProbe := []string{"apt", "pacman", "yum", "dnf", "apk"}
			for _, pm := range managersToProbe {
				if CheckCommand(pm) {
					PrintSuccess("Detected package manager: %s", pm)
					pkgManager = pm
					break
				}
			}
		}

	} else if osName == "windows" {
		pkgManager = "scoop"
	} else if osName == "darwin" {
		pkgManager = "brew"
	}

	return osName, arch, distro, pkgManager
}

func GetApps() (map[string]App, error) {
	appsUrl := viper.GetString("appsUrl")
	resp, err := http.Get(appsUrl)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	var apps map[string]App
	if err := json.NewDecoder(resp.Body).Decode(&apps); err != nil {
		return nil, err
	}
	return apps, nil
}

func Yellow() *color.Color {
	return color.New(color.FgYellow)
}

func PrintBlue(format string, a ...interface{}) {
	blue := color.New(color.FgBlue).SprintFunc()
	fmt.Println(blue(fmt.Sprintf(format, a...)))
}

func PrintSuccess(format string, a ...interface{}) {
	green := color.New(color.FgGreen).SprintFunc()
	fmt.Println(green(fmt.Sprintf(format, a...)))
}

func PrintError(format string, a ...interface{}) {
	red := color.New(color.FgRed).SprintFunc()
	fmt.Println(red(fmt.Sprintf(format, a...)))
}

func PrintInfo(format string, a ...interface{}) {
	cyan := color.New(color.FgCyan).SprintFunc()
	fmt.Println(cyan(fmt.Sprintf(format, a...)))
}

func PrintHighlight(format string, a ...interface{}) {
	magenta := color.New(color.FgMagenta).SprintFunc()
	fmt.Println(magenta(fmt.Sprintf(format, a...)))
}
