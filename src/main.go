package main

import (
	"fmt"
	"os"
	"zoi/src/commands"

	"github.com/fatih/color"
)

var (
	VerBranch = "Prod."
	VerStatus = "Alpha"
	VerNumber = "1.1.0"
	VerCommit = "dev"
)

var requiredTools = map[string]string{
	"git": "<=2.25.0",
}

func main() {
	if len(os.Args) < 2 || os.Args[1] == "help" {
		commands.PrintUsage()
		return
	}

	if os.Args[1] == "--version" || os.Args[1] == "-v" {
		commands.VersionCommand(VerBranch, VerStatus, VerNumber, VerCommit)
		return
	}

	command := os.Args[1]
	args := os.Args[2:]

	switch command {
	case "create":
		if len(args) < 2 {
			fmt.Println(color.YellowString("Usage: zoi create <app-template> <app-name>"))
			return
		}
		commands.CreateCommand(args[0], args[1])
	case "make":
		if len(args) < 1 {
			fmt.Println(color.YellowString("Usage: zoi make <config.yaml>"))
			return
		}
		commands.MakeCommand(args[0])
	case "set":
		if len(args) < 2 {
			fmt.Println(color.YellowString("Usage: zoi set <key> <value>"))
			fmt.Println(color.CyanString("       Available keys: appsUrl"))
			return
		}
		commands.SetCommand(args[0], args[1])
	case "install":
		if len(args) < 1 {
			fmt.Println(color.YellowString("Usage: zoi install <package>[@version]"))
			return
		}
		commands.InstallCommand(args[0])
	case "uninstall":
		if len(args) < 1 {
			fmt.Println(color.YellowString("Usage: zoi uninstall <package>"))
			return
		}
		commands.UninstallCommand(args[0])
	case "vm":
		commands.VmCommand(args)
	case "check":
		if len(args) > 0 {
			fmt.Println(color.YellowString("Usage: zoi check (no arguments expected)"))
			return
		}
		commands.CheckCommand(requiredTools)
	case "env":
		if len(args) > 1 {
			fmt.Println(color.YellowString("Usage: zoi env <environment-name>"))
			return
		}
		commands.EnvCommand(args)
	case "version":
		if len(args) > 0 {
			fmt.Println(color.YellowString("Usage: zoi version (no arguments expected)"))
			return
		}
		commands.VersionCommand(VerBranch, VerStatus, VerNumber, VerCommit)
	case "about":
		if len(args) > 0 {
			fmt.Println(color.YellowString("Usage: zoi about (no arguments expected)"))
			return
		}
		commands.AboutCommand()
	case "info":
		if len(args) > 0 {
			fmt.Println(color.YellowString("Usage: zoi info (no arguments expected)"))
			return
		}
		commands.InfoCommand()
	case "update":
		if len(args) > 0 {
			fmt.Println(color.YellowString("Usage: zoi update (no arguments expected)"))
			return
		}
		commands.UpdateCommand(VerBranch, VerStatus, VerNumber)
	case "run":
		if len(args) < 1 {
			fmt.Println(color.YellowString("Usage: zoi run <command-name>"))
			return
		}
		commands.RunCommand(args[0])
	default:
		commands.NotFoundCommand()
	}
}
