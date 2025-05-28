package commands

import (
	"fmt"
	"strings"

	"github.com/fatih/color"
)

func UninstallCommand(pkg string) {
	red := color.New(color.FgRed).SprintFunc()
	green := color.New(color.FgGreen).SprintFunc()
	cyan := color.New(color.FgCyan).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()

	pkgName := strings.Split(pkg, "@")[0]

	fmt.Printf("%s Attempting to uninstall %s\n", cyan("ℹ"), cyan(pkgName))

	fmt.Printf("%s Detecting system package manager...\n", cyan("ℹ"))
	pm, pmErr := detectPackageManager()
	if pmErr != nil {
		fmt.Printf("%s Could not detect a supported package manager: %v\n", yellow("!"), pmErr)
		fmt.Printf("%s Cannot automatically uninstall '%s'. Please uninstall it manually.\n", red("✗"), pkgName)
		return
	}
	fmt.Printf("%s Detected: %s\n", green("✓"), pm.Name)

	if pm.UninstallCmd == "" {
		fmt.Printf("%s Uninstall command not defined for package manager: %s\n", red("✗"), pm.Name)
		fmt.Printf("%s Cannot automatically uninstall '%s'.\n", red("✗"), pkgName)
		return
	}

	fmt.Printf("%s Checking if '%s' is managed by %s...\n", cyan("ℹ"), pkgName, pm.Name)
	pmInstalled := isInstalled(pm, pkgName)
	if !pmInstalled {
		fmt.Printf("%s Package manager %s does not report managing '%s'.\n", yellow("!"), pm.Name, pkgName)
		fmt.Printf("%s Will attempt uninstallation anyway.\n", yellow("!"))
	}

	uninstallCmdStr := fmt.Sprintf(pm.UninstallCmd, pkgName)

	fmt.Printf("\n%s The following command will be run:\n", cyan("ℹ"))
	fmt.Printf("  %s\n", cyan(uninstallCmdStr))
	if !confirmPrompt(fmt.Sprintf("Proceed with uninstallation of %s?", pkgName)) {
		fmt.Printf("%s Uninstallation cancelled by user.\n", yellow("!"))
		return
	}

	fmt.Printf("\n%s Uninstalling %s via %s...\n", cyan("ℹ"), pkgName, pm.Name)
	if err := executeCommand(uninstallCmdStr); err != nil {
		fmt.Printf("%s Uninstallation command failed: %v\n", red("✗"), err)
	} else {
		fmt.Printf("%s Uninstallation command executed successfully for '%s'.\n", green("✓"), pkgName)
	}

	fmt.Printf("\n%s Verifying uninstallation of '%s'...\n", cyan("ℹ"), pkgName)
	if isInstalled(pm, pkgName) {
		fmt.Printf("%s Package manager %s still reports '%s' as managed. Uninstallation might have failed or requires additional steps.\n", yellow("!"), pm.Name, pkgName)
	} else {
		_, directErr := checkToolDirectly(pkgName)
		if directErr == nil {
			fmt.Printf("%s '%s' seems to have been uninstalled by %s, but it's still found in PATH.\n", yellow("!"), pkgName, pm.Name)
			fmt.Printf("%s This could be a leftover or a different installation. You might need to remove it manually from your PATH or filesystem.\n", yellow("!"))
		} else {
			fmt.Printf("%s '%s' seems to have been successfully uninstalled by %s and is no longer directly found.\n", green("✓"), pkgName, pm.Name)
		}
	}
}
