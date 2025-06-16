package cmd

import (
	"fmt"
	"io"
	"net/http"
	"os"
	"runtime"
	"zoi/src"

	"github.com/spf13/cobra"
)

const (
	installScriptURL     = "https://zusty.codeberg.page/Zoi/@main/app/install.sh"
	installPowershellURL = "https://zusty.codeberg.page/Zoi/@main/app/install.ps1"
)

var updateCmd = &cobra.Command{
	Use:   "update",
	Short: "Update Zoi to the latest version",
	Run: func(cmd *cobra.Command, args []string) {
		forceUpdate, _ := cmd.Flags().GetBool("force")

		if forceUpdate {
			src.PrintInfo("Force flag detected. Skipping version check and reinstalling...")
			runInstaller()
			return
		}

		src.PrintInfo("Checking for new versions...")

		remoteConfig, err := src.FetchRemoteVersionInfo()
		if err != nil {
			src.PrintError("Could not fetch version information: %v", err)
			return
		}

		var remoteDetails src.VersionDetails
		if currentVersionInfo.Branch == "Dev." {
			remoteDetails = remoteConfig.Latest.Development
			src.PrintInfo("(Checking development branch)")
		} else {
			remoteDetails = remoteConfig.Latest.Production
			src.PrintInfo("(Checking production branch)")
		}

		updateAvailable, err := src.IsUpdateAvailable(
			currentVersionInfo.Branch,
			currentVersionInfo.Status,
			currentVersionInfo.Number,
			remoteDetails,
		)
		if err != nil {
			src.PrintError("Could not compare versions: %v", err)
			return
		}

		if !updateAvailable {
			src.PrintSuccess("You are already on the latest version (%s %s).", currentVersionInfo.Status, currentVersionInfo.Number)
			src.PrintInfo("Use 'zoi update --force' to reinstall anyway.")
			return
		}

		src.PrintHighlight("New version available! Remote: %s %s | Current: %s %s",
			remoteDetails.Status, remoteDetails.Version, currentVersionInfo.Status, currentVersionInfo.Number)
		fmt.Println()

		runInstaller()
	},
}

func runInstaller() {
	src.PrintInfo("Starting Zoi update process...")
	var scriptURL, filePattern string

	if runtime.GOOS == "windows" {
		scriptURL = installPowershellURL
		filePattern = "zoi-installer-*.ps1"
	} else {
		scriptURL = installScriptURL
		filePattern = "zoi-installer-*.sh"
	}

	src.PrintInfo("Downloading update script from %s...", scriptURL)
	resp, err := http.Get(scriptURL)
	if err != nil {
		src.PrintError("Failed to start download: %v", err)
		return
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		src.PrintError("Failed to download script: received status code %d", resp.StatusCode)
		return
	}

	tempFile, err := os.CreateTemp("", filePattern)
	if err != nil {
		src.PrintError("Failed to create temporary file for installer: %v", err)
		return
	}
	defer os.Remove(tempFile.Name())

	_, err = io.Copy(tempFile, resp.Body)
	if err != nil {
		src.PrintError("Failed to write update script to disk: %v", err)
		return
	}

	if err := tempFile.Close(); err != nil {
		src.PrintError("Failed to close temporary file: %v", err)
		return
	}

	if runtime.GOOS != "windows" {
		if err := os.Chmod(tempFile.Name(), 0755); err != nil {
			src.PrintError("Failed to make update script executable: %v", err)
			return
		}
	}

	fmt.Println()
	src.PrintInfo("Executing update script...")
	src.PrintInfo("You may be prompted for your password to install to a system directory.")
	if err := src.ExecuteCommand(tempFile.Name()); err != nil {
		src.PrintError("Update script failed during execution: %v", err)
		return
	}

	fmt.Println()
	src.PrintHighlight("Update process finished.")
	src.PrintInfo("Please open a new terminal or restart your shell to use the new version.")
}

func init() {
	updateCmd.Flags().BoolP("force", "f", false, "Force re-installation by skipping the version check")
	rootCmd.AddCommand(updateCmd)
}
