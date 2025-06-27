package cmd

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"zoi/src"
	"zoi/src/pkgmanager"

	"github.com/alecthomas/chroma/v2/quick"
	"github.com/spf13/cobra"
	"gopkg.in/yaml.v2"
)

var (
	viewProviderGithub  bool
	viewProviderGitlab  bool
	viewProviderGitea   bool
	viewProviderForgejo bool
	viewRaw             bool
)

var pkgViewCmd = &cobra.Command{
	Use:   "view [handle] <endpoint-url> | [git-url]",
	Short: "Display information about a package",
	Long: `Displays information about a package recipe.

By default, it shows a formatted, human-readable summary.
Use the --raw flag to view the raw zoi.yaml file with syntax highlighting.

- View a package from your local database:
  zoi pkg view <package-handle>

- View a package from a specific remote database:
  zoi pkg view <package-handle> <endpoint-url>

- View the zoi.yaml from the root of a git repository:
  zoi pkg view <git-url>`,
	Aliases: []string{"v"},
	Args:    cobra.RangeArgs(1, 2),
	Run: func(cmd *cobra.Command, args []string) {
		var yamlContent []byte
		var err error

		providerHint := getViewProviderFromFlags()

		switch len(args) {
		case 1:
			target := args[0]
			if strings.HasPrefix(target, "http://") || strings.HasPrefix(target, "https://") {
				src.PrintInfo("Fetching remote recipe from root of %s...", target)
				yamlContent, err = pkgmanager.FetchRawRecipeFromURL(target, providerHint)
			} else {
				yamlContent, err = loadRecipeContentFromDB(target)
			}
		case 2:
			handle := args[0]
			endpointURL := args[1]
			src.PrintInfo("Fetching recipe for '%s' from remote database %s...", handle, endpointURL)
			yamlContent, err = pkgmanager.FetchRecipeFromRemoteDB(handle, endpointURL, providerHint)
		}

		if err != nil {
			src.PrintError("%v", err)
			return
		}

		if len(args) == 2 {
			handle := args[0]
			endpointURL := args[1]
			src.PrintInfo("Verifying recipe signature...")
			if err := pkgmanager.VerifyRecipeSignature(yamlContent, handle, endpointURL, providerHint); err != nil {
				src.PrintError("SECURITY WARNING: Recipe signature verification failed: %v", err)
			}
		}

		if viewRaw {
			err = quick.Highlight(os.Stdout, string(yamlContent), "yaml", "terminal256", "onedark")
			if err != nil {
				src.PrintError("Syntax highlighting failed, printing plain text: %v", err)
				fmt.Println(string(yamlContent))
			}
			return
		}

		var recipe pkgmanager.PackageRecipe
		if err := yaml.Unmarshal(yamlContent, &recipe); err != nil {
			src.PrintError("Could not parse package recipe: %v", err)
			return
		}

		printFormattedRecipe(recipe)
	},
}

func printFormattedRecipe(recipe pkgmanager.PackageRecipe) {
	yellow := src.Yellow()

	fmt.Println()
	src.PrintHighlight("%s - %s - %s",
		recipe.PackageInfo.Name,
		yellow.Sprint(recipe.PackageInfo.Handle),
		recipe.PackageInfo.Desc)
	fmt.Println()

	fmt.Printf("  %-12s %s\n", "Version:", yellow.Sprint(recipe.PackageInfo.Version))
	fmt.Printf("  %-12s %s\n", "Website:", yellow.Sprint(recipe.PackageInfo.Website))
	fmt.Printf("  %-12s %s\n", "Repository:", yellow.Sprint(recipe.PackageInfo.Repo))
	if recipe.PackageInfo.Bin != "" {
		fmt.Printf("  %-12s %s\n", "Install:", yellow.Sprint("Pre-compiled Binary"))
	} else {
		fmt.Printf("  %-12s %s\n", "Install:", yellow.Sprint("From Source"))
	}

	if len(recipe.Depends) > 0 {
		fmt.Println()
		src.PrintHighlight("Runtime Dependencies:")
		for _, dep := range recipe.Depends {
			fmt.Printf("  - %s: minimum %s\n", yellow.Sprint(dep.Handle), dep.Version)
		}
	}

	if len(recipe.Build.Depends) > 0 {
		fmt.Println()
		src.PrintHighlight("Build Dependencies:")
		for _, dep := range recipe.Build.Depends {
			fmt.Printf("  - %s: minimum %s\n", yellow.Sprint(dep.Handle), dep.Version)
		}
	}
	fmt.Println()
}

func getViewProviderFromFlags() pkgmanager.Provider {
	if viewProviderGithub {
		return pkgmanager.ProviderGitHub
	}
	if viewProviderGitlab {
		return pkgmanager.ProviderGitLab
	}
	if viewProviderGitea {
		return pkgmanager.ProviderGitea
	}
	if viewProviderForgejo {
		return pkgmanager.ProviderForgejo
	}
	return ""
}
func loadRecipeContentFromDB(handle string) ([]byte, error) {
	home, _ := os.UserHomeDir()
	dbPath := filepath.Join(home, ".zoi", "pkgs", "db")
	dbIndexPath := filepath.Join(dbPath, "pkgs.json")

	data, err := os.ReadFile(dbIndexPath)
	if err != nil {
		return nil, fmt.Errorf("could not read package database. Did you run 'zoi pkg sync'?")
	}
	var dbConfig pkgmanager.PackageManagerConfig
	if err := json.Unmarshal(data, &dbConfig); err != nil {
		return nil, fmt.Errorf("failed to parse package database: %w", err)
	}

	pkgData, ok := dbConfig.Packages[handle]
	if !ok {
		return nil, fmt.Errorf("package '%s' not found in the database", handle)
	}

	recipePath := filepath.Join(dbPath, pkgData.PkgFile)
	return os.ReadFile(recipePath)
}

func init() {
	pkgViewCmd.Flags().BoolVar(&viewRaw, "raw", false, "Display the raw zoi.yaml file with syntax highlighting")

	pkgViewCmd.Flags().BoolVar(&viewProviderGithub, "github", false, "Hint that the URL is a GitHub-style repository")
	pkgViewCmd.Flags().BoolVar(&viewProviderGitlab, "gitlab", false, "Hint that the URL is a GitLab-style repository")
	pkgViewCmd.Flags().BoolVar(&viewProviderGitea, "gitea", false, "Hint that the URL is a Gitea-style repository (e.g. Codeberg)")
	pkgViewCmd.Flags().BoolVar(&viewProviderForgejo, "forgejo", false, "Hint that the URL is a Forgejo-style repository")

	pkgCmd.AddCommand(pkgViewCmd)
}
