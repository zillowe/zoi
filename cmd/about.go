package cmd

import (
	"fmt"
	"zoi/src"

	"github.com/spf13/cobra"
)

const (
	author   = "Zusty < Zillowe Foundation"
	homepage = "https://codeberg.org/Zusty/Zoi"
	license  = "ZFPL-1.0"
)

var aboutCmd = &cobra.Command{
	Use:   "about",
	Short: "Display details and information about Zoi",
	Run: func(cmd *cobra.Command, args []string) {
		yellow := src.Yellow()

		fmt.Println()
		src.PrintHighlight(`
   ██████████████████ ██████████████ ██████████ 
   ██░░░░░░░░░░░░░░██ ██░░░░░░░░░░██ ██░░░░░░██ 
   ████████████░░░░██ ██░░██████░░██ ████░░████ 
           ████░░████ ██░░██  ██░░██   ██░░██   
         ████░░████   ██░░██  ██░░██   ██░░██   
       ████░░████     ██░░██  ██░░██   ██░░██   
     ████░░████       ██░░██  ██░░██   ██░░██   
   ████░░████         ██░░██  ██░░██   ██░░██   
   ██░░░░████████████ ██░░██████░░██ ████░░████ 
   ██░░░░░░░░░░░░░░██ ██░░░░░░░░░░██ ██░░░░░░██ 
   ██████████████████ ██████████████ ██████████ 
		`)
		fmt.Println()

		fmt.Printf("  %s\n\n", cmd.Root().Short)

		fmt.Printf("  %-12s %s\n", "Version:", yellow.Sprint(cmd.Root().Version))
		fmt.Printf("  %-12s %s\n", "Author:", yellow.Sprint(author))
		fmt.Printf("  %-12s %s\n", "Homepage:", yellow.Sprint(homepage))
		fmt.Printf("  %-12s %s\n", "License:", yellow.Sprint(license))

		fmt.Println()
	},
}

func init() {
	rootCmd.AddCommand(aboutCmd)
}
