package pkgmanager

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"zoi/src"

	"golang.org/x/crypto/openpgp"
)

func verifySignature(signedData, signatureData []byte) (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", fmt.Errorf("could not get home directory: %w", err)
	}
	keyringPath := filepath.Join(home, ".gnupg", "pubring.kbx")
	if _, err := os.Stat(keyringPath); os.IsNotExist(err) {
		keyringPath = filepath.Join(home, ".gnupg", "pubring.gpg")
		if _, err := os.Stat(keyringPath); os.IsNotExist(err) {
			return "", fmt.Errorf("GPG keyring not found. Please run 'zoi pkg trust' or ensure GPG is set up")
		}
	}

	keyringFile, err := os.Open(keyringPath)
	if err != nil {
		return "", fmt.Errorf("could not open GPG keyring at %s: %w", keyringPath, err)
	}
	defer keyringFile.Close()

	keyring, err := openpgp.ReadKeyRing(keyringFile)
	if err != nil {
		return "", fmt.Errorf("failed to parse GPG keyring: %w", err)
	}

	signer, err := openpgp.CheckArmoredDetachedSignature(keyring, bytes.NewReader(signedData), bytes.NewReader(signatureData))
	if err != nil {
		return "", fmt.Errorf("invalid signature: %w", err)
	}

	if signer == nil {
		return "", fmt.Errorf("could not find a trusted key in your keyring to verify this signature")
	}

	signerName := ""
	for _, identity := range signer.Identities {
		signerName = identity.Name
		break
	}

	return signerName, nil
}

func VerifyRecipeSignature(recipeContent []byte, handle, repoURL string, providerHint Provider) error {
	pkgData, err := getRemotePackageData(handle, repoURL, providerHint)
	if err != nil {
		return err
	}
	if pkgData.SigFile == "" {
		return fmt.Errorf("no signature file ('sigFile') specified for recipe '%s'", handle)
	}

	sigContent, err := getRawFileContent(repoURL, providerHint, pkgData.SigFile)
	if err != nil {
		return fmt.Errorf("could not download recipe signature file: %w", err)
	}

	signerName, err := verifySignature(recipeContent, sigContent)
	if err != nil {
		return err
	}

	src.PrintSuccess("Recipe signature verified by: %s", signerName)
	return nil
}

func getRemotePackageData(handle, repoURL string, providerHint Provider) (*Package, error) {
	dbContent, err := getRawFileContent(repoURL, providerHint, "pkgs.json")
	if err != nil {
		return nil, fmt.Errorf("could not find pkgs.json in remote database at %s", repoURL)
	}

	var dbConfig PackageManagerConfig
	if err := json.Unmarshal(dbContent, &dbConfig); err != nil {
		return nil, fmt.Errorf("failed to parse remote pkgs.json: %w", err)
	}

	pkgData, ok := dbConfig.Packages[handle]
	if !ok {
		return nil, fmt.Errorf("package '%s' not found in remote database: %s", handle, repoURL)
	}

	return &pkgData, nil
}

func getRawFileContent(repoURL string, providerHint Provider, filePathInRepo string) ([]byte, error) {
	rawURL, err := getRawFileURLForPath(repoURL, providerHint, filePathInRepo)
	if err != nil {
		return nil, err
	}

	resp, err := http.Get(rawURL)
	if err != nil || resp.StatusCode != http.StatusOK {
		rawURL, _ = getRawFileURLForPath(strings.Replace(repoURL, "/main/", "/master/", 1), providerHint, filePathInRepo)
		resp, err = http.Get(rawURL)
		if err != nil || resp.StatusCode != http.StatusOK {
			return nil, fmt.Errorf("could not download file %s: %v", filePathInRepo, err)
		}
	}
	defer resp.Body.Close()
	return io.ReadAll(resp.Body)
}

func getRawFileURLForPath(repoURL string, providerHint Provider, filePathInRepo string) (string, error) {
	parsedURL, err := url.Parse(repoURL)
	if err != nil {
		return "", fmt.Errorf("invalid repository URL: %w", err)
	}

	provider := providerHint
	if provider == "" {
		provider = DetectProvider(parsedURL.Host)
	}
	if provider == ProviderUnknown {
		return "", fmt.Errorf("unsupported Git provider for URL '%s'", repoURL)
	}

	repoPath := strings.TrimSuffix(parsedURL.Path, ".git")

	branch := "main"

	switch provider {
	case ProviderGitHub:
		return fmt.Sprintf("https://raw.githubusercontent.com%s/%s/%s", repoPath, branch, filePathInRepo), nil
	case ProviderGitLab:
		return fmt.Sprintf("https://%s%s/-/raw/%s/%s", parsedURL.Host, repoPath, branch, filePathInRepo), nil
	case ProviderGitea, ProviderForgejo:
		return fmt.Sprintf("https://%s%s/raw/branch/%s/%s", parsedURL.Host, repoPath, branch, filePathInRepo), nil
	}
	return "", fmt.Errorf("cannot construct raw file URL for provider: %s", provider)
}
