package cmd

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"
	"github.com/spf13/viper"
)

type VersionInfo struct {
	Branch string
	Status string
	Number string
	Commit string
}

var rootCmd = &cobra.Command{
	Use:          "zoi",
	Short:        "Zoi - Universal Package Manager & Environment Setup Tool.",
	SilenceUsage: true,
}

var currentVersionInfo VersionInfo

func Execute(versionInfo VersionInfo) {
	currentVersionInfo = versionInfo

	fullVersion := fmt.Sprintf("%s %s %s %s",
		versionInfo.Branch, versionInfo.Status, versionInfo.Number, versionInfo.Commit)
	rootCmd.Version = fullVersion

	if err := rootCmd.Execute(); err != nil {
		os.Exit(1)
	}
}

func init() {
	cobra.OnInitialize(initConfig)
}

func initConfig() {
	home, err := os.UserHomeDir()
	cobra.CheckErr(err)

	configPath := home + "/.zoi"
	viper.AddConfigPath(configPath)
	viper.SetConfigName("config")
	viper.SetConfigType("yaml")

	viper.AutomaticEnv()

	if err := viper.ReadInConfig(); err != nil {
		if _, ok := err.(viper.ConfigFileNotFoundError); !ok {
			cobra.CheckErr(err)
		}
	}
}
