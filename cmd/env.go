package cmd

import (
	"fmt"
	"zoi/src"

	"github.com/spf13/cobra"
)

var envCmd = &cobra.Command{
	Use:   "env [environment]",
	Short: "Display info, check packages, and run setup for an environment",
	Args:  cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		envName := args[0]
		yellow := src.Yellow()

		config, err := src.LoadProjectConfig()
		if err != nil {
			src.PrintError("%v", err)
			return
		}

		var targetEnv *src.EnvSpec
		for i, e := range config.Environments {
			if e.Cmd == envName {
				targetEnv = &config.Environments[i]
				break
			}
		}

		if targetEnv == nil {
			src.PrintError("Environment '%s' not found in zoi.yaml", envName)
			return
		}

		src.PrintHighlight("--- Project Information ---")
		fmt.Printf("Name: %s\n", yellow.Sprint(config.Name))
		fmt.Printf("Environment: %s\n\n", yellow.Sprint(targetEnv.Name))

		src.PrintHighlight("--- Checking Packages ---")
		allPackagesFound := true
		for _, pkg := range config.Packages {
			fmt.Printf("Checking for %s... ", yellow.Sprint(pkg.Name))
			if src.CheckCommand(pkg.Check) {
				src.PrintSuccess("OK")
			} else {
				src.PrintError("MISSING")
				allPackagesFound = false
			}
		}

		if !allPackagesFound {
			src.PrintError("\nSome required packages are missing. Please install them and try again.")
			return
		}

		fmt.Println()
		src.PrintHighlight("--- Running Setup Commands ---")
		for _, runCmd := range targetEnv.Run {
			src.PrintInfo("-> %s", runCmd)
			if err := src.ExecuteCommand(runCmd); err != nil {
				src.PrintError("Command failed with error: %v", err)
				src.PrintError("Halting environment setup.")
				return
			}
		}

		fmt.Println()
		src.PrintSuccess("Environment '%s' is ready!", targetEnv.Name)
	},
}

func init() {
	rootCmd.AddCommand(envCmd)
}
