package cmd

import (
	"os"
	"path/filepath"
	"strings"
	"zoi/src"

	"github.com/go-git/go-git/v5"
	"github.com/spf13/cobra"
)

var buildNoCache bool

var pkgBuildCmd = &cobra.Command{
	Use:     "build [package-handle]",
	Short:   "Build a package from source using its recipe",
	Aliases: []string{"b"},
	Args:    cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		target := args[0]

		recipe, _, err := getRecipe(target)
		if err != nil {
			src.PrintError("%v", err)
			return
		}
		src.PrintInfo("Found recipe for package: %s (%s)", recipe.PackageInfo.Name, recipe.PackageInfo.Version)

		home, _ := os.UserHomeDir()
		handle := recipe.PackageInfo.Handle
		pkgStorePath := filepath.Join(home, ".zoi", "pkgs", "store", handle)
		codePath := filepath.Join(pkgStorePath, "code")

		src.PrintInfo("Cloning source code from %s...", recipe.PackageInfo.Repo)
		os.RemoveAll(codePath)
		_, err = git.PlainClone(codePath, false, &git.CloneOptions{
			URL:      recipe.PackageInfo.Repo,
			Progress: os.Stdout,
		})
		if err != nil {
			src.PrintError("Failed to clone repository: %v", err)
			return
		}

		src.PrintHighlight("--- Building Package ---")
		shellExt := src.GetShellExtension()
		buildCmd := strings.ReplaceAll(recipe.Build.Cmd, "{shellExt}", shellExt)
		src.PrintInfo("Executing build command: %s", buildCmd)

		originalDir, _ := os.Getwd()
		os.Chdir(codePath)
		if err := src.ExecuteCommand(buildCmd); err != nil {
			os.Chdir(originalDir)
			src.PrintError("Build failed: %v", err)
			return
		}
		os.Chdir(originalDir)

		sourceBinPath := filepath.Join(codePath, recipe.Build.Bin)
		src.PrintSuccess("Build complete! Binary available at: %s", sourceBinPath)

		if buildNoCache {
			src.PrintInfo("Removing source code due to --no-cache flag...")
			os.RemoveAll(codePath)
		}
	},
}

func init() {
	pkgBuildCmd.Flags().BoolVarP(&buildNoCache, "no-cache", "n", false, "Remove source code after building")
	pkgBuildCmd.Flags().BoolVar(&providerGithub, "github", false, "Hint that the URL is a GitHub-style repository")
	pkgBuildCmd.Flags().BoolVar(&providerGitlab, "gitlab", false, "Hint that the URL is a GitLab-style repository")
	pkgBuildCmd.Flags().BoolVar(&providerGitea, "gitea", false, "Hint that the URL is a Gitea-style repository")
	pkgBuildCmd.Flags().BoolVar(&providerForgejo, "forgejo", false, "Hint that the URL is a Forgejo-style repository")

	pkgCmd.AddCommand(pkgBuildCmd)
}
