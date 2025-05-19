package commands

import (
	"fmt"
	"os"
	"strings"

	"github.com/fatih/color"
	"gopkg.in/yaml.v3"
)

const zoiYAMLFile = "zoi.yaml"

func EnvCommand(args []string) {
	cyan := color.New(color.FgCyan).SprintFunc()
	green := color.New(color.FgGreen).SprintFunc()
	red := color.New(color.FgRed).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()

	data, err := os.ReadFile(zoiYAMLFile)
	if err != nil {
		if os.IsNotExist(err) {
			fmt.Printf("%s File '%s' not found in the current directory.\n", red("✗"), zoiYAMLFile)
			return
		}
		fmt.Printf("%s Failed to read YAML file '%s': %v\n", red("✗"), zoiYAMLFile, err)
		return
	}

	var config ZoiFileConfig
	if err := yaml.Unmarshal(data, &config); err != nil {
		fmt.Printf("%s Invalid YAML format in '%s': %v\n", red("✗"), zoiYAMLFile, err)
		return
	}

	if len(args) == 0 {
		fmt.Printf("%s Project: %s\n", cyan("🗂"), green(config.Name))
		if len(config.Packages) > 0 {
			fmt.Println(cyan("\n🔧 Global Packages:"))
			for _, pkg := range config.Packages {
				checkAndPrintPackageVersion(pkg.Name, pkg.Check)
			}
		} else {
			fmt.Println(yellow("  No global packages defined in zoi.yaml."))
		}
	} else {
		envCmd := args[0]
		var targetEnv *EnvironmentSpec
		for i, envSpec := range config.Environments {
			if envSpec.Cmd == envCmd {
				targetEnv = &config.Environments[i]
				break
			}
		}

		if targetEnv == nil {
			fmt.Printf("%s Environment command '%s' not found in '%s'.\n", red("✗"), envCmd, zoiYAMLFile)
			fmt.Println(yellow("ℹ Available environment commands:"))
			for _, envSpec := range config.Environments {
				fmt.Printf("  - %s (Environment: %s)\n", envSpec.Cmd, envSpec.Name)
			}
			return
		}

		fmt.Printf("%s Activating environment: %s\n", cyan("🚀"), green(targetEnv.Name))

		packagesToCheck := make(map[string]string)

		for _, pkg := range config.Packages {
			packagesToCheck[pkg.Name] = pkg.Check
		}
		for _, pkgName := range targetEnv.Check {
			foundGlobal := false
			for _, gp := range config.Packages {
				if gp.Name == pkgName {
					packagesToCheck[pkgName] = gp.Check
					foundGlobal = true
					break
				}
			}
			if !foundGlobal {
				packagesToCheck[pkgName] = ""
			}
		}

		if len(packagesToCheck) > 0 {
			fmt.Println(cyan("\n🔧 Packages Health Check:"))
			for pkgName, checkCmd := range packagesToCheck {
				checkAndPrintPackageVersion(pkgName, checkCmd)
			}
		} else {
			fmt.Println(yellow("  No packages specified for health check in this environment or globally."))
		}

		if len(targetEnv.Run) > 0 {
			fmt.Println(cyan("\n⚙️ Running setup commands:"))
			for i, cmdToRun := range targetEnv.Run {
				fmt.Printf("%s [%d/%d] Running: %s\n", cyan("▸"), i+1, len(targetEnv.Run), cmdToRun)
				output, errCmd := runCommandOutput(cmdToRun)
				trimmedOutput := strings.TrimSpace(output)
				if errCmd != nil {
					fmt.Printf("%s Error: %v\n", red("✗"), errCmd)
					if trimmedOutput != "" {
						fmt.Printf("Output:\n%s\n", trimmedOutput)
					}
				} else {
					if trimmedOutput != "" {
						fmt.Println(trimmedOutput)
					}
					fmt.Printf("%s Command finished successfully.\n", green("✓"))
				}
			}
		} else {
			fmt.Println(yellow("  No setup commands to run for this environment."))
		}
	}
}

func checkAndPrintPackageVersion(toolName, checkCommand string) {
	cyan := color.New(color.FgCyan).SprintFunc()
	green := color.New(color.FgGreen).SprintFunc()
	red := color.New(color.FgRed).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()

	fmt.Printf("  %s Checking %s... ", cyan("▪"), toolName)

	var version string
	var errVal error

	if checkCommand != "" {
		rawOutput, cmdErr := runCommandOutput(checkCommand)
		if cmdErr != nil {
			errVal = fmt.Errorf("check command '%s' failed: %w", checkCommand, cmdErr)
		} else {
			version = extractVersion(rawOutput)
			if version == "" {
				trimmedRaw := strings.TrimSpace(rawOutput)
				if len(trimmedRaw) > 50 {
					version = "unknown (output: " + trimmedRaw[:50] + "...)"
				} else if trimmedRaw != "" {
					version = "unknown (output: " + trimmedRaw + ")"
				} else {
					version = "unknown (empty output from check command)"
				}
			}
		}
	} else {
		version, errVal = checkToolDirectly(toolName)
	}

	if errVal != nil {
		fmt.Printf("%s Not found or error: %v\n", red("✗"), errVal)
	} else if strings.HasPrefix(version, "unknown") || version == "" {
		fmt.Printf("%s Installed (%s)\n", yellow("?"), version)
	} else {
		fmt.Printf("%s Version %s\n", green("✓"), green(version))
	}
}
