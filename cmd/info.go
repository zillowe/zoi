package cmd

import (
	"fmt"
	"zoi/src"

	"github.com/spf13/cobra"
	"github.com/spf13/viper"
)

var infoCmd = &cobra.Command{
	Use:   "info",
	Short: "Display details and information about the system",
	Run: func(cmd *cobra.Command, args []string) {
		osName := viper.GetString("os")
		arch := viper.GetString("arch")
		distro := viper.GetString("distro")
		pkgManager := viper.GetString("pkgManager")

		appsUrl := viper.GetString("appsUrl")
		configFile := viper.ConfigFileUsed()

		yellow := src.Yellow()

		src.PrintHighlight("--- System Information ---")
		osVal := "Not Found"
		if osName != "" {
			osVal = osName
		}
		fmt.Printf("OS:               %s\n", yellow.Sprint(osVal))
		archVal := "Not Found"
		if arch != "" {
			archVal = arch
		}
		fmt.Printf("Architecture:     %s\n", yellow.Sprint(archVal))

		if osName == "linux" {
			distroVal := "Unknown"
			if distro != "" {
				distroVal = distro
			}
			fmt.Printf("Distribution:     %s\n", yellow.Sprint(distroVal))
		}

		pkgVal := "Not Found"
		if pkgManager != "" {
			pkgVal = pkgManager
		}
		fmt.Printf("Package Manager:  %s\n", yellow.Sprint(pkgVal))

		fmt.Println()
		src.PrintHighlight("--- Zoi Configuration ---")
		if configFile != "" {
			fmt.Printf("Config File:  %s\n", yellow.Sprint(configFile))
		} else {
			fmt.Printf("Config File:  %s\n", yellow.Sprint("Not found (using defaults)"))
		}
		fmt.Printf("Apps URL:     %s\n", yellow.Sprint(appsUrl))

	},
}

func init() {
	rootCmd.AddCommand(infoCmd)
}
