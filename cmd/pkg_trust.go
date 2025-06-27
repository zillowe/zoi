package cmd

import (
	"strings"
	"zoi/src"
	"zoi/src/pkgmanager"

	"github.com/spf13/cobra"
)

var pkgTrustCmd = &cobra.Command{
	Use:   "trust",
	Short: "Install the official Zoi GPG public key to verify packages",
	Long: `Installs the official Zoi GPG public key into your local GPG keyring.

This key is used to verify the digital signatures of all official package
recipes, ensuring they have not been tampered with. This is a critical
security step and should be done after installing Zoi.`,
	Run: func(cmd *cobra.Command, args []string) {
		src.PrintInfo("Attempting to install the official Zoi PGP signing key...")
		if !src.CheckCommand("gpg --version") {
			src.PrintError("GPG command not found. Please install GnuPG to use signature verification.")
			return
		}
		if err := pkgmanager.ImportZoiSigningKey(); err != nil {
			if strings.Contains(err.Error(), "key already known") {
				src.PrintSuccess("Zoi signing key is already present in your GPG keyring.")
				return
			}
			src.PrintError("Failed to import key: %v", err)
			return
		}
		src.PrintSuccess("\n✅ Official Zoi GPG key successfully imported!")
		src.PrintInfo("You can now securely install and verify packages.")
	},
}

func init() {
	pkgCmd.AddCommand(pkgTrustCmd)
}
