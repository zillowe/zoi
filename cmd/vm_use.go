package cmd

import (
	"os"
	"path/filepath"
	"strings"
	"zoi/src"
	"zoi/src/vm"

	"github.com/spf13/cobra"
)

var vmUseCmd = &cobra.Command{
	Use:   "use [tool]@[version]",
	Short: "Set the global version for a tool",
	Args:  cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		parts := strings.Split(args[0], "@")
		tool := parts[0]
		version := ""
		if len(parts) > 1 {
			version = parts[1]
		}

		if tool != "go" {
			src.PrintError("Sorry, only 'go' is supported at the moment.")
			return
		}

		home, _ := os.UserHomeDir()
		toolDir := filepath.Join(home, ".zoi", "vm", tool)
		targetVersionDir := ""

		if version == "" {
			latestLink := filepath.Join(toolDir, "latest")
			target, err := os.Readlink(latestLink)
			if err != nil {
				src.PrintError("Could not determine latest installed version. Please install one first.")
				return
			}
			targetVersionDir = target
			version = filepath.Base(target)
		} else {
			targetVersionDir = filepath.Join(toolDir, version)
		}

		if _, err := os.Stat(targetVersionDir); os.IsNotExist(err) {
			src.PrintError("Go version %s is not installed. Please run 'zoi vm install %s' first.", version, args[0])
			return
		}

		globalLink := filepath.Join(toolDir, "global")
		if err := vm.UpdateSymlink(targetVersionDir, globalLink); err != nil {
			src.PrintError("Failed to set global version: %v", err)
			return
		}

		if err := vm.UpdateShellProfile(); err != nil {
			src.PrintError("Failed to update shell profile: %v", err)
		}

		src.PrintSuccess("Now using %s version %s.", tool, version)
	},
}

func init() {
	vmCmd.AddCommand(vmUseCmd)
}
