package cmd

import (
	"fmt"
	"runtime"
	"zoi/src"

	"github.com/spf13/cobra"
)

const (
	installScriptURL     = "https://gitlab.com/Zillwen/Zusty/Zoi/-/raw/main/app/install.sh"
	installPowershellURL = "https://gitlab.com/Zillwen/Zusty/Zoi/-/raw/main/app/install.ps1"
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
	var scriptURL string
	if runtime.GOOS == "windows" {
		scriptURL = installPowershellURL
	} else {
		scriptURL = installScriptURL
	}

	if err := src.DownloadAndExecuteScript(scriptURL); err != nil {
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
