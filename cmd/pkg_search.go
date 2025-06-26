package cmd

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"text/tabwriter"
	"zoi/src"
	"zoi/src/pkgmanager"

	"github.com/spf13/cobra"
)

var pkgSearchCmd = &cobra.Command{
	Use:   "search [keyword]",
	Short: "Search for packages by name or description",
	Args:  cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		keyword := strings.ToLower(args[0])

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

		var matches []pkgmanager.Package
		for handle, pkg := range dbConfig.Packages {
			pkg.Name = handle
			if strings.Contains(strings.ToLower(handle), keyword) || strings.Contains(strings.ToLower(pkg.Desc), keyword) {
				matches = append(matches, pkg)
			}
		}
		
		if len(matches) == 0 {
			src.PrintInfo("No packages found matching '%s'.", keyword)
			return
		}

		sort.Slice(matches, func(i, j int) bool {
			return matches[i].Name < matches[j].Name
		})

		w := tabwriter.NewWriter(os.Stdout, 0, 0, 3, ' ', 0)
		fmt.Fprintln(w, "HANDLE\tVERSION\tDESCRIPTION")
		fmt.Fprintln(w, "------\t-------\t-----------")

		for _, pkg := range matches {
			fmt.Fprintf(w, "%s\t%s\t%s\n", src.Yellow().Sprint(pkg.Name), pkg.Version, pkg.Desc)
		}

		w.Flush()
	},
}

func init() {
	pkgCmd.AddCommand(pkgSearchCmd)
}
