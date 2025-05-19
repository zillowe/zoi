package commands

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/fatih/color"
	"gopkg.in/yaml.v3"
)

func loadConfig() (*GlobalConfig, error) {
	cfgPath, err := getConfigPath()
	if err != nil {
		return nil, err
	}

	cfg := &GlobalConfig{AppsURL: defaultAppsURL}

	data, err := os.ReadFile(cfgPath)
	if os.IsNotExist(err) {
		return cfg, nil
	}
	if err != nil {
		return nil, err
	}

	if err := yaml.Unmarshal(data, cfg); err != nil {
		return nil, fmt.Errorf("failed to parse config file %s: %w", cfgPath, err)
	}

	if cfg.AppsURL == "" {
		cfg.AppsURL = defaultAppsURL
	}

	return cfg, nil
}

func saveConfig(cfg *GlobalConfig) error {
	cfgPath, err := getConfigPath()
	if err != nil {
		return err
	}

	data, err := yaml.Marshal(cfg)
	if err != nil {
		return fmt.Errorf("failed to marshal config data: %w", err)
	}

	configDirPath := filepath.Dir(cfgPath)
	if err := os.MkdirAll(configDirPath, 0750); err != nil {
		return fmt.Errorf("failed to create config directory %s: %w", configDirPath, err)
	}

	return os.WriteFile(cfgPath, data, 0640)
}

func getAppsURL() string {
	cfg, err := loadConfig()
	if err != nil {
		return defaultAppsURL
	}
	return cfg.AppsURL
}

func getConfigValue(cfg *GlobalConfig, key string) string {
	switch key {
	case "appsUrl":
		return cfg.AppsURL
	default:
		return "[unknown key]"
	}
}

func SetCommand(key, value string) {
	red := color.New(color.FgRed).SprintFunc()
	green := color.New(color.FgGreen).SprintFunc()
	cyan := color.New(color.FgCyan).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()

	allowedKeys := map[string]bool{"appsUrl": true}

	if !allowedKeys[key] {
		fmt.Printf("%s Unknown or restricted config key: %s\n", red("✗"), key)
		fmt.Printf("%s Allowed keys: appsUrl\n", yellow("ℹ"))
		return
	}

	cfg, err := loadConfig()
	if err != nil {
		fmt.Printf("%s Failed to load config: %v\n", red("✗"), err)
		return
	}

	switch key {
	case "appsUrl":
		if value == "default" {
			cfg.AppsURL = defaultAppsURL
			fmt.Printf("%s Resetting apps URL to default.\n", cyan("ℹ"))
		} else {
			cfg.AppsURL = value
		}
	}

	if err := saveConfig(cfg); err != nil {
		fmt.Printf("%s Failed to save config: %v\n", red("✗"), err)
		return
	}

	fmt.Printf("%s Config key '%s' updated to: %s\n",
		green("✓"), key, cyan(getConfigValue(cfg, key)))
}
