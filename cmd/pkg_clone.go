package cmd

import (
	"fmt"
	"os"
	"path/filepath"
	"zoi/src"

	"github.com/go-git/go-git/v5"
	"github.com/spf13/cobra"
)

var pkgCloneCmd = &cobra.Command{
	Use:     "clone [package-handle]",
	Short:   "Clone a package's source code repository",
	Aliases: []string{"c"},
	Args:    cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		target := args[0]

		recipe, handle, err := getRecipe(target)
		if err != nil {
			src.PrintError("%v", err)
			return
		}
		src.PrintInfo("Found recipe for package: %s", recipe.PackageInfo.Name)

		home, _ := os.UserHomeDir()
		pkgStorePath := filepath.Join(home, ".zoi", "pkgs", "store", handle)
		clonePath := filepath.Join(pkgStorePath, "code")

		if _, err := os.Stat(clonePath); err == nil {
			src.PrintError("Clone destination already exists: %s", clonePath)
			src.PrintInfo("To re-clone, please remove this directory first.")
			return
		}

		src.PrintInfo("Cloning %s into %s...", recipe.PackageInfo.Repo, clonePath)
		_, err = git.PlainClone(clonePath, false, &git.CloneOptions{
			URL:      recipe.PackageInfo.Repo,
			Progress: os.Stdout,
		})
		if err != nil {
			src.PrintError("Failed to clone repository: %v", err)
			return
		}

		fmt.Println()
		src.PrintSuccess("Successfully cloned repository.")
		src.PrintInfo("Source code is available at: %s", clonePath)
	},
}

func init() {
	pkgCloneCmd.Flags().BoolVar(&providerGithub, "github", false, "Hint that the URL is a GitHub-style repository")
	pkgCloneCmd.Flags().BoolVar(&providerGitlab, "gitlab", false, "Hint that the URL is a GitLab-style repository")
	pkgCloneCmd.Flags().BoolVar(&providerGitea, "gitea", false, "Hint that the URL is a Gitea-style repository")
	pkgCloneCmd.Flags().BoolVar(&providerForgejo, "forgejo", false, "Hint that the URL is a Forgejo-style repository")

	pkgCmd.AddCommand(pkgCloneCmd)
}
