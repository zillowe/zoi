package pkgmanager

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"zoi/src"

	"github.com/go-git/go-git/v5"
	"gopkg.in/yaml.v2"
)

type DependencyResolver struct {
	processed            map[string]bool
	homeDir              string
	systemPackageManager string
}

func NewResolver() (*DependencyResolver, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return nil, err
	}
	_, _, _, pkgManager := src.GetSystemInfo()

	return &DependencyResolver{
		processed:            make(map[string]bool),
		homeDir:              home,
		systemPackageManager: pkgManager,
	}, nil
}

func (dr *DependencyResolver) isInstalled(handle string) bool {
	storePath := filepath.Join(dr.homeDir, ".zoi", "pkgs", "store", handle)
	_, err := os.Stat(storePath)
	return err == nil
}

func (dr *DependencyResolver) ResolveAndInstall(recipe *PackageRecipe, handle string, noCache bool) error {
	if dr.processed[handle] {
		return nil
	}

	src.PrintHighlight("\n--- Resolving: %s ---", handle)

	if dr.isInstalled(handle) {
		src.PrintSuccess("Package '%s' is already installed. Skipping.", handle)
		dr.processed[handle] = true
		return nil
	}

	isBuildFromSource := recipe.PackageInfo.Bin == "" && recipe.PackageInfo.Installer == ""
	if isBuildFromSource {
		src.PrintInfo("Build from source detected. Resolving build dependencies...")
		if err := dr.resolveDependencyList(recipe.Build.Depends, noCache); err != nil {
			return err
		}
	}

	src.PrintInfo("Resolving runtime dependencies...")
	if err := dr.resolveDependencyList(recipe.Depends, noCache); err != nil {
		return err
	}

	src.PrintHighlight("--- Installing: %s ---", handle)
	var installedBinPath string
	var err error

	if recipe.PackageInfo.Installer != "" {
		err = installFromInstaller(recipe, handle)
		if err == nil {
			dr.processed[handle] = true
		}
		return err
	} else if isBuildFromSource {
		installedBinPath, err = installFromSource(recipe, handle, !noCache)
	} else {
		installedBinPath, err = installFromBinary(recipe, handle)
	}

	if err != nil {
		return err
	}

	binsPath := filepath.Join(dr.homeDir, ".zoi", "pkgs", "bins")
	os.MkdirAll(binsPath, 0755)
	symlinkPath := filepath.Join(binsPath, handle)

	if recipe.PackageInfo.Installer == "" {
		if err := src.UpdateSymlink(installedBinPath, symlinkPath); err != nil {
			return fmt.Errorf("failed to create symlink: %w", err)
		}
		src.PrintSuccess("Binary for '%s' linked to %s", handle, symlinkPath)
	}

	recipePath := filepath.Join(dr.homeDir, ".zoi", "pkgs", "store", handle, "zoi.yaml")
	recipeData, err := yaml.Marshal(recipe)
	if err != nil {
		return fmt.Errorf("failed to serialize recipe for saving: %w", err)
	}
	if err := os.WriteFile(recipePath, recipeData, 0644); err != nil {
		return fmt.Errorf("failed to save recipe metadata: %w", err)
	}

	dr.processed[handle] = true
	return nil
}

func (dr *DependencyResolver) resolveDependencyList(dependencies []Dependency, noCache bool) error {
	if len(dependencies) == 0 {
		src.PrintInfo("No dependencies in this list to process.")
		return nil
	}

	for _, dep := range dependencies {
		targetPM := dep.Install.PM
		targetHandle := dep.Install.Handle
		if targetHandle == "" {
			targetHandle = dep.Handle
		}

		if targetPM == "" || targetPM == "zoi" {
			src.PrintInfo("Resolving Zoi package dependency: '%s'", dep.Handle)
			depRecipe, err := LoadPackageRecipe(dep.Handle)
			if err != nil {
				return fmt.Errorf("could not load recipe for dependency '%s': %w", dep.Handle, err)
			}
			if err := dr.ResolveAndInstall(depRecipe, dep.Handle, noCache); err != nil {
				return fmt.Errorf("failed to resolve dependency '%s': %w", dep.Handle, err)
			}
			continue
		}

		if targetPM == dr.systemPackageManager {
			src.PrintInfo("System dependency found for '%s': ensuring '%s' is installed...", targetPM, targetHandle)
			if err := src.InstallPackage(targetPM, targetHandle); err != nil {
				return fmt.Errorf("failed to install system dependency '%s' with %s: %w", targetHandle, targetPM, err)
			}
		} else {
			src.PrintInfo("Skipping dependency '%s' (for %s); not applicable to your system (%s).", targetHandle, targetPM, dr.systemPackageManager)
		}
	}
	return nil
}

func LoadPackageRecipe(handle string) (*PackageRecipe, error) {
	home, _ := os.UserHomeDir()
	dbPath := filepath.Join(home, ".zoi", "pkgs", "db")
	dbIndexPath := filepath.Join(dbPath, "pkgs.json")

	data, err := os.ReadFile(dbIndexPath)
	if err != nil {
		return nil, fmt.Errorf("could not read package database. Did you run 'zoi pkg sync'?")
	}
	var dbConfig PackageManagerConfig
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
	var recipe PackageRecipe
	if err := yaml.Unmarshal(recipeData, &recipe); err != nil {
		return nil, fmt.Errorf("failed to parse package recipe %s: %w", recipePath, err)
	}
	return &recipe, nil
}

func LoadInstalledRecipe(handle string) (*PackageRecipe, error) {
	home, _ := os.UserHomeDir()
	recipePath := filepath.Join(home, ".zoi", "pkgs", "store", handle, "zoi.yaml")

	data, err := os.ReadFile(recipePath)
	if err != nil {
		return nil, fmt.Errorf("could not read installed recipe for '%s'. It may be corrupted or was installed with an older Zoi version", handle)
	}

	var recipe PackageRecipe
	if err := yaml.Unmarshal(data, &recipe); err != nil {
		return nil, fmt.Errorf("failed to parse installed recipe for '%s': %w", handle, err)
	}
	return &recipe, nil
}

func installFromInstaller(recipe *PackageRecipe, handle string) error {
	replacer := strings.NewReplacer(
		"{version}", recipe.PackageInfo.Version,
		"{os}", runtime.GOOS,
		"{arch}", runtime.GOARCH,
		"{shellExt}", src.GetShellExtension(),
	)
	url := replacer.Replace(recipe.PackageInfo.Installer)

	return src.DownloadAndExecuteScript(url)
}

func installFromSource(recipe *PackageRecipe, handle string, useCache bool) (string, error) {
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

	if !useCache {
		src.PrintInfo("Removing source code due to --no-cache flag...")
		os.RemoveAll(codePath)
	}
	return destBinPath, nil
}

func installFromBinary(recipe *PackageRecipe, handle string) (string, error) {
	home, _ := os.UserHomeDir()
	pkgStorePath := filepath.Join(home, ".zoi", "pkgs", "store", handle)
	binStoreDir := filepath.Join(pkgStorePath, "bin")

	return DownloadAndExtractBinary(*recipe, binStoreDir)
}
