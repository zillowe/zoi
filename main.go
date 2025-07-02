package main

import (
	"zoi/cmd"
)

var (
	VerBranch = "Prod."
	VerStatus = "Beta"
	VerNumber = "2.0.0"
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
