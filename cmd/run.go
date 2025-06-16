package cmd

import (
	"zoi/src"

	"github.com/spf13/cobra"
)

var runCmd = &cobra.Command{
	Use:   "run [command]",
	Short: "Execute a command defined in a local zoi.yaml file",
	Args:  cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		commandName := args[0]
		yellow := src.Yellow()

		config, err := src.LoadProjectConfig()
		if err != nil {
			src.PrintError("%v", err)
			return
		}

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
	},
}

func init() {
	rootCmd.AddCommand(runCmd)
}
