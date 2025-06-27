package main

import (
	"zoi/cmd"
)

var (
	VerBranch = "Prod."
	VerStatus = "Release"
	VerNumber = "1.0.1"
	VerCommit = "dev"
)

func main() {
	cmd.Execute(cmd.VersionInfo{
		Branch: VerBranch,
		Status: VerStatus,
		Number: VerNumber,
		Commit: VerCommit,
	})
}
