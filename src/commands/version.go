package commands

import (
	"fmt"
	"runtime"

	"github.com/fatih/color"
)

func VersionCommand(VerBranch, VerStatus, VerNumber, VerCommit string) {
	cyan := color.New(color.FgCyan).SprintFunc()
	green := color.New(color.FgGreen).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()

	versionString := fmt.Sprintf("%s %s %s", VerBranch, VerStatus, VerNumber)
	fmt.Printf("%s %s\n",
		cyan("Zoi"),
		green(versionString),
	)

	runtimeInfo := fmt.Sprintf("%s/%s", runtime.GOOS, runtime.GOARCH)
	distro := getLinuxDistro()
	if distro != "unknown" && runtime.GOOS == "linux" {
		runtimeInfo = fmt.Sprintf("%s/%s/%s", runtime.GOOS, distro, runtime.GOARCH)
	}
	fmt.Printf("Runtime: %s\n", green(runtimeInfo))

	if VerCommit != "" && VerCommit != "dev" {
		fmt.Printf("Commit: %s\n", yellow(VerCommit))
	} else {
		fmt.Printf("Commit: %s\n", yellow("Not set (dev build)"))
	}
}
