package src

import (
	"fmt"
	"os"

	"gopkg.in/yaml.v2"
)

type ProjectConfig struct {
	Name         string        `yaml:"name"`
	Packages     []PackageSpec `yaml:"packages"`
	Commands     []CommandSpec `yaml:"commands"`
	Environments []EnvSpec     `yaml:"environments"`
}

type PackageSpec struct {
	Name  string `yaml:"name"`
	Check string `yaml:"check"`
}

type CommandSpec struct {
	Cmd string `yaml:"cmd"`
	Run string `yaml:"run"`
}

type EnvSpec struct {
	Name string   `yaml:"name"`
	Cmd  string   `yaml:"cmd"`
	Run  []string `yaml:"run"`
}

func LoadProjectConfig() (*ProjectConfig, error) {
	filename := "zoi.yaml"
	if _, err := os.Stat(filename); os.IsNotExist(err) {
		return nil, fmt.Errorf("no %s file found in the current directory", filename)
	}

	data, err := os.ReadFile(filename)
	if err != nil {
		return nil, fmt.Errorf("error reading %s: %w", filename, err)
	}

	var config ProjectConfig
	err = yaml.Unmarshal(data, &config)
	if err != nil {
		return nil, fmt.Errorf("error parsing %s: %w", filename, err)
	}

	return &config, nil
}
