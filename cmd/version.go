package cmd

import (
	"zoi/src"

	"github.com/spf13/cobra"
)

var versionCmd = &cobra.Command{
	Use:     "version",
	Short:   "Print the version number of Zoi",
	Aliases: []string{"v"},
	Run: func(cmd *cobra.Command, args []string) {
		src.PrintBlue("zoi version %s", cmd.Root().Version)
	},
}

func init() {
	rootCmd.AddCommand(versionCmd)
}
