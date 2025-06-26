package pkgmanager

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"

	"gopkg.in/yaml.v2"
)

type Provider string

const (
	ProviderGitHub  Provider = "github"
	ProviderGitLab  Provider = "gitlab"
	ProviderGitea   Provider = "gitea"
	ProviderForgejo Provider = "forgejo"
	ProviderUnknown Provider = "unknown"
)

func FetchRawRecipeFromURL(repoURL string, providerHint Provider) ([]byte, error) {
	parsedURL, err := url.Parse(repoURL)
	if err != nil {
		return nil, fmt.Errorf("invalid repository URL: %w", err)
	}

	provider := providerHint
	if provider == "" {
		provider = DetectProvider(parsedURL.Host)
	}

	if provider == ProviderUnknown {
		return nil, fmt.Errorf("unsupported Git provider for URL '%s'. Please specify one with a flag", repoURL)
	}

	path := strings.TrimSuffix(parsedURL.Path, ".git")

	rawURL, err := getRawFileURL(provider, parsedURL.Host, path, "main", "zoi.yaml")
	if err != nil {
		return nil, err
	}
	resp, err := http.Get(rawURL)

	if err != nil || resp.StatusCode != http.StatusOK {
		rawURL, _ = getRawFileURL(provider, parsedURL.Host, path, "master", "zoi.yaml")
		resp, err = http.Get(rawURL)
		if err != nil || resp.StatusCode != http.StatusOK {
			return nil, fmt.Errorf("could not find zoi.yaml in 'main' or 'master' branch at %s", parsedURL.Host+path)
		}
	}
	defer resp.Body.Close()

	return io.ReadAll(resp.Body)
}

func FetchRecipeFromURL(repoURL string, providerHint Provider) (*PackageRecipe, error) {
	body, err := FetchRawRecipeFromURL(repoURL, providerHint)
	if err != nil {
		return nil, err
	}

	var recipe PackageRecipe
	if err := yaml.Unmarshal(body, &recipe); err != nil {
		return nil, fmt.Errorf("failed to parse remote zoi.yaml: %w", err)
	}

	return &recipe, nil
}

func FetchRecipeFromRemoteDB(handle, repoURL string, providerHint Provider) ([]byte, error) {
	parsedURL, err := url.Parse(repoURL)
	if err != nil {
		return nil, fmt.Errorf("invalid repository URL: %w", err)
	}

	provider := providerHint
	if provider == "" {
		provider = DetectProvider(parsedURL.Host)
	}
	if provider == ProviderUnknown {
		return nil, fmt.Errorf("unsupported Git provider for URL '%s'", repoURL)
	}

	repoPath := strings.TrimSuffix(parsedURL.Path, ".git")

	dbURL, _ := getRawFileURL(provider, parsedURL.Host, repoPath, "main", "pkgs.json")
	resp, err := http.Get(dbURL)
	if err != nil || resp.StatusCode != http.StatusOK {
		dbURL, _ = getRawFileURL(provider, parsedURL.Host, repoPath, "master", "pkgs.json")
		resp, err = http.Get(dbURL)
		if err != nil || resp.StatusCode != http.StatusOK {
			return nil, fmt.Errorf("could not find pkgs.json in remote database at %s", repoURL)
		}
	}
	defer resp.Body.Close()

	var dbConfig PackageManagerConfig
	if err := json.NewDecoder(resp.Body).Decode(&dbConfig); err != nil {
		return nil, fmt.Errorf("failed to parse remote pkgs.json: %w", err)
	}

	pkgData, ok := dbConfig.Packages[handle]
	if !ok {
		return nil, fmt.Errorf("package '%s' not found in remote database: %s", handle, repoURL)
	}

	recipeURL, _ := getRawFileURL(provider, parsedURL.Host, repoPath, "main", pkgData.PkgFile)
	resp, err = http.Get(recipeURL)
	if err != nil || resp.StatusCode != http.StatusOK {
		recipeURL, _ = getRawFileURL(provider, parsedURL.Host, repoPath, "master", pkgData.PkgFile)
		resp, err = http.Get(recipeURL)
		if err != nil || resp.StatusCode != http.StatusOK {
			return nil, fmt.Errorf("found package in db, but failed to fetch recipe file: %s", pkgData.PkgFile)
		}
	}
	defer resp.Body.Close()

	return io.ReadAll(resp.Body)
}

func getRawFileURL(provider Provider, host, repoPath, branch, filePathInRepo string) (string, error) {
	switch provider {
	case ProviderGitHub:
		return fmt.Sprintf("https://raw.githubusercontent.com%s/%s/%s", repoPath, branch, filePathInRepo), nil
	case ProviderGitLab:
		return fmt.Sprintf("https://%s%s/-/raw/%s/%s", host, repoPath, branch, filePathInRepo), nil
	case ProviderGitea, ProviderForgejo:
		return fmt.Sprintf("https://%s%s/raw/branch/%s/%s", host, repoPath, branch, filePathInRepo), nil
	}
	return "", fmt.Errorf("cannot construct raw file URL for provider: %s", provider)
}

func DetectProvider(host string) Provider {
	if strings.Contains(host, "github.com") {
		return ProviderGitHub
	}
	if strings.Contains(host, "gitlab.com") {
		return ProviderGitLab
	}
	if strings.Contains(host, "codeberg.org") {
		return ProviderGitea
	}
	return ProviderUnknown
}
