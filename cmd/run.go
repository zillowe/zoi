package cmd

import (
	"errors"
	"zoi/src"

	"github.com/manifoldco/promptui"
	"github.com/spf13/cobra"
)

var runCmd = &cobra.Command{
	Use:   "run [command]",
	Short: "Execute a command defined in a local zoi.yaml file",
	Long: `Execute a command defined in a local zoi.yaml file.
If no command is specified, it will launch an interactive prompt to choose one.`,
	Args: cobra.MaximumNArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		config, err := src.LoadProjectConfig()
		if err != nil {
			src.PrintError("%v", err)
			return
		}

		if len(config.Commands) == 0 {
			src.PrintError("No commands are defined in your zoi.yaml file.")
			return
		}

		if len(args) == 1 {
			commandName := args[0]
			executeProjectCommand(commandName, config)
			return
		}

		commandNames := make([]string, len(config.Commands))
		for i, c := range config.Commands {
			commandNames[i] = c.Cmd
		}

		templates := &promptui.SelectTemplates{
			Label:    "{{ . }}",
			Active:   `{{ "›" | green | bold }} {{ . | green | bold }}`,
			Inactive: "  {{ . | faint }}",
			Selected: `{{ "✔" | green | bold }} {{ "Selected command:" | bold }} {{ . | yellow }}`,
		}

		prompt := promptui.Select{
			Label:     "Select a command to run",
			Items:     commandNames,
			Templates: templates,
		}

		_, selectedCommand, err := prompt.Run()
		if err != nil {
			if errors.Is(err, promptui.ErrInterrupt) {
				src.PrintInfo("Command selection cancelled.")
			} else {
				src.PrintError("Prompt failed %v\n", err)
			}
			return
		}

		executeProjectCommand(selectedCommand, config)
	},
}

func executeProjectCommand(commandName string, config *src.ProjectConfig) {
	yellow := src.Yellow()

	for _, c := range config.Commands {
		if c.Cmd == commandName {
			src.PrintInfo("Running command: %s", yellow.Sprint(c.Cmd))
			src.PrintInfo("-> %s", c.Run)

			if err := src.ExecuteCommand(c.Run); err != nil {
				src.PrintError("Command failed with error: %v", err)
				return
			}
			src.PrintSuccess("Command '%s' finished successfully.", c.Cmd)
			return
		}
	}

	src.PrintError("Command '%s' not found in zoi.yaml", commandName)
}

func init() {
	rootCmd.AddCommand(runCmd)
}
