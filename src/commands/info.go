package commands

import (
	"fmt"
	"runtime"

	"github.com/fatih/color"
)

func InfoCommand() {
	cyan := color.New(color.FgCyan).SprintFunc()
	green := color.New(color.FgGreen).SprintFunc()

	fmt.Println(cyan("Detected environment:"))
	fmt.Printf("- OS:              %s\n", green(getOS()))
	distro := getLinuxDistro()
	if distro != "unknown" && runtime.GOOS == "linux" {
		fmt.Printf("- Linux Distro:    %s\n", green(distro))
	}
	fmt.Printf("- Architecture:    %s\n", green(getArch()))
	pm, err := detectPackageManager()
	if err == nil {
		fmt.Printf("- Package Manager: %s\n", green(pm.Name))
	} else {
		fmt.Printf("- Package Manager: %s\n", color.YellowString("Not detected or unsupported"))
	}
}
