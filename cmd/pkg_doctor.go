package cmd

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"zoi/src"

	"github.com/manifoldco/promptui"
	"github.com/spf13/cobra"
)

var pkgDoctorCmd = &cobra.Command{
	Use:     "doctor",
	Short:   "Check for issues and clean up the Zoi package installation",
	Aliases: []string{"doc"},
	Run: func(cmd *cobra.Command, args []string) {
		home, err := os.UserHomeDir()
		if err != nil {
			src.PrintError("Could not get user home directory: %v", err)
			return
		}

		binsPath := filepath.Join(home, ".zoi", "pkgs", "bins")
		storePath := filepath.Join(home, ".zoi", "pkgs", "store")

		src.PrintHighlight("--- Zoi Package Doctor ---")

		brokenLinks, err := checkBrokenSymlinks(binsPath)
		if err != nil {
			src.PrintError("Error checking symlinks: %v", err)
		}

		orphanedDirs, err := checkOrphanedStoreDirs(binsPath, storePath)
		if err != nil {
			src.PrintError("Error checking for orphans: %v", err)
		}

		sourceCodeDirs, err := findSourceCodeDirs(storePath)
		if err != nil {
			src.PrintError("Error checking for source code: %v", err)
		}

		if len(brokenLinks) == 0 && len(orphanedDirs) == 0 && len(sourceCodeDirs) == 0 {
			src.PrintSuccess("\n✅ No issues found. Your Zoi installation looks healthy!")
			return
		}

		promptAndFixIssues(brokenLinks, orphanedDirs, sourceCodeDirs, binsPath)
	},
}

func init() {
	pkgCmd.AddCommand(pkgDoctorCmd)
}

func checkBrokenSymlinks(binsPath string) ([]string, error) {
	src.PrintInfo("\n1. Checking for broken symlinks in %s...", binsPath)
	var broken []string

	if _, err := os.Stat(binsPath); os.IsNotExist(err) {
		src.PrintInfo("   Bins directory not found. Nothing to check.")
		return nil, nil
	}

	links, err := os.ReadDir(binsPath)
	if err != nil {
		return nil, err
	}

	for _, link := range links {
		linkPath := filepath.Join(binsPath, link.Name())
		if _, err := os.Stat(linkPath); os.IsNotExist(err) {
			broken = append(broken, link.Name())
		}
	}

	if len(broken) > 0 {
		src.PrintError("   Found %d broken symlinks.", len(broken))
	} else {
		src.PrintSuccess("   No broken symlinks found.")
	}
	return broken, nil
}

func checkOrphanedStoreDirs(binsPath, storePath string) ([]string, error) {
	src.PrintInfo("\n2. Checking for orphaned package data...")
	var orphans []string

	if _, err := os.Stat(storePath); os.IsNotExist(err) {
		src.PrintInfo("   Store directory not found. Nothing to check.")
		return nil, nil
	}

	storeDirs, err := os.ReadDir(storePath)
	if err != nil {
		return nil, err
	}

	for _, dir := range storeDirs {
		if !dir.IsDir() {
			continue
		}
		handle := dir.Name()
		symlinkPath := filepath.Join(binsPath, handle)
		if _, err := os.Lstat(symlinkPath); os.IsNotExist(err) {
			orphans = append(orphans, handle)
		}
	}

	if len(orphans) > 0 {
		src.PrintError("   Found %d orphaned packages (data exists but not installed).", len(orphans))
	} else {
		src.PrintSuccess("   No orphaned package data found.")
	}
	return orphans, nil
}

func findSourceCodeDirs(storePath string) ([]string, error) {
	src.PrintInfo("\n3. Checking for leftover source code...")
	var sources []string

	if _, err := os.Stat(storePath); os.IsNotExist(err) {
		src.PrintInfo("   Store directory not found. Nothing to check.")
		return nil, nil
	}

	storeDirs, err := os.ReadDir(storePath)
	if err != nil {
		return nil, err
	}

	for _, dir := range storeDirs {
		if !dir.IsDir() {
			continue
		}
		handle := dir.Name()
		codePath := filepath.Join(storePath, handle, "code")
		if _, err := os.Stat(codePath); err == nil {
			sources = append(sources, handle)
		}
	}

	if len(sources) > 0 {
		src.PrintInfo("   Found %d packages with leftover build source code.", len(sources))
	} else {
		src.PrintSuccess("   No leftover source code found.")
	}
	return sources, nil
}

func promptAndFixIssues(brokenLinks, orphanedDirs, sourceCodeDirs []string, binsPath string) {
	home, _ := os.UserHomeDir()
	storePath := filepath.Join(home, ".zoi", "pkgs", "store")

	if len(brokenLinks) > 0 {
		prompt := promptui.Prompt{
			Label:     fmt.Sprintf("Do you want to remove the %d broken symlinks?", len(brokenLinks)),
			IsConfirm: true,
		}
		if _, err := prompt.Run(); err == nil {
			for _, handle := range brokenLinks {
				linkPath := filepath.Join(binsPath, handle)
				if err := os.Remove(linkPath); err == nil {
					src.PrintInfo("   Removed: %s", linkPath)
				}
			}
			src.PrintSuccess("Broken symlinks removed.")
		} else if errors.Is(err, promptui.ErrAbort) {
			src.PrintInfo("Skipping.")
		}
	}

	if len(orphanedDirs) > 0 {
		prompt := promptui.Prompt{
			Label:     fmt.Sprintf("Do you want to delete the %d orphaned package data directories?", len(orphanedDirs)),
			IsConfirm: true,
		}
		if _, err := prompt.Run(); err == nil {
			for _, handle := range orphanedDirs {
				dirPath := filepath.Join(storePath, handle)
				if err := os.RemoveAll(dirPath); err == nil {
					src.PrintInfo("   Deleted: %s", dirPath)
				}
			}
			src.PrintSuccess("Orphaned package data deleted.")
		} else if errors.Is(err, promptui.ErrAbort) {
			src.PrintInfo("Skipping.")
		}
	}

	if len(sourceCodeDirs) > 0 {
		prompt := promptui.Prompt{
			Label:     fmt.Sprintf("Do you want to clean up the %d leftover source code directories?", len(sourceCodeDirs)),
			IsConfirm: true,
		}
		if _, err := prompt.Run(); err == nil {
			for _, handle := range sourceCodeDirs {
				codePath := filepath.Join(storePath, handle, "code")
				if err := os.RemoveAll(codePath); err == nil {
					src.PrintInfo("   Cleaned: %s", codePath)
				}
			}
			src.PrintSuccess("Source code cleaned up.")
		} else if errors.Is(err, promptui.ErrAbort) {
			src.PrintInfo("Skipping.")
		}
	}

	fmt.Println()
	src.PrintHighlight("Doctor check complete.")
}
