package cmd

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"zoi/src"
	"zoi/src/pkgmanager"

	"github.com/go-git/go-git/v5"
	"github.com/spf13/cobra"
	"gopkg.in/yaml.v2"
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
	Short:   "Install a package from the database or a git repository URL",
	Aliases: []string{"i"},
	Args:    cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		target := args[0]

		recipe, handle, err := getRecipe(target)
		if err != nil {
			src.PrintError("%v", err)
			return
		}
		
		if recipe.PackageInfo.Bin == "" && recipe.PackageInfo.Installer == "" && recipe.Build.Cmd == "" {
			src.PrintError("Invalid recipe for '%s': must contain a 'bin' url, an 'installer' url, or a 'build' command.", handle)
			return
		}
		if recipe.PackageInfo.Bin != "" && recipe.PackageInfo.Installer != "" {
			src.PrintError("Invalid recipe for '%s': cannot contain both a 'bin' url and an 'installer' url.", handle)
			return
		}

		err = processInstallation(recipe, handle)
		if err != nil {
			src.PrintError("Installation failed: %v", err)
			return
		}

		src.PrintHighlight("\nInstallation of '%s' complete!", recipe.PackageInfo.Name)
	},
}

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

func processInstallation(recipe *pkgmanager.PackageRecipe, handle string) error {
	src.PrintInfo("Starting installation for: %s (%s)", recipe.PackageInfo.Name, recipe.PackageInfo.Version)

	var installedBinPath string
	var err error

	if recipe.PackageInfo.Installer != "" {
		err = installFromInstallerScript(recipe)
		if err == nil {
			return nil
		}
	} else if recipe.PackageInfo.Bin != "" {
		installedBinPath, err = installFromBinary(recipe, handle)
	} else {
		src.PrintInfo("No pre-compiled binary or installer script found. Building from source...")
		installedBinPath, err = installFromSource(recipe, handle)
	}

	if err != nil {
		return err
	}
	
	home, _ := os.UserHomeDir()
	binsPath := filepath.Join(home, ".zoi", "pkgs", "bins")
	os.MkdirAll(binsPath, 0755)
	symlinkPath := filepath.Join(binsPath, handle)

	if err := src.UpdateSymlink(installedBinPath, symlinkPath); err != nil {
		return fmt.Errorf("failed to create symlink: %w", err)
	}

	src.PrintSuccess("Binary linked to %s", symlinkPath)
	return nil
}

func installFromInstallerScript(recipe *pkgmanager.PackageRecipe) error {
	replacer := strings.NewReplacer("{shellExt}", src.GetShellExtension())
	url := replacer.Replace(recipe.PackageInfo.Installer)
	
	return src.DownloadAndExecuteScript(url)
}

func loadPackageRecipe(handle string) (*pkgmanager.PackageRecipe, error) {
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
	recipeData, err := os.ReadFile(recipePath)
	if err != nil {
		return nil, fmt.Errorf("could not read package recipe at %s: %w", recipePath, err)
	}
	var recipe pkgmanager.PackageRecipe
	if err := yaml.Unmarshal(recipeData, &recipe); err != nil {
		return nil, fmt.Errorf("failed to parse package recipe %s: %w", recipePath, err)
	}
	return &recipe, nil
}

func installFromBinary(recipe *pkgmanager.PackageRecipe, handle string) (string, error) {
	home, _ := os.UserHomeDir()
	pkgStorePath := filepath.Join(home, ".zoi", "pkgs", "store", handle)
	binStoreDir := filepath.Join(pkgStorePath, "bin")

	return pkgmanager.DownloadAndExtractBinary(*recipe, binStoreDir)
}

func installFromSource(recipe *pkgmanager.PackageRecipe, handle string) (string, error) {
	home, _ := os.UserHomeDir()
	pkgStorePath := filepath.Join(home, ".zoi", "pkgs", "store", handle)
	codePath := filepath.Join(pkgStorePath, "code")
	binStoreDir := filepath.Join(pkgStorePath, "bin")
	destBinPath := filepath.Join(binStoreDir, handle)

	src.PrintInfo("Cloning source code from %s...", recipe.PackageInfo.Repo)
	os.RemoveAll(codePath)
	_, err := git.PlainClone(codePath, false, &git.CloneOptions{
		URL: recipe.PackageInfo.Repo, Progress: os.Stdout,
	})
	if err != nil {
		return "", fmt.Errorf("failed to clone repository: %w", err)
	}

	src.PrintHighlight("--- Building Package ---")

	shellExt := src.GetShellExtension()
	buildCmd := strings.ReplaceAll(recipe.Build.Cmd, "{shellExt}", shellExt)
	src.PrintInfo("Executing build command: %s", buildCmd)

	originalDir, _ := os.Getwd()
	os.Chdir(codePath)
	if err := src.ExecuteCommand(buildCmd); err != nil {
		os.Chdir(originalDir)
		return "", fmt.Errorf("build failed: %w", err)
	}

	os.Chdir(originalDir)
	src.PrintSuccess("Build complete.")

	os.MkdirAll(binStoreDir, 0755)
	sourceBinPath := filepath.Join(codePath, recipe.Build.Bin)
	if err := os.Rename(sourceBinPath, destBinPath); err != nil {
		return "", fmt.Errorf("failed to move compiled binary: %w", err)
	}

	if noCache {
		src.PrintInfo("Removing source code due to --no-cache flag...")
		os.RemoveAll(codePath)
	}
	return destBinPath, nil
}

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
		recipe, err = loadPackageRecipe(handle)
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
