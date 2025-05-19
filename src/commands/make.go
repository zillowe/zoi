package commands

import (
	"fmt"
	"os"
	"runtime"
	"strings"

	"github.com/fatih/color"
	"gopkg.in/yaml.v3"
)

func MakeCommand(yamlFile string) {
	red := color.New(color.FgRed).SprintFunc()
	green := color.New(color.FgGreen).SprintFunc()
	cyan := color.New(color.FgCyan).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()

	data, err := os.ReadFile(yamlFile)
	if err != nil {
		fmt.Printf("%s Failed to read YAML file '%s': %v\n", red("✗"), yamlFile, err)
		return
	}

	var config YamlConfig
	if err := yaml.Unmarshal(data, &config); err != nil {
		fmt.Printf("%s Invalid YAML format in '%s': %v\n", red("✗"), yamlFile, err)
		return
	}

	if config.AppName == "" {
		fmt.Printf("%s Missing 'appName' in YAML file '%s'\n", red("✗"), yamlFile)
		return
	}
	appName := config.AppName
	fmt.Printf("%s Processing YAML config for app: %s\n", cyan("ℹ"), appName)

	for _, pkg := range config.Packages {
		fmt.Printf("\n%s Checking package: %s...\n", cyan("ℹ"), pkg.Name)
		if pkg.Check == "" {
			fmt.Printf("%s No check command defined for %s, attempting install...\n", yellow("!"), pkg.Name)
		} else {
			fmt.Printf("%s Running check: '%s'\n", cyan("▸"), pkg.Check)
			output, err := runCommandOutput(pkg.Check)
			if err == nil {
				fmt.Printf("%s %s already installed.\n", green("✓"), pkg.Name)
				fmt.Printf("%s Output: %s\n", cyan("↪"), strings.TrimSpace(output))
				continue
			} else {
				fmt.Printf("%s %s not found or check failed: %v\n", yellow("!"), pkg.Name, err)
				fmt.Printf("%s Output: %s\n", cyan("↪"), strings.TrimSpace(output))
			}
		}

		var installCmd string
		os := runtime.GOOS
		switch os {
		case "linux":
			installCmd = pkg.Install.Linux
		case "windows":
			installCmd = pkg.Install.Windows
		case "darwin":
			installCmd = pkg.Install.Darwin
		}

		if installCmd == "" {
			installCmd = pkg.Install.Default
		}

		if installCmd == "" {
			fmt.Printf("%s No install command found for %s on %s in YAML. Cannot proceed.\n", red("✗"), pkg.Name, os)
			return
		}

		fmt.Printf("%s Attempting to install %s using command: '%s'\n", cyan("ℹ"), pkg.Name, installCmd)
		if !confirmPrompt(fmt.Sprintf("Install package '%s' from YAML?", pkg.Name)) {
			fmt.Printf("%s Installation skipped by user.\n", yellow("!"))
			return
		}

		if err := executeCommand(installCmd); err != nil {
			fmt.Printf("%s Failed to install %s: %v\n", red("✗"), pkg.Name, err)
			return
		}

		fmt.Printf("%s Successfully installed %s.\n", green("✓"), pkg.Name)

		if pkg.Check != "" {
			fmt.Printf("%s Verifying installation: '%s'\n", cyan("▸"), pkg.Check)
			if err := executeCommand(pkg.Check); err != nil {
				fmt.Printf("%s Verification failed after installing %s: %v\n", yellow("!"), pkg.Name, err)
			} else {
				fmt.Printf("%s Installation verified.\n", green("✓"))
			}
		}
	}

	if config.CreateCommand == "" {
		fmt.Printf("%s No 'createCommand' defined in YAML file '%s'. Nothing more to do.\n", yellow("!"), yamlFile)
	} else {
		fmt.Printf("\n%s Creating application '%s' using command from YAML: '%s'\n", cyan("ℹ"), appName, config.CreateCommand)
		createCmd := strings.ReplaceAll(config.CreateCommand, "${appName}", appName)
		if err := executeCommand(createCmd); err != nil {
			fmt.Printf("%s Failed to create application '%s' using YAML command: %v\n", red("✗"), appName, err)
			return
		}
		fmt.Printf("%s Successfully created application '%s' using YAML config.\n", green("✓"), appName)
	}
}
