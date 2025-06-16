package cmd

import (
	"fmt"
	"strings"
	"zoi/src"

	"github.com/spf13/cobra"
	"github.com/spf13/viper"
)

var createCmd = &cobra.Command{
	Use:   "create [app-template] [new-project-name]",
	Short: "Creates a new project from a remote application template",
	Args:  cobra.ExactArgs(2),
	Run: func(cmd *cobra.Command, args []string) {
		appTemplateName := args[0]
		newProjectName := args[1]
		yellow := src.Yellow()

		src.PrintInfo("Fetching available application templates...")
		apps, err := src.GetApps()
		if err != nil {
			src.PrintError("Error fetching apps: %v", err)
			return
		}

		app, ok := apps[appTemplateName]
		if !ok {
			src.PrintError("Application template not found: %s", appTemplateName)
			return
		}

		fmt.Println()
		src.PrintHighlight("--- Preparing template: %s ---", yellow.Sprint(appTemplateName))

		pkgManager := viper.GetString("pkgManager")

		for pkg, details := range app.Packages {
			fmt.Printf("Checking for package %s... ", yellow.Sprint(pkg))
			if src.CheckCommand(details.CheckCmd) {
				src.PrintSuccess("OK")
				continue
			}
			src.PrintError("MISSING")

			var pkgToInstall string
			switch pkgManager {
			case "pacman":
				pkgToInstall = details.Pacman
			case "apt":
				pkgToInstall = details.Apt
			case "scoop":
				pkgToInstall = details.Scoop
			case "brew":
				pkgToInstall = details.Brew
			case "yum":
				pkgToInstall = details.Brew
			case "dnf":
				pkgToInstall = details.Brew
			case "apk":
				pkgToInstall = details.Brew
			default:
				src.PrintError("Package '%s' does not have an install command for your package manager ('%s').", pkg, pkgManager)
				return
			}
			if err := src.InstallPackage(pkgManager, pkgToInstall); err != nil {
				src.PrintError("Error installing package %s: %s", pkg, err)
				return
			}
		}

		fmt.Printf("Checking for application %s... ", yellow.Sprint(appTemplateName))
		if !src.CheckCommand(app.CheckCmd) {
			src.PrintError("MISSING")
			if err := src.InstallApp(app.InstallCmd); err != nil {
				src.PrintError("Error installing app %s: %s", appTemplateName, err)
				return
			}
		} else {
			src.PrintSuccess("OK")
		}

		fmt.Println()

		if app.CreateCommand == "" {
			src.PrintSuccess("Template '%s' is ready. No 'createCommand' defined.", appTemplateName)
			return
		}

		src.PrintHighlight("--- Creating Application: %s ---", yellow.Sprint(newProjectName))
		finalCommand := strings.ReplaceAll(app.CreateCommand, "${appName}", newProjectName)
		src.PrintInfo("-> %s", finalCommand)

		if err := src.ExecuteCommand(finalCommand); err != nil {
			src.PrintError("Application creation command failed: %v", err)
			return
		}

		fmt.Println()
		src.PrintSuccess("Application '%s' created successfully!", newProjectName)
	},
}

func init() {
	rootCmd.AddCommand(createCmd)
}
