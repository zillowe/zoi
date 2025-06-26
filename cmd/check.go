package cmd

import (
	"fmt"
	"zoi/src"

	"github.com/spf13/cobra"
)

var checkCmd = &cobra.Command{
	Use:   "check",
	Short: "Verify system health and check for tool requirements",
	Run: func(cmd *cobra.Command, args []string) {
		toolsToCheck := []string{
			"git",
		}

		yellow := src.Yellow()
		allChecksPassed := true

		src.PrintHighlight("--- Checking for Essential Tools ---")

		for _, tool := range toolsToCheck {
			fmt.Printf("Checking for %s... ", yellow.Sprint(tool))
			if src.CheckCommand(tool) {
				src.PrintSuccess("OK")
			} else {
				src.PrintError("MISSING")
				allChecksPassed = false
			}
		}

		fmt.Println()

		if allChecksPassed {
			src.PrintSuccess("All essential tools checked are installed.")
		} else {
			src.PrintError("Some required tools are missing. Please install them to ensure full functionality.")
		}
	},
}

func init() {
	rootCmd.AddCommand(checkCmd)
}
