package cmd

import (
	"fmt"
	"zoi/src"

	"github.com/spf13/cobra"
)

var versionCmd = &cobra.Command{
	Use:     "version",
	Short:   "Print the version number of Zoi",
	Aliases: []string{"v"},
	Run: func(cmd *cobra.Command, args []string) {
		src.PrintHighlight("--- Zoi Version ---")
		branchToPrint := currentVersionInfo.Branch
		if currentVersionInfo.Branch == "Prod." {
			branchToPrint = "Production"
		} else if currentVersionInfo.Branch == "Dev." {
			branchToPrint = "Development"
		}
		yellow := src.Yellow()

		fmt.Printf("Branch: %s\n", yellow.Sprint(branchToPrint))
		fmt.Printf("Status: %s\n", yellow.Sprint(currentVersionInfo.Status))
		fmt.Printf("Number: %s\n", yellow.Sprint(currentVersionInfo.Number))
		fmt.Printf("Commit: %s\n", yellow.Sprint(currentVersionInfo.Commit))
	},
}

func init() {
	rootCmd.AddCommand(versionCmd)
}
