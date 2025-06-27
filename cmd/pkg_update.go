package cmd

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"zoi/src"
	"zoi/src/pkgmanager"

	"github.com/hashicorp/go-version"
	"github.com/manifoldco/promptui"
	"github.com/spf13/cobra"
)

var updateNoCache bool

var pkgUpdateCmd = &cobra.Command{
	Use:     "update [package-handle]",
	Short:   "Update installed packages to their latest versions",
	Aliases: []string{"u", "upgrade"},
	Args:    cobra.MaximumNArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		if len(args) == 1 {
			handle := args[0]
			needsUpdate, currentVer, latestVer, err := checkPackageUpdate(handle)
			if err != nil {
				src.PrintError("%v", err)
				return
			}
			if !needsUpdate {
				src.PrintSuccess("Package '%s' is already up to date (version %s).", handle, currentVer)
				return
			}
			src.PrintHighlight("Update available for %s: %s -> %s", handle, currentVer, latestVer)
			if err := reinstallPackage(handle); err != nil {
				src.PrintError("Update for '%s' failed: %v", handle, err)
			}
			return
		}

		updateAllPackages()
	},
}

func updateAllPackages() {
	src.PrintInfo("Checking for updates for all installed packages...")
	home, _ := os.UserHomeDir()
	installedHandles := getInstalledHandles(home)

	if len(installedHandles) == 0 {
		src.PrintInfo("No packages are currently installed.")
		return
	}

	var updatesAvailable []string
	for handle := range installedHandles {
		needsUpdate, _, _, err := checkPackageUpdate(handle)
		if err == nil && needsUpdate {
			updatesAvailable = append(updatesAvailable, handle)
		}
	}

	if len(updatesAvailable) == 0 {
		src.PrintSuccess("All installed packages are up to date.")
		return
	}

	src.PrintHighlight("\nFound updates for the following packages:")
	for _, handle := range updatesAvailable {
		fmt.Printf("- %s\n", handle)
	}

	prompt := promptui.Prompt{
		Label:     "Do you want to apply these updates?",
		IsConfirm: true,
	}
	_, err := prompt.Run()
	if err != nil {
		if errors.Is(err, promptui.ErrAbort) {
			src.PrintInfo("Update cancelled.")
			return
		}
	}

	for _, handle := range updatesAvailable {
		if err := reinstallPackage(handle); err != nil {
			src.PrintError("Update for '%s' failed: %v", handle, err)
		}
	}
}

func checkPackageUpdate(handle string) (needsUpdate bool, currentVerStr, latestVerStr string, err error) {
	currentRecipe, err := pkgmanager.LoadInstalledRecipe(handle)
	if err != nil {
		return false, "", "", err
	}

	latestRecipe, err := pkgmanager.LoadPackageRecipe(handle)
	if err != nil {
		return false, "", "", err
	}

	currentV, err := version.NewVersion(currentRecipe.PackageInfo.Version)
	if err != nil {
		return false, "", "", fmt.Errorf("invalid installed version for '%s': %w", handle, err)
	}

	latestV, err := version.NewVersion(latestRecipe.PackageInfo.Version)
	if err != nil {
		return false, "", "", fmt.Errorf("invalid latest version for '%s': %w", handle, err)
	}

	return latestV.GreaterThan(currentV), currentV.String(), latestV.String(), nil
}

func reinstallPackage(handle string) error {
	src.PrintHighlight("\n--- Updating: %s ---", handle)

	home, _ := os.UserHomeDir()
	storePath := filepath.Join(home, ".zoi", "pkgs", "store", handle)
	binPath := filepath.Join(home, ".zoi", "pkgs", "bins", handle)

	src.PrintInfo("Removing old version...")
	os.Remove(binPath)
	if err := os.RemoveAll(storePath); err != nil {
		return fmt.Errorf("failed to remove old package directory: %w", err)
	}

	latestRecipe, err := pkgmanager.LoadPackageRecipe(handle)
	if err != nil {
		return err
	}

	resolver, err := pkgmanager.NewResolver()
	if err != nil {
		return err
	}

	return resolver.ResolveAndInstall(latestRecipe, handle, updateNoCache)
}

func init() {
	pkgUpdateCmd.Flags().BoolVar(&updateNoCache, "no-cache", false, "Remove source code after building (if applicable)")
	pkgCmd.AddCommand(pkgUpdateCmd)
}
