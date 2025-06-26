package cmd

import (
	"github.com/spf13/cobra"
)

var pkgCmd = &cobra.Command{
	Use:     "pkg",
	Short:   "Manage universal packages with Zoi",
	Aliases: []string{"p"},
}

func init() {
	rootCmd.AddCommand(pkgCmd)
}
