package commands

import (
	"encoding/json"
	"regexp"
	"strconv"
	"strings"
)

type AppDefinition struct {
	Name         string                `json:"name"`
	Dependencies map[string]Dependency `json:"dependencies"`
	Create       string                `json:"create"`
}

type Dependency struct {
	Check   string      `json:"check"`
	Install InstallSpec `json:"install"`
}

type InstallSpec struct {
	Linux   map[string]string `json:"linux,omitempty"`
	Darwin  string            `json:"darwin,omitempty"`
	Win32   string            `json:"win32,omitempty"`
	Default string            `json:"default,omitempty"`
}

func (i *InstallSpec) UnmarshalJSON(data []byte) error {
	var defaultCmd string
	if err := json.Unmarshal(data, &defaultCmd); err == nil {
		i.Default = defaultCmd
		return nil
	}

	type Alias InstallSpec
	var tmp Alias
	if err := json.Unmarshal(data, &tmp); err != nil {
		return err
	}
	*i = InstallSpec(tmp)
	return nil
}

type YamlConfig struct {
	AppName       string        `yaml:"appName"`
	Packages      []YamlPackage `yaml:"packages"`
	CreateCommand string        `yaml:"createCommand"`
}

type YamlPackage struct {
	Name    string          `yaml:"name"`
	Check   string          `yaml:"check"`
	Install InstallCommands `yaml:"install"`
}

type InstallCommands struct {
	Linux   string `yaml:"linux,omitempty"`
	Windows string `yaml:"windows,omitempty"`
	Darwin  string `yaml:"darwin,omitempty"`
	Default string `yaml:"default,omitempty"`
}

type PackageManager struct {
	Name         string
	CheckCommand string
	InstallCmd   string
	VersionRegex *regexp.Regexp
}

var packageManagers = map[string]PackageManager{
	"scoop": {
		Name:         "scoop",
		CheckCommand: "scoop which %s",
		InstallCmd:   "scoop install %s",
		VersionRegex: regexp.MustCompile(`Version:\s+([\d.]+)`),
	},
	"apt": {
		Name:         "apt",
		CheckCommand: "dpkg -s %s",
		InstallCmd:   "sudo apt install -y %s",
		VersionRegex: regexp.MustCompile(`Version:\s+(.+)`),
	},
	"pacman": {
		Name:         "pacman",
		CheckCommand: "pacman -Qi %s",
		InstallCmd:   "sudo pacman -S --noconfirm %s",
		VersionRegex: regexp.MustCompile(`Version\s+:\s+([\d\w.:-]+)`),
	},
	"dnf": {
		Name:         "dnf",
		CheckCommand: "dnf list installed %s",
		InstallCmd:   "sudo dnf install -y %s",
		VersionRegex: regexp.MustCompile(`([\d.]+)-\d+\..+`),
	},
	"yum": {
		Name:         "yum",
		CheckCommand: "yum list installed %s",
		InstallCmd:   "sudo yum install -y %s",
		VersionRegex: regexp.MustCompile(`([\d.]+)-\d+\..+`),
	},
	"apk": {
		Name:         "apk",
		CheckCommand: "apk info -e %s",
		InstallCmd:   "sudo apk add %s",
		VersionRegex: regexp.MustCompile(`-\s*([\d.]+[a-zA-Z]*)`),
	},
	"brew": {
		Name:         "brew",
		CheckCommand: "brew list --versions %s",
		InstallCmd:   "brew install %s",
		VersionRegex: regexp.MustCompile(`\s+(\d+\.\d+\.\d+|\d+\.\d+)`),
	},
}

type GlobalConfig struct {
	AppsURL string `yaml:"appsUrl"`
}

const (
	defaultAppsURL = "https://zusty.codeberg.page/Zoi/@app/apps.json"
	configDir      = ".zoi"
	configFile     = "config.yaml"
)

type Version struct {
	Branch string
	Status string
	Number string
}

var statusOrder = map[string]int{
	"Pre-Alpha":    0,
	"Alpha":        1,
	"Pre-Beta":     2,
	"Beta":         3,
	"Pre-Release":  4,
	"Early-Access": 5,
	"Demo":         6,
	"Release":      7,
}

func parseVersion(branch, status, number string) Version {
	return Version{
		Branch: branch,
		Status: status,
		Number: number,
	}
}

func (v Version) Compare(other Version) int {
	currentRank, okCurrent := statusOrder[v.Status]
	otherRank, okOther := statusOrder[other.Status]
	if !okCurrent {
		currentRank = -1
	}
	if !okOther {
		otherRank = -1
	}

	if otherRank > currentRank {
		return 1
	}
	if otherRank < currentRank {
		return -1
	}

	return compareSemver(v.Number, other.Number)
}

func compareSemver(a, b string) int {
	cleanRegex := regexp.MustCompile(`^(\d+\.\d+(?:\.\d+)?)?.*`)
	aClean := cleanRegex.ReplaceAllString(a, "$1")
	bClean := cleanRegex.ReplaceAllString(b, "$1")

	aParts := strings.Split(aClean, ".")
	bParts := strings.Split(bClean, ".")

	maxLen := len(aParts)
	if len(bParts) > maxLen {
		maxLen = len(bParts)
	}

	for i := 0; i < maxLen; i++ {
		var aNum, bNum int
		if i < len(aParts) {
			numStr := regexp.MustCompile(`^(\d+).*`).ReplaceAllString(aParts[i], "$1")
			aNum, _ = strconv.Atoi(numStr)
		}
		if i < len(bParts) {
			numStr := regexp.MustCompile(`^(\d+).*`).ReplaceAllString(bParts[i], "$1")
			bNum, _ = strconv.Atoi(numStr)
		}

		if bNum > aNum {
			return 1
		}
		if bNum < aNum {
			return -1
		}
	}

	return 0
}

type VersionInfo struct {
	Latest struct {
		Production struct {
			Version string `json:"version"`
			Status  string `json:"status"`
		} `json:"production"`
		Development struct {
			Version string `json:"version"`
			Status  string `json:"status"`
		} `json:"development"`
	} `json:"latest"`
}

type RunCommandItem struct {
	Cmd string `yaml:"cmd"`
	Run string `yaml:"run"`
}

type RunConfig struct {
	Commands []RunCommandItem `yaml:"commands"`
}

type ZoiFileConfig struct {
	Name         string            `yaml:"name,omitempty"`
	Packages     []YamlPackage     `yaml:"packages,omitempty"`
	Commands     []RunCommandItem  `yaml:"commands,omitempty"`
	Environments []EnvironmentSpec `yaml:"environments,omitempty"`
}

type EnvironmentSpec struct {
	Name  string   `yaml:"name"`
	Cmd   string   `yaml:"cmd"`
	Run   []string `yaml:"run,omitempty"`
	Check []string `yaml:"check,omitempty"`
}
