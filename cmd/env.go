package cmd

import (
	"errors"
	"fmt"
	"zoi/src"

	"github.com/manifoldco/promptui"
	"github.com/spf13/cobra"
)

var envCmd = &cobra.Command{
	Use:   "env [environment]",
	Short: "Display info, check packages, and run setup for an environment",
	Long: `Display info, check packages, and run setup for a project environment.
If no environment is specified, it will launch an interactive prompt to choose one.`,
	Args: cobra.MaximumNArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		config, err := src.LoadProjectConfig()
		if err != nil {
			src.PrintError("%v", err)
			return
		}

		if len(config.Environments) == 0 {
			src.PrintError("No environments are defined in your zoi.yaml file.")
			return
		}

		if len(args) == 1 {
			envName := args[0]
			setupEnvironment(envName, config)
			return
		}

		envCmds := make([]string, len(config.Environments))
		for i, e := range config.Environments {
			envCmds[i] = e.Cmd
		}

		templates := &promptui.SelectTemplates{
			Label:    "{{ . }}",
			Active:   `{{ "›" | green | bold }} {{ . | green | bold }}`,
			Inactive: "  {{ . | faint }}",
			Selected: `{{ "✔" | green | bold }} {{ "Selected environment:" | bold }} {{ . | yellow }}`,
		}

		prompt := promptui.Select{
			Label:     "Select an environment to set up",
			Items:     envCmds,
			Templates: templates,
		}

		_, selectedEnv, err := prompt.Run()
		if err != nil {
			if errors.Is(err, promptui.ErrInterrupt) {
				src.PrintInfo("Environment selection cancelled.")
			}
			return
		}

		setupEnvironment(selectedEnv, config)
	},
}

func setupEnvironment(envName string, config *src.ProjectConfig) {
	yellow := src.Yellow()

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
}

func init() {
	rootCmd.AddCommand(envCmd)
}
