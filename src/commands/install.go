package commands

import (
	"fmt"
	"os/exec"
	"regexp"
	"strings"

	"github.com/fatih/color"
)

func InstallCommand(pkg string) {
	red := color.New(color.FgRed).SprintFunc()
	green := color.New(color.FgGreen).SprintFunc()
	cyan := color.New(color.FgCyan).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()

	parts := strings.Split(pkg, "@")
	pkgName := parts[0]
	wantVersion := ""
	if len(parts) > 1 {
		wantVersion = parts[1]
	}
	fmt.Printf("%s Attempting to install %s%s\n", cyan("ℹ"), cyan(pkgName), func() string {
		if wantVersion != "" {
			return cyan("@"+wantVersion)
		}
		return ""
	}())

	fmt.Printf("%s Checking if '%s' is directly available in PATH and getting version...\n", cyan("ℹ"), pkgName)
	directVersion, directErr := checkToolDirectly(pkgName)
	pkgPath := ""
	if directErr == nil {
		fmt.Printf("%s Found '%s' in PATH, version: %s\n", green("✓"), pkgName, directVersion)
		var errPath error
		pkgPath, errPath = exec.LookPath(pkgName)
		if errPath != nil {
			pkgPath = "(path lookup failed)"
		}
	} else {
		fmt.Printf("%s Tool '%s' not found directly in PATH or version check failed: %v\n", yellow("!"), pkgName, directErr)
	}

	fmt.Printf("%s Detecting system package manager...\n", cyan("ℹ"))
	pm, pmErr := detectPackageManager()
	pmInstalled := false
	if pmErr != nil {
		fmt.Printf("%s Could not detect a supported package manager: %v\n", yellow("!"), pmErr)
		if directErr != nil {
			fmt.Printf("%s Cannot automatically install '%s'. Please install it manually.\n", red("✗"), pkgName)
			return
		}
		fmt.Printf("%s Will rely on the version found directly in PATH.\n", yellow("!"))
	} else {
		fmt.Printf("%s Detected: %s\n", green("✓"), pm.Name)
		fmt.Printf("%s Checking if '%s' is managed by %s...\n", cyan("ℹ"), pkgName, pm.Name)
		pmInstalled = isInstalled(pm, pkgName)
		if pmInstalled {
			fmt.Printf("%s Package manager %s reports that '%s' is managed.\n", green("✓"), pm.Name, pkgName)
		} else {
			fmt.Printf("%s Package manager %s does not report managing '%s'.\n", yellow("!"), pm.Name, pkgName)
		}
	}

	proceedWithInstall := false
	installReason := ""

	if directErr == nil { 
		source := fmt.Sprintf("PATH (%s)", pkgPath)
		satisfied := false
		if wantVersion == "" { 
			satisfied = true
			fmt.Printf("%s %s@%s is already available and satisfies the request (Found via %s)\n",
				green("✓"), cyan(pkgName), green(directVersion), cyan(source))
		} else { 
			if versionMatches(wantVersion, directVersion) {
				satisfied = true
				fmt.Printf("%s %s@%s is already available and satisfies requested version %s (Found via %s)\n",
					green("✓"), cyan(pkgName), green(directVersion), cyan("@"+wantVersion), cyan(source))
			} else {
				satisfied = false
				fmt.Printf("%s %s@%s is available (Found via %s), but does not match requested version %s\n",
					yellow("!"), cyan(pkgName), yellow(directVersion), cyan(source), cyan("@"+wantVersion))
				reinstallPrompt := fmt.Sprintf("Attempt to install requested version %s@%s anyway using %s?", pkgName, wantVersion, pm.Name)
				if pmErr == nil && confirmPrompt(reinstallPrompt) {
					proceedWithInstall = true
					installReason = fmt.Sprintf("User chose to install requested version %s over existing %s.", wantVersion, directVersion)
				} else if pmErr != nil {
                     fmt.Printf("%s Cannot attempt installation as no supported package manager was found.\n", red("✗"))
                } else {
					fmt.Printf("%s Installation cancelled. Please manage existing installations manually.\n", yellow("!"))
					return
				}
			}
		}
		if satisfied {
			if pmErr == nil && !pmInstalled {
				fmt.Printf("%s Note: %s thinks '%s' is installed, but %s does not report managing it.\n", yellow("!"), source, pkgName, pm.Name)
			}
			if !proceedWithInstall { 
                 return 
            }
		}
	} else {
		if pmErr == nil { 
			if pmInstalled { 
				fmt.Printf("%s Tool '%s' not found directly, but package manager '%s' reports it installed.\n", yellow("!"), pkgName, pm.Name)
				fmt.Printf("%s This might indicate a broken installation or PATH issue.\n", yellow("!"))
				promptMsg := fmt.Sprintf("Attempt to (re)install %s%s using %s?", pkgName, func() string {
					if wantVersion != "" { return "@"+wantVersion }
					return ""
                }(), pm.Name)
				if confirmPrompt(promptMsg) {
					proceedWithInstall = true
					installReason = fmt.Sprintf("Tool not working directly, user chose to (re)install via %s.", pm.Name)
				} else {
					fmt.Printf("%s Installation cancelled.\n", yellow("!"))
					return 
				}
			} else { 
				fmt.Printf("%s Tool '%s' not found via PATH or package manager '%s'.\n", yellow("!"), pkgName, pm.Name)
				proceedWithInstall = true 
				installReason = fmt.Sprintf("Tool not found, proceeding with installation via %s.", pm.Name)
			}
		} else {
			fmt.Printf("%s Cannot find '%s' and no supported package manager detected. Please install manually.\n", red("✗"), pkgName)
			return 
		}
	}

	if proceedWithInstall {
		if pmErr != nil {
			fmt.Printf("%s Cannot proceed with installation: No supported package manager detected.\n", red("✗"))
			return
		}

		fmt.Printf("%s %s\n", cyan("ℹ"), installReason)

		installPkgArg := formatInstallArg(pm, pkgName, wantVersion)
		if wantVersion != "" && !strings.Contains(installPkgArg, wantVersion) { 
			fmt.Printf("%s Warning: Could not format specific version '%s' for package manager '%s'. Attempting install using '%s'.\n", yellow("!"), wantVersion, pm.Name, installPkgArg)
		} else if wantVersion != "" {
			fmt.Printf("%s Using formatted package argument for %s: '%s'\n", cyan("ℹ"), pm.Name, installPkgArg)
		}

		installCmd := fmt.Sprintf(pm.InstallCmd, installPkgArg)

		fmt.Printf("\n%s The following command will be run:\n", cyan("ℹ"))
		fmt.Printf("  %s\n", cyan(installCmd))
		if !confirmPrompt("Proceed with installation?") {
			fmt.Printf("%s Installation cancelled by user.\n", yellow("!"))
			return
		}

		fmt.Printf("\n%s Installing %s via %s...\n", cyan("ℹ"), installPkgArg, pm.Name)
		if err := executeCommand(installCmd); err != nil {
			fmt.Printf("%s Installation command failed: %v\n", red("✗"), err)
		} else {
			fmt.Printf("%s Installation command executed successfully.\n", green("✓")) 
		}


		fmt.Printf("%s Verifying installation after attempt...\n", cyan("ℹ"))
		finalVersion, finalErr := checkToolDirectly(pkgName)
		if finalErr == nil {
			fmt.Printf("%s Verification successful. Found '%s' version: %s\n", green("✓"), pkgName, finalVersion)
			if wantVersion != "" && !versionMatches(wantVersion, finalVersion) {
				fmt.Printf("%s Warning: Installed version '%s' does not match requested version '%s'.\n", yellow("!"), finalVersion, wantVersion)
			}
		} else {
			fmt.Printf("%s Verification failed after install attempt: %v\n", red("✗"), finalErr)
			fmt.Printf("%s Please check the installation manually.\n", yellow("!"))
		}
	} else {
        fmt.Printf("%s No installation action performed.\n", cyan("ℹ"))
    }
}

func formatInstallArg(pm PackageManager, pkgName, version string) string {
	if version == "" {
		return pkgName
	}
	switch pm.Name {
	case "apt", "dpkg":
		return fmt.Sprintf("%s=%s", pkgName, version)
	case "dnf", "yum":
		return fmt.Sprintf("%s-%s", pkgName, version)
	case "pacman":
		return fmt.Sprintf("%s=%s", pkgName, version)
	case "apk":
		return fmt.Sprintf("%s=%s", pkgName, version)
	case "brew":
		if regexp.MustCompile(`^\d+\.\d+$`).MatchString(version) {
			return fmt.Sprintf("%s@%s", pkgName, version)
		}
		return pkgName
	case "scoop":
		return fmt.Sprintf("%s@%s", pkgName, version)
	default:
		return fmt.Sprintf("%s=%s", pkgName, version)
	}
}

func isInstalled(pm PackageManager, pkg string) bool {
	if pm.CheckCommand == "" {
		return false
	}
	checkCmdStr := fmt.Sprintf(pm.CheckCommand, pkg)
	_, err := runCommandOutput(checkCmdStr)
	return err == nil
}