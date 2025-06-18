package cmd

import (
	"github.com/spf13/cobra"
)

var vmCmd = &cobra.Command{
	Use:   "vm",
	Short: "Manage language versions (e.g. Go)",
}

func init() {
	rootCmd.AddCommand(vmCmd)
}
