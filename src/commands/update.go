package commands

import (
	"encoding/json"
	"fmt"
	"net/http"
	"runtime"
	"strings"
	"time"

	"github.com/fatih/color"
)

func UpdateCommand(VerBranch, VerStatus, VerNumber string) {
	red := color.New(color.FgRed).SprintFunc()
	green := color.New(color.FgGreen).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()
	cyan := color.New(color.FgCyan).SprintFunc()

	fmt.Printf("%s Checking for updates...\n", cyan("ℹ"))

	currentVer := parseVersion(VerBranch, VerStatus, VerNumber)
	buildChannel := "production"
	if strings.ToLower(strings.TrimSuffix(currentVer.Branch, ".")) == "dev" {
		buildChannel = "development"
	}
	fmt.Printf("%s Current version (%s): %s %s %s\n", cyan("▸"), buildChannel, currentVer.Branch, currentVer.Status, currentVer.Number)

	versionURL := "https://zusty.codeberg.page/Zoi/@app/version.json"
	fmt.Printf("%s Fetching version info from: %s\n", cyan("▸"), versionURL)
	client := http.Client{Timeout: 15 * time.Second}
	resp, err := client.Get(versionURL)
	if err != nil {
		fmt.Printf("%s Failed to fetch version info: %v\n", red("✗"), err)
		return
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		fmt.Printf("%s Failed to fetch version info: Received status code %d\n", red("✗"), resp.StatusCode)
		return
	}

	var remoteVersionInfo VersionInfo
	if err := json.NewDecoder(resp.Body).Decode(&remoteVersionInfo); err != nil {
		fmt.Printf("%s Failed to parse version info: %v\n", red("✗"), err)
		return
	}

	var latestVer Version
	var latestBranch string
	if buildChannel == "development" {
		latestBranch = "Dev."
		latestVer = parseVersion(latestBranch, remoteVersionInfo.Latest.Development.Status, remoteVersionInfo.Latest.Development.Version)
	} else {
		latestBranch = ""
		latestVer = parseVersion(latestBranch, remoteVersionInfo.Latest.Production.Status, remoteVersionInfo.Latest.Production.Version)
	}
	fmt.Printf("%s Latest available version (%s): %s %s %s\n", cyan("▸"), buildChannel, latestVer.Branch, latestVer.Status, latestVer.Number)

	comparisonResult := currentVer.Compare(latestVer)

	if comparisonResult < 1 {
		fmt.Printf("%s You are already running the latest version for the '%s' channel.\n", green("✓"), buildChannel)
		return
	}

	fmt.Printf("%s A new version is available: %s -> %s\n",
		yellow("!"),
		yellow(fmt.Sprintf("%s %s %s", currentVer.Branch, currentVer.Status, currentVer.Number)),
		green(fmt.Sprintf("%s %s %s", latestVer.Branch, latestVer.Status, latestVer.Number)),
	)

	if !confirmPrompt("Would you like to update now?") {
		fmt.Printf("%s Update cancelled by user.\n", yellow("!"))
		return
	}

	fmt.Printf("\n%s Starting update process...\n", cyan("ℹ"))
	var installCmd string
	var installURL string
	if runtime.GOOS == "windows" {
		installURL = "https://zusty.codeberg.page/Zoi/@app/install.ps1"
		installCmd = "irm 'https://zusty.codeberg.page/Zoi/@app/install.ps1' | iex"
	} else {
		installURL = "https://zusty.codeberg.page/Zoi/@app/install.sh"
		installCmd = "curl -fsSL https://zusty.codeberg.page/Zoi/@app/install.sh | bash"
	}

	fmt.Printf("%s Running install script from: %s\n", cyan("▸"), installURL)
	err = executeCommand(installCmd)
	if err != nil {
		fmt.Printf("%s Update failed during script execution: %v\n", red("✗"), err)
		fmt.Printf("%s Command attempted: %s\n", red("↪"), installCmd)
		return
	}

	fmt.Printf("\n%s Update process completed. Please restart Zoi to use the new version.\n", green("✓"))
	fmt.Printf("%s You might need to restart your terminal session for PATH changes to take effect.\n", yellow("!"))
	fmt.Printf("%s Expected version after update: %s\n", cyan("ℹ"), green(fmt.Sprintf("%s %s %s", latestVer.Branch, latestVer.Status, latestVer.Number)))
}
