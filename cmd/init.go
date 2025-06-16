package cmd

import (
	"os"
	"path/filepath"
	"zoi/src"

	"github.com/spf13/cobra"
	"github.com/spf13/viper"
)

var initCmd = &cobra.Command{
	Use:   "init",
	Short: "Initialize Zoi configuration",
	Run: func(cmd *cobra.Command, args []string) {
		home, err := os.UserHomeDir()
		cobra.CheckErr(err)
		configPath := filepath.Join(home, ".zoi")
		if _, err := os.Stat(configPath); os.IsNotExist(err) {
			os.MkdirAll(configPath, os.ModePerm)
		}

		src.PrintInfo("Detecting system information...")
		osName, arch, distro, pkgManager := src.GetSystemInfo()

		if osName == "linux" && pkgManager == "" {
			src.PrintError("WARNING: Could not auto-detect a supported package manager.")
			src.PrintInfo("You may need to set it manually via 'zoi set pkgManager <manager>' (e.g. apt, dnf, pacman).")
		}

		viper.Set("os", osName)
		viper.Set("arch", arch)
		viper.Set("distro", distro)
		viper.Set("pkgManager", pkgManager)
		viper.SetDefault("appsUrl", "https://zusty.codeberg.page/Zoi/@main/app/apps.json")

		if err := viper.WriteConfigAs(filepath.Join(configPath, "config.yaml")); err != nil {
			src.PrintError("Error initializing Zoi: %v", err)
			return
		}
		src.PrintSuccess("Zoi configuration file created at %s", viper.ConfigFileUsed())
	},
}

func init() {
	rootCmd.AddCommand(initCmd)
}
