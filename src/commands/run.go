package commands

import (
	"fmt"
	"os"
	"strings"

	"github.com/fatih/color"
	"gopkg.in/yaml.v3"
)

const defaultRunYAML = "zoi.yaml"

func RunCommand(commandName string) {
	red := color.New(color.FgRed).SprintFunc()
	green := color.New(color.FgGreen).SprintFunc()
	cyan := color.New(color.FgCyan).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()

	fmt.Printf("%s Looking for '%s' in %s...\n", cyan("ℹ"), commandName, defaultRunYAML)

	data, err := os.ReadFile(defaultRunYAML)
	if err != nil {
		if os.IsNotExist(err) {
			fmt.Printf("%s File '%s' not found in the current directory.\n", red("✗"), defaultRunYAML)
			return
		}
		fmt.Printf("%s Failed to read YAML file '%s': %v\n", red("✗"), defaultRunYAML, err)
		return
	}

	var config RunConfig
	if err := yaml.Unmarshal(data, &config); err != nil {
		fmt.Printf("%s Invalid YAML format in '%s': %v\n", red("✗"), defaultRunYAML, err)
		return
	}

	var commandToRun *RunCommandItem
	for _, cmd := range config.Commands {
		if cmd.Cmd == commandName {
			foundCmd := cmd
			commandToRun = &foundCmd
			break
		}
	}

	if commandToRun == nil {
		fmt.Printf("%s Command '%s' not found in '%s'.\n", red("✗"), commandName, defaultRunYAML)
		fmt.Println(yellow("ℹ Available commands in this file:"))
		for _, cmd := range config.Commands {
			fmt.Printf("  - %s\n", cmd.Cmd)
		}
		return
	}

	fmt.Printf("%s Found command: '%s'\n", green("✓"), commandToRun.Cmd)
	fmt.Printf("%s Running script: '%s'\n", cyan("▸"), commandToRun.Run)

	output, err := runCommandOutput(commandToRun.Run)
	if err != nil {
		fmt.Printf("%s Failed to execute command '%s': %v\n", red("✗"), commandToRun.Cmd, err)
		if output != "" {
			trimmedOutput := strings.TrimRight(output, "\r\n")
			fmt.Printf("Output:\n%s\n", trimmedOutput)
		}
		return
	}

	if output != "" {
		trimmedOutput := strings.TrimRight(output, "\r\n")
		fmt.Println(trimmedOutput)
	}

	fmt.Printf("%s Command '%s' executed successfully.\n", green("✓"), commandToRun.Cmd)
}
