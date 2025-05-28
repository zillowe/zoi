package commands

import (
	"fmt"

	"github.com/fatih/color"
)

func PrintUsage() {
	cyan := color.New(color.FgCyan).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()
	green := color.New(color.FgGreen).SprintFunc()

	fmt.Printf("%s - Universal Environment Setup Tool\n\n", cyan("Zoi"))
	fmt.Printf("%s\n", yellow("Usage:"))
	fmt.Printf("  zoi %s\n\n", green("<command> [arguments...]"))

	fmt.Printf("%s\n", cyan("Available Commands:"))
	fmt.Printf("  %s               Display project and environment information from zoi.yaml, check packages, and run setup commands\n", green("env"))
	fmt.Printf("  %s           Show Zoi version information\n", green("version"))
	fmt.Printf("  %s             Display details and information about Zoi\n", green("about"))
	fmt.Printf("  %s              Display details and information about the system\n", green("info"))
	fmt.Printf("  %s             Verify system health and check for tool requirements\n", green("check"))
	fmt.Printf("  %s                (Still in development) Manage language versions (e.g. Go, Python) via subcommands\n", green("vm"))
	fmt.Printf("  %s           Install system-wide packages or tools (e.g. 'zoi install node@18')\n", green("install"))
	fmt.Printf("  %s         Uninstall system-wide packages or tools (e.g. 'zoi uninstall node')\n", green("uninstall"))
	fmt.Printf("  %s            Create a new application from a predefined template\n", green("create"))
	fmt.Printf("  %s              Generate an application from a local YAML configuration file\n", green("make"))
	fmt.Printf("  %s               Execute a command defined in a local zoi.yaml file\n", green("run"))
	fmt.Printf("  %s               Manage Zoi configuration settings (e.g. 'zoi set appsUrl default')\n", green("set"))
	fmt.Printf("  %s            Check for and apply updates to Zoi itself\n", green("update"))
	fmt.Printf("  %s              Show this help message\n\n", green("help"))

	fmt.Printf("%s\n", cyan("Flags:"))
	fmt.Printf("  %s, %s     Show Zoi version information\n", green("-v"), green("--version"))

	fmt.Printf("\n%s Run %s for more details on a specific command.\n", yellow("Hint:"), green("'zoi <command> --help'"))
}
