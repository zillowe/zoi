package cmd

import (
	"os"
	"path/filepath"
	"zoi/src"

	"github.com/go-git/go-git/v5"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
)

const defaultEndpoint = "https://gitlab.com/Zusty/Zoi-Pkgs.git"

var pkgSyncCmd = &cobra.Command{
	Use:     "sync",
	Short:   "Sync the local package database from the remote endpoint",
	Aliases: []string{"s"},
	Run: func(cmd *cobra.Command, args []string) {
		endpoint := viper.GetString("pkg.endpoint")
		if endpoint == "" {
			src.PrintInfo("No endpoint configured, using default: %s", defaultEndpoint)
			endpoint = defaultEndpoint
		}

		home, _ := os.UserHomeDir()
		dbPath := filepath.Join(home, ".zoi", "pkgs", "db")

		if _, err := os.Stat(dbPath); os.IsNotExist(err) {
			src.PrintInfo("Cloning package database from %s...", endpoint)
			_, err := git.PlainClone(dbPath, false, &git.CloneOptions{
				URL:      endpoint,
				Progress: os.Stdout,
			})
			if err != nil {
				src.PrintError("Failed to clone package database: %v", err)
				return
			}
			src.PrintSuccess("Package database synced successfully.")
		} else {
			src.PrintInfo("Package database exists. Pulling updates...")
			r, err := git.PlainOpen(dbPath)
			if err != nil {
				src.PrintError("Failed to open existing database repository: %v", err)
				return
			}
			w, err := r.Worktree()
			if err != nil {
				src.PrintError("Failed to get worktree: %v", err)
				return
			}
			err = w.Pull(&git.PullOptions{RemoteName: "origin", Progress: os.Stdout})
			if err != nil && err != git.NoErrAlreadyUpToDate {
				src.PrintError("Failed to pull updates: %v", err)
				return
			}
			src.PrintSuccess("Package database is up to date.")
		}
	},
}

func init() {
	pkgCmd.AddCommand(pkgSyncCmd)

	viper.SetDefault("pkg.endpoint", "")
}
