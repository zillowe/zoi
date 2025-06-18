package cmd

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"zoi/src"
	"zoi/src/vm"

	"github.com/spf13/cobra"
)

var vmInstallCmd = &cobra.Command{
	Use:   "install [tool]@[version]",
	Short: "Install a specific tool version (e.g. go@1.20.2)",
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

		src.PrintInfo("Fetching available Go versions...")
		allGoVersions, err := vm.GetGoVersions()
		if err != nil {
			src.PrintError("Failed to get versions: %v", err)
			return
		}

		var targetVersion *vm.GoFile
		if version == "" {
			src.PrintInfo("Finding the latest stable version...")
			targetVersion, err = vm.FindLatestGoVersion(runtime.GOOS, runtime.GOARCH, allGoVersions)
			if err != nil {
				src.PrintError("Failed to find latest version: %v", err)
				return
			}
			version = strings.TrimPrefix(targetVersion.Version, "go")
		} else {
			targetVersion, err = vm.FindGoVersion(version, runtime.GOOS, runtime.GOARCH, allGoVersions)
			if err != nil {
				src.PrintError("Failed to find version '%s': %v", version, err)
				return
			}
		}

		home, _ := os.UserHomeDir()
		installDir := filepath.Join(home, ".zoi", "vm", tool, version)

		if _, err := os.Stat(installDir); err == nil {
			src.PrintSuccess("Go version %s is already installed at %s", version, installDir)
			return
		}
		//! Delete
		if targetVersion.SHA256 == "" {
			yellow := src.Yellow()
			src.PrintInfo("%s", yellow.Sprint("WARNING: No checksum found for this archived version. Verification will be skipped."))
		}

		if err := vm.DownloadAndExtractGo(targetVersion, installDir); err != nil {
			src.PrintError("Installation failed: %v", err)
			os.RemoveAll(installDir)
			return
		}

		latestLink := filepath.Join(home, ".zoi", "vm", tool, "latest")
		if err := vm.UpdateSymlink(installDir, latestLink); err != nil {
			src.PrintError("Failed to update 'latest' symlink: %v", err)
		}

		src.PrintSuccess("Successfully installed %s to %s", args[0], installDir)
		src.PrintInfo("Run 'zoi vm use %s' to start using it.", args[0])
	},
}

func init() {
	vmCmd.AddCommand(vmInstallCmd)
}
