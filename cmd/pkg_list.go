package cmd

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"text/tabwriter"
	"zoi/src"
	"zoi/src/pkgmanager"

	"github.com/spf13/cobra"
)

var listInstalledOnly bool

var pkgListCmd = &cobra.Command{
	Use:     "list",
	Short:   "List all available packages in the local database",
	Aliases: []string{"ls"},
	Run: func(cmd *cobra.Command, args []string) {
		home, _ := os.UserHomeDir()
		dbPath := filepath.Join(home, ".zoi", "pkgs", "db")
		dbIndexPath := filepath.Join(dbPath, "pkgs.json")

		data, err := os.ReadFile(dbIndexPath)
		if err != nil {
			src.PrintError("Could not read package database. Did you run 'zoi pkg sync'?")
			return
		}

		var dbConfig pkgmanager.PackageManagerConfig
		if err := json.Unmarshal(data, &dbConfig); err != nil {
			src.PrintError("Failed to parse package database: %v", err)
			return
		}

		var installedHandles map[string]bool
		if listInstalledOnly {
			installedHandles = getInstalledHandles(home)
		}

		packages := make([]pkgmanager.Package, 0, len(dbConfig.Packages))
		for handle, pkg := range dbConfig.Packages {
			pkg.Name = handle
			
			if listInstalledOnly {
				if _, ok := installedHandles[handle]; ok {
					packages = append(packages, pkg)
				}
			} else {
				packages = append(packages, pkg)
			}
		}

		sort.Slice(packages, func(i, j int) bool {
			return packages[i].Name < packages[j].Name
		})

		w := tabwriter.NewWriter(os.Stdout, 0, 0, 3, ' ', 0)
		fmt.Fprintln(w, "HANDLE\tVERSION\tDESCRIPTION")
		fmt.Fprintln(w, "------\t-------\t-----------")

		for _, pkg := range packages {
			fmt.Fprintf(w, "%s\t%s\t%s\n", src.Yellow().Sprint(pkg.Name), pkg.Version, pkg.Desc)
		}

		w.Flush()
	},
}

func getInstalledHandles(homeDir string) map[string]bool {
	storePath := filepath.Join(homeDir, ".zoi", "pkgs", "store")
	installed := make(map[string]bool)

	entries, err := os.ReadDir(storePath)
	if err != nil {
		return installed
	}

	for _, entry := range entries {
		if entry.IsDir() {
			installed[entry.Name()] = true
		}
	}
	return installed
}


func init() {
	pkgListCmd.Flags().BoolVarP(&listInstalledOnly, "installed", "I", false, "Only show installed packages")
	pkgCmd.AddCommand(pkgListCmd)
}
