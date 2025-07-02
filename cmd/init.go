package cmd

import (
	"os"
	"path/filepath"
	"zoi/src"

	"github.com/spf13/cobra"
	"github.com/spf13/viper"
)

var initCmd = &cobra.Command{
	Use:   "init [shell]",
	Short: "Initialize Zoi configuration or shell environment",
	Long: `Initializes Zoi's main configuration file in ~/.zoi/config.yaml.

If a shell name (bash, zsh, fish) is provided as an argument,
it will attempt to add the Zoi package binary path to your shell's PATH.`,
	Args: cobra.MaximumNArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		if len(args) > 0 {
			shellName := args[0]
			src.PrintInfo("Configuring shell environment for '%s'...", shellName)
			if err := src.AddPkgPathToShell(shellName); err != nil {
				src.PrintError("Failed to configure shell: %v", err)
			}
			return
		}

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
			src.PrintInfo("You may need to set it manually via 'zoi set pkgManager <manager>'")
		}

		viper.Set("os", osName)
		viper.Set("arch", arch)
		viper.Set("distro", distro)
		viper.Set("pkgManager", pkgManager)
		viper.SetDefault("appsUrl", "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/app/apps.json")
		viper.SetDefault("pkg.endpoint", "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi-Pkgs.git")

		configFile := filepath.Join(configPath, "config.yaml")
		if err := viper.WriteConfigAs(configFile); err != nil {
			src.PrintError("Error initializing Zoi: %v", err)
			return
		}
		src.PrintSuccess("Zoi configuration file created at %s", configFile)
		src.PrintInfo("\nTo enable the 'zoi pkg' commands, run 'zoi init <your-shell>' (e.g. 'zoi init bash').")
	},
}

func init() {
	rootCmd.AddCommand(initCmd)
}
