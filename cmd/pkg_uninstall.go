package cmd

import (
	"fmt"
	"os"
	"path/filepath"
	"zoi/src"

	"github.com/spf13/cobra"
)

var pkgUninstallCmd = &cobra.Command{
	Use:     "uninstall [package-handle]",
	Short:   "Uninstall a package",
	Aliases: []string{"ui", "remove", "rm"},
	Args:    cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		handle := args[0]
		yellow := src.Yellow()

		home, err := os.UserHomeDir()
		if err != nil {
			src.PrintError("Could not get user home directory: %v", err)
			return
		}

		binsPath := filepath.Join(home, ".zoi", "pkgs", "bins")
		storePath := filepath.Join(home, ".zoi", "pkgs", "store")

		symlinkPath := filepath.Join(binsPath, handle)
		packageStorePath := filepath.Join(storePath, handle)

		if _, err := os.Lstat(symlinkPath); os.IsNotExist(err) {
			src.PrintError("Package '%s' is not installed.", handle)
			return
		}

		src.PrintInfo("Removing binary link: %s", symlinkPath)
		if err := os.Remove(symlinkPath); err != nil {
			src.PrintError("Warning: could not remove symlink: %v", err)
		} else {
			src.PrintSuccess("Binary link removed.")
		}

		src.PrintInfo("Removing package data from store: %s", packageStorePath)
		if err := os.RemoveAll(packageStorePath); err != nil {
			src.PrintError("Failed to remove package store directory: %v", err)
			return
		}
		src.PrintSuccess("Package data removed.")

		fmt.Println()
		src.PrintHighlight("Successfully uninstalled '%s'.", yellow.Sprint(handle))
	},
}

func init() {
	pkgCmd.AddCommand(pkgUninstallCmd)
}
