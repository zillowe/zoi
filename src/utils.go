package src

import (
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"io"

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
			managersToProbe := []string{"apt", "pacman", "yum", "dnf", "apk", "zypper"}
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

func UpdateSymlink(targetPath, linkPath string) error {
	if err := os.MkdirAll(filepath.Dir(linkPath), 0755); err != nil {
		return err
	}
	if _, err := os.Lstat(linkPath); err == nil {
		if err := os.Remove(linkPath); err != nil {
			return fmt.Errorf("failed to remove existing symlink at %s: %w", linkPath, err)
		}
	} else if !os.IsNotExist(err) {
		return fmt.Errorf("failed to check symlink status at %s: %w", linkPath, err)
	}

	return os.Symlink(targetPath, linkPath)
}

func GetShellExtension() string {
	if runtime.GOOS == "windows" {
		return "ps1"
	}
	return "sh"
}

func AddPkgPathToShell(shellName string) error {
	home, err := os.UserHomeDir()
	if err != nil {
		return err
	}

	pathToAdd := filepath.Join(home, ".zoi", "pkgs", "bins")
	comment := "# Added by Zoi Package Manager to enable its commands"
	var profilePath, pathCmd string

	switch strings.ToLower(shellName) {
	case "bash":
		profilePath = filepath.Join(home, ".bashrc")
		pathCmd = fmt.Sprintf("export PATH=\"%s:$PATH\"", pathToAdd)
	case "zsh":
		profilePath = filepath.Join(home, ".zshrc")
		pathCmd = fmt.Sprintf("export PATH=\"%s:$PATH\"", pathToAdd)
	case "fish":
		profilePath = filepath.Join(home, ".config", "fish", "config.fish")
		pathCmd = fmt.Sprintf("fish_add_path \"%s\"", pathToAdd)
	default:
		return fmt.Errorf("unsupported shell '%s'. Supported shells are: bash, zsh, fish", shellName)
	}

	if err := os.MkdirAll(filepath.Dir(profilePath), 0755); err != nil {
		return fmt.Errorf("failed to create profile directory: %w", err)
	}

	content, err := os.ReadFile(profilePath)
	if err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("could not read profile file %s: %w", profilePath, err)
	}

	if strings.Contains(string(content), pathToAdd) {
		PrintSuccess("PATH already configured in %s. No changes needed.", profilePath)
		return nil
	}

	f, err := os.OpenFile(profilePath, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644)
	if err != nil {
		return fmt.Errorf("failed to open profile file %s: %w", profilePath, err)
	}
	defer f.Close()

	lineToAdd := fmt.Sprintf("\n%s\n%s\n", comment, pathCmd)
	if _, err := f.WriteString(lineToAdd); err != nil {
		return fmt.Errorf("failed to write to profile file: %w", err)
	}

	PrintSuccess("Successfully updated %s!", profilePath)
	PrintInfo("Please restart your shell or run 'source %s' for the changes to take effect.", profilePath)

	return nil
}

func DownloadAndExecuteScript(scriptURL string) error {
	PrintInfo("Downloading installer script from %s...", scriptURL)

	resp, err := http.Get(scriptURL)
	if err != nil {
		return fmt.Errorf("failed to start download: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("failed to download script: received status code %d", resp.StatusCode)
	}

	filePattern := "zoi-installer-*" + GetShellExtension()
	tempFile, err := os.CreateTemp("", filePattern)
	if err != nil {
		return fmt.Errorf("failed to create temporary file for installer: %w", err)
	}
	defer os.Remove(tempFile.Name())

	if _, err := io.Copy(tempFile, resp.Body); err != nil {
		return fmt.Errorf("failed to write installer script to disk: %w", err)
	}

	if err := tempFile.Close(); err != nil {
		return fmt.Errorf("failed to close temporary file: %w", err)
	}

	if runtime.GOOS != "windows" {
		if err := os.Chmod(tempFile.Name(), 0755); err != nil {
			return fmt.Errorf("failed to make installer script executable: %w", err)
		}
	}

	PrintInfo("\nExecuting installer script...")
	PrintInfo("The script may prompt for your password to complete the installation.")
	return ExecuteCommand(tempFile.Name())
}
