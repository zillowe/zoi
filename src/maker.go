package src

import (
	"fmt"
	"os"

	"gopkg.in/yaml.v2"
)

type MakerConfig struct {
	AppName       string         `yaml:"appName"`
	Packages      []MakerPackage `yaml:"packages"`
	CreateCommand string         `yaml:"createCommand"`
}

type MakerPackage struct {
	Name    string      `yaml:"name"`
	Check   string      `yaml:"check"`
	Install interface{} `yaml:"install"`
}

func LoadMakerConfig(filename string) (*MakerConfig, error) {
	if _, err := os.Stat(filename); os.IsNotExist(err) {
		return nil, fmt.Errorf("configuration file not found: %s", filename)
	}

	data, err := os.ReadFile(filename)
	if err != nil {
		return nil, fmt.Errorf("error reading %s: %w", filename, err)
	}

	var config MakerConfig
	err = yaml.Unmarshal(data, &config)
	if err != nil {
		return nil, fmt.Errorf("error parsing %s: %w", filename, err)
	}

	return &config, nil
}
