package cmd

import (
	"fmt"
	"strings"
	"zoi/src"
	"zoi/src/pkgmanager"

	"github.com/spf13/cobra"
)

var (
	noCache         bool
	providerGithub  bool
	providerGitlab  bool
	providerGitea   bool
	providerForgejo bool
)

var pkgInstallCmd = &cobra.Command{
	Use:     "install [package-handle | git-url]",
	Short:   "Install a package and its dependencies",
	Aliases: []string{"i"},
	Args:    cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		target := args[0]

		// getRecipe remains a local helper in this file to parse initial user input.
		recipe, handle, err := getRecipe(target)
		if err != nil {
			src.PrintError("%v", err)
			return
		}

		// 1. Create a new dependency resolver for this installation run.
		resolver, err := pkgmanager.NewResolver()
		if err != nil {
			src.PrintError("Failed to initialize installer: %v", err)
			return
		}

		// 2. Hand off the recipe to the resolver to start the process.
		err = resolver.ResolveAndInstall(recipe, handle)
		if err != nil {
			src.PrintError("\nInstallation failed: %v", err)
			return
		}

		src.PrintHighlight("\nInstallation of '%s' and its dependencies complete!", handle)
	},
}

// getProviderFromFlags remains the same.
func getProviderFromFlags() pkgmanager.Provider {
	if providerGithub {
		return pkgmanager.ProviderGitHub
	}
	if providerGitlab {
		return pkgmanager.ProviderGitLab
	}
	if providerGitea {
		return pkgmanager.ProviderGitea
	}
	if providerForgejo {
		return pkgmanager.ProviderForgejo
	}
	return ""
}

// getRecipe fetches the initial recipe, either from a URL or the local database.
func getRecipe(target string) (*pkgmanager.PackageRecipe, string, error) {
	var recipe *pkgmanager.PackageRecipe
	var err error
	var handle string

	if strings.HasPrefix(target, "http://") || strings.HasPrefix(target, "https://") {
		providerHint := getProviderFromFlags()
		recipe, err = pkgmanager.FetchRecipeFromURL(target, providerHint)
		if err == nil {
			handle = recipe.PackageInfo.Handle
			if handle == "" {
				return nil, "", fmt.Errorf("remote zoi.yaml must define a 'package.handle'")
			}
		}
	} else {
		handle = target
		// We use the public function from the pkgmanager package now.
		recipe, err = pkgmanager.LoadPackageRecipe(handle)
	}

	if err != nil {
		return nil, "", err
	}

	return recipe, handle, nil
}


func init() {
	pkgInstallCmd.Flags().BoolVarP(&noCache, "no-cache", "n", false, "Remove source code after building (source builds only)")
	pkgInstallCmd.Flags().BoolVar(&providerGithub, "github", false, "Hint that the URL is a GitHub-style repository")
	pkgInstallCmd.Flags().BoolVar(&providerGitlab, "gitlab", false, "Hint that the URL is a GitLab-style repository")
	pkgInstallCmd.Flags().BoolVar(&providerGitea, "gitea", false, "Hint that the URL is a Gitea-style repository (e.g. Codeberg)")
	pkgInstallCmd.Flags().BoolVar(&providerForgejo, "forgejo", false, "Hint that the URL is a Forgejo-style repository")

	pkgCmd.AddCommand(pkgInstallCmd)
}
