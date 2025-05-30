package commands

import (
	"fmt"

	"github.com/fatih/color"
)

func AboutCommand() {
	cyan := color.New(color.FgCyan).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()

	fmt.Println(cyan("About Zoi:"))
	fmt.Printf("  Zoi is a universal environment setup tool for developers.\n")
	fmt.Printf("\n")
	fmt.Printf("  Zoi is a part of the Zillowe Development Suite (ZDS)\n")
	fmt.Printf("\n")
	fmt.Printf("  Created by Zillowe Foundation > Zusty\n")
	fmt.Printf("  Hosted on %s\n", yellow("https://Codeberg.org/Zusty/Zoi"))
}
