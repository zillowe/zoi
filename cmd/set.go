package cmd

import (
	"errors"
	"zoi/src"

	"github.com/manifoldco/promptui"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
)

var setCmd = &cobra.Command{
	Use:   "set [key] [value]",
	Short: "Set a configuration value, interactively or directly",
	Long: `Set a configuration value in your ~/.zoi/config.yaml file.
- Call with a key and value to set it directly.
- Call without arguments to launch an interactive prompt.`,
	Args: func(cmd *cobra.Command, args []string) error {
		if len(args) == 0 || len(args) == 2 {
			return nil
		}
		return errors.New("this command requires either 0 or 2 arguments")
	},
	Run: func(cmd *cobra.Command, args []string) {
		if len(args) == 2 {
			setKeyValue(args[0], args[1])
			return
		}

		configurableKeys := []string{"os", "arch", "distro", "pkgManager", "appsUrl", "pkg.endpoint"}

		templates := &promptui.SelectTemplates{
			Label:    "{{ . }}",
			Active:   `{{ "›" | green | bold }} {{ . | green | bold }}`,
			Inactive: "  {{ . | faint }}",
			Selected: `{{ "✔" | green | bold }} {{ "Selected key:" | bold }} {{ . | yellow }}`,
		}

		selectPrompt := promptui.Select{
			Label:     "Select a configuration key to change",
			Items:     configurableKeys,
			Templates: templates,
		}

		_, selectedKey, err := selectPrompt.Run()
		if err != nil {
			if errors.Is(err, promptui.ErrInterrupt) {
				src.PrintInfo("Configuration cancelled.")
			}
			return
		}

		currentValue := viper.GetString(selectedKey)

		inputPrompt := promptui.Prompt{
			Label:   "Enter the new value for '" + selectedKey + "'",
			Default: currentValue,
		}

		newValue, err := inputPrompt.Run()
		if err != nil {
			if errors.Is(err, promptui.ErrInterrupt) {
				src.PrintInfo("Configuration cancelled.")
			}
			return
		}

		setKeyValue(selectedKey, newValue)
	},
}

func setKeyValue(key, value string) {
	if key == "appsUrl" && value == "default" {
		value = "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/app/apps.json"
	}

	if key == "pkg.endpoint" && value == "default" {
		value = "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi-Pkgs.git"
	}

	viper.Set(key, value)
	if err := viper.WriteConfig(); err != nil {
		src.PrintError("Error writing configuration: %v", err)
		return
	}

	yellow := src.Yellow()
	src.PrintSuccess("Set %s to %s", key, yellow.Sprint(value))
}

func init() {
	rootCmd.AddCommand(setCmd)
}
