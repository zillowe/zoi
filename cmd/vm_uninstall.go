package cmd

import (
	"os"
	"path/filepath"
	"strings"
	"zoi/src"

	"github.com/spf13/cobra"
)

var vmUninstallCmd = &cobra.Command{
	Use:   "uninstall [tool]@[version]",
	Short: "Uninstall a specific tool version",
	Args:  cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		parts := strings.Split(args[0], "@")
		if len(parts) < 2 || parts[1] == "" {
			src.PrintError("You must specify a version to uninstall (e.g. go@1.20.2).")
			return
		}
		tool, version := parts[0], parts[1]

		if tool != "go" {
			src.PrintError("Sorry, only 'go' is supported at the moment.")
			return
		}

		home, _ := os.UserHomeDir()
		versionDir := filepath.Join(home, ".zoi", "vm", tool, version)

		if _, err := os.Stat(versionDir); os.IsNotExist(err) {
			src.PrintError("Go version %s is not installed.", version)
			return
		}

		for _, linkName := range []string{"global", "latest"} {
			linkPath := filepath.Join(home, ".zoi", "vm", tool, linkName)
			if target, err := os.Readlink(linkPath); err == nil && target == versionDir {
				src.PrintInfo("Removing '%s' symlink...", linkName)
				os.Remove(linkPath)
			}
		}

		src.PrintInfo("Uninstalling Go version %s from %s...", version, versionDir)
		if err := os.RemoveAll(versionDir); err != nil {
			src.PrintError("Failed to uninstall: %v", err)
			return
		}

		src.PrintSuccess("Successfully uninstalled Go version %s.", version)
	},
}

func init() {
	vmCmd.AddCommand(vmUninstallCmd)
}
