package cmd

import (
	"zoi/src"

	"github.com/spf13/cobra"
	"github.com/spf13/viper"
)

var setCmd = &cobra.Command{
	Use:   "set [key] [value]",
	Short: "Set a configuration value",
	Args:  cobra.ExactArgs(2),
	Run: func(cmd *cobra.Command, args []string) {
		key := args[0]
		value := args[1]
		if key == "appsUrl" && value == "default" {
			value = "https://zusty.codeberg.page/Zoi/@main/app/apps.json"
		}
		viper.Set(key, value)
		if err := viper.WriteConfig(); err != nil {
			src.PrintError("Error setting configuration: %v", err)
			return
		}
		src.PrintSuccess("Set %s to %s", key, value)
	},
}

func init() {
	rootCmd.AddCommand(setCmd)
}
