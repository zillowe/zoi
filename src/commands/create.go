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

func CreateCommand(app string, appName string) {
	red := color.New(color.FgRed).SprintFunc()
	green := color.New(color.FgGreen).SprintFunc()
	cyan := color.New(color.FgCyan).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()

	fmt.Printf("%s Fetching app definitions...\n", cyan("ℹ"))
	appsURL := getAppsURL()
	client := http.Client{Timeout: 30 * time.Second}
	resp, err := client.Get(appsURL)
	if err != nil {
		fmt.Printf("%s Failed to fetch app definitions from %s: %v\n", red("✗"), appsURL, err)
		return
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		fmt.Printf("%s Failed to fetch app definitions: Received status code %d from %s\n", red("✗"), resp.StatusCode, appsURL)
		return
	}

	var apps map[string]AppDefinition
	if err := json.NewDecoder(resp.Body).Decode(&apps); err != nil {
		fmt.Printf("%s Invalid app definitions format in %s: %v\n", red("✗"), appsURL, err)
		return
	}

	appDef, exists := apps[app]
	if !exists {
		fmt.Printf("%s Application template '%s' not found in %s\n", red("✗"), app, appsURL)
		return
	}

	fmt.Printf("%s Found template: %s\n", green("✓"), appDef.Name)

	for depName, dep := range appDef.Dependencies {
		fmt.Printf("\n%s Checking dependency: %s...\n", cyan("ℹ"), depName)
		if dep.Check == "" {
			fmt.Printf("%s No check command defined for %s, attempting install...\n", yellow("!"), depName)
		} else {
			fmt.Printf("%s Running check: '%s'\n", cyan("▸"), dep.Check)
			output, err := runCommandOutput(dep.Check)
			if err == nil {
				fmt.Printf("%s %s already installed.\n", green("✓"), depName)
				fmt.Printf("%s Output: %s\n", cyan("↪"), strings.TrimSpace(output))
				continue
			} else {
				fmt.Printf("%s %s not found or check failed: %v\n", yellow("!"), depName, err)
				fmt.Printf("%s Output: %s\n", cyan("↪"), strings.TrimSpace(output))
			}
		}

		var installCmd string
		os := runtime.GOOS
		distro := getLinuxDistro()

		switch os {
		case "linux":
			installCmd = dep.Install.Linux[distro]
			if installCmd == "" {
				installCmd = dep.Install.Linux["default"]
			}
		case "darwin":
			installCmd = dep.Install.Darwin
		case "windows":
			installCmd = dep.Install.Win32
		}

		if installCmd == "" {
			installCmd = dep.Install.Default
		}

		if installCmd == "" {
			fmt.Printf("%s No install command found for %s on %s/%s. Cannot proceed.\n", red("✗"), depName, os, distro)
			return
		}

		fmt.Printf("%s Attempting to install %s using command: '%s'\n", cyan("ℹ"), depName, installCmd)
		if !confirmPrompt(fmt.Sprintf("Install dependency '%s'?", depName)) {
			fmt.Printf("%s Installation skipped by user.\n", yellow("!"))
			return
		}

		if err := executeCommand(installCmd); err != nil {
			fmt.Printf("%s Failed to install %s: %v\n", red("✗"), depName, err)
			return
		}

		fmt.Printf("%s Successfully installed %s.\n", green("✓"), depName)

		if dep.Check != "" {
			fmt.Printf("%s Verifying installation: '%s'\n", cyan("▸"), dep.Check)
			if err := executeCommand(dep.Check); err != nil {
				fmt.Printf("%s Verification failed after installing %s: %v\n", yellow("!"), depName, err)
			} else {
				fmt.Printf("%s Installation verified.\n", green("✓"))
			}
		}
	}

	fmt.Printf("\n%s Creating application '%s' using command: '%s'\n", cyan("ℹ"), appName, appDef.Create)
	createCmd := strings.ReplaceAll(appDef.Create, "${appName}", appName)
	if err := executeCommand(createCmd); err != nil {
		fmt.Printf("%s Failed to create application '%s': %v\n", red("✗"), appName, err)
		return
	}
	fmt.Printf("%s Successfully created application '%s'.\n", green("✓"), appName)
}
