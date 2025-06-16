package cmd

import (
	"fmt"
	"strings"
	"zoi/src"

	"github.com/spf13/cobra"
	"github.com/spf13/viper"
)

var makeCmd = &cobra.Command{
	Use:   "make [recipe-file.yaml]",
	Short: "Generate an application from a local YAML recipe file",
	Args:  cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		recipeFile := args[0]
		yellow := src.Yellow()

		config, err := src.LoadMakerConfig(recipeFile)
		if err != nil {
			src.PrintError("%v", err)
			return
		}

		src.PrintHighlight("--- Starting application creation for: %s ---", yellow.Sprint(config.AppName))
		fmt.Println()

		src.PrintHighlight("--- Checking Dependencies ---")
		allPackagesOk := true
		for _, pkg := range config.Packages {
			fmt.Printf("Checking for %s... ", yellow.Sprint(pkg.Name))
			if src.CheckCommand(pkg.Check) {
				src.PrintSuccess("OK")
				continue
			}

			src.PrintError("MISSING")
			src.PrintInfo("--> Installing %s...", pkg.Name)

			err := installPackageFromRecipe(pkg)
			if err != nil {
				src.PrintError("Installation failed for %s: %v", pkg.Name, err)
				allPackagesOk = false
				break
			}
		}

		if !allPackagesOk {
			src.PrintError("\nDependency installation failed. Halting application creation.")
			return
		}

		fmt.Println()

		src.PrintHighlight("--- Creating Application ---")
		finalCommand := strings.ReplaceAll(config.CreateCommand, "${appName}", config.AppName)
		src.PrintInfo("-> %s", finalCommand)

		if err := src.ExecuteCommand(finalCommand); err != nil {
			src.PrintError("Application creation command failed: %v", err)
			return
		}

		fmt.Println()
		src.PrintSuccess("Application '%s' created successfully!", config.AppName)
	},
}

func installPackageFromRecipe(pkg src.MakerPackage) error {
	switch installCmd := pkg.Install.(type) {
	case string:
		return src.ExecuteCommand(installCmd)

	case map[interface{}]interface{}:
		pkgManager := viper.GetString("pkgManager")
		if pkgManager == "" {
			return fmt.Errorf("package manager not configured in Zoi. Please run 'zoi init'")
		}

		packageName, ok := installCmd[pkgManager]
		if !ok {
			return fmt.Errorf("no install entry found for your package manager '%s'", pkgManager)
		}

		packageNameStr, ok := packageName.(string)
		if !ok {
			return fmt.Errorf("invalid package name format for manager '%s'", pkgManager)
		}

		return src.InstallPackage(pkgManager, packageNameStr)

	default:
		return fmt.Errorf("unknown type for 'install' directive in package %s", pkg.Name)
	}
}

func init() {
	rootCmd.AddCommand(makeCmd)
}
