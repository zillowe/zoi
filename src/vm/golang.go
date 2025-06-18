package vm

import (
	"archive/tar"
	"compress/gzip"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"

	"zoi/src"

	"github.com/PuerkitoBio/goquery"
)

type GoVersionEntry struct {
	Version string   `json:"version"`
	Stable  bool     `json:"stable"`
	Files   []GoFile `json:"files"`
}
type GoFile struct {
	Filename string `json:"filename"`
	OS       string `json:"os"`
	Arch     string `json:"arch"`
	Version  string `json:"version"`
	SHA256   string `json:"sha256"`
	Kind     string `json:"kind"`
}

func GetGoVersions() ([]GoVersionEntry, error) {
	resp, err := http.Get("https://go.dev/dl/?mode=json")
	if err != nil {
		return nil, fmt.Errorf("failed to fetch go versions: %w", err)
	}
	defer resp.Body.Close()

	var versions []GoVersionEntry
	if err := json.NewDecoder(resp.Body).Decode(&versions); err != nil {
		return nil, fmt.Errorf("failed to parse go versions JSON: %w", err)
	}
	return versions, nil
}

func probeForArchivedGoVersion(version, osName, archName string) (*GoFile, error) {
	src.PrintInfo("Version not found in active releases. Probing archive for go%s...", version)

	fileName := fmt.Sprintf("go%s.%s-%s.tar.gz", version, osName, archName)
	url := "https://go.dev/dl/" + fileName

	resp, err := http.Head(url)
	if err != nil {
		return nil, fmt.Errorf("failed to probe archive: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("version go%s not found in archive for %s/%s (HTTP status: %s)", version, osName, archName, resp.Status)
	}

	src.PrintSuccess("Found go%s in archive!", version)
	return &GoFile{
		Filename: fileName,
		OS:       osName,
		Arch:     archName,
		Version:  "go" + version,
		Kind:     "archive",
		SHA256:   "",
	}, nil
}

func scrapeForArchivedGo(version, osName, archName string) (*GoFile, error) {
	src.PrintInfo("Version not found in active releases. Scraping archive page for go%s...", version)
	url := "https://go.dev/dl/"

	resp, err := http.Get(url)
	if err != nil {
		return nil, fmt.Errorf("failed to fetch archive page: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("failed to get archive page, status: %s", resp.Status)
	}

	doc, err := goquery.NewDocumentFromReader(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to parse archive page HTML: %w", err)
	}

	targetFilename := fmt.Sprintf("go%s.%s-%s.tar.gz", version, osName, archName)
	var foundFile *GoFile

	doc.Find("table.downloadtable tbody tr").EachWithBreak(func(i int, s *goquery.Selection) bool {
		filenameCell := s.Find("td.filename a")
		if filenameCell.Text() == targetFilename {
			checksum := s.Find("td tt").Text()

			foundFile = &GoFile{
				Filename: targetFilename,
				OS:       osName,
				Arch:     archName,
				Version:  "go" + version,
				Kind:     "archive",
				SHA256:   checksum,
			}
			return false
		}
		return true
	})

	if foundFile == nil {
		return nil, fmt.Errorf("could not find download details for %s on the archive page", targetFilename)
	}

	src.PrintSuccess("Found go%s in archive with checksum!", version)
	return foundFile, nil
}

func FindGoVersion(version, osName, archName string, availableVersions []GoVersionEntry) (*GoFile, error) {
	targetVersionStr := "go" + version
	for _, entry := range availableVersions {
		if entry.Version == targetVersionStr {
			for _, file := range entry.Files {
				if file.OS == osName && file.Arch == archName && file.Kind == "archive" {
					return &file, nil
				}
			}
		}
	}

	return scrapeForArchivedGo(version, osName, archName)
}

func FindLatestGoVersion(osName, archName string, availableVersions []GoVersionEntry) (*GoFile, error) {
	for _, entry := range availableVersions {
		if entry.Stable {
			for _, file := range entry.Files {
				if file.OS == osName && file.Arch == archName && file.Kind == "archive" {
					return &file, nil
				}
			}
		}
	}
	return nil, fmt.Errorf("no suitable stable go version found for %s/%s", osName, archName)
}

func DownloadAndExtractGo(file *GoFile, destPath string) error {
	stripPrefix := "go/"

	downloadURL := "https://go.dev/dl/" + file.Filename
	src.PrintInfo("Downloading from %s...", downloadURL)

	resp, err := http.Get(downloadURL)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("download failed with status: %s", resp.Status)
	}

	tempArchive, err := os.CreateTemp("", "zoi-go-*.tar.gz")
	if err != nil {
		return fmt.Errorf("failed to create temp file: %w", err)
	}
	defer os.Remove(tempArchive.Name())

	hash := sha256.New()
	tee := io.TeeReader(resp.Body, hash)

	if _, err := io.Copy(tempArchive, tee); err != nil {
		return fmt.Errorf("failed to save download: %w", err)
	}

	tempArchive.Close()

	if file.SHA256 != "" {
		actualChecksum := hex.EncodeToString(hash.Sum(nil))
		if actualChecksum != file.SHA256 {
			return fmt.Errorf("checksum mismatch! expected %s, got %s", file.SHA256, actualChecksum)
		}
		src.PrintSuccess("Checksum verified successfully.")
	} else {
		src.PrintInfo("%s", "WARNING: No checksum available for this version. Verification skipped.")
	}

	archiveFile, err := os.Open(tempArchive.Name())
	if err != nil {
		return fmt.Errorf("failed to reopen temp archive for extraction: %w", err)
	}
	defer archiveFile.Close()

	src.PrintInfo("Extracting to %s...", destPath)

	gzr, err := gzip.NewReader(archiveFile)
	if err != nil {
		return err
	}
	defer gzr.Close()

	tr := tar.NewReader(gzr)

	if err := os.MkdirAll(destPath, 0755); err != nil {
		return err
	}

	for {
		header, err := tr.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			return err
		}

		targetPath := strings.TrimPrefix(header.Name, stripPrefix)
		if targetPath == "" {
			continue
		}
		target := filepath.Join(destPath, targetPath)

		switch header.Typeflag {
		case tar.TypeDir:
			if err := os.MkdirAll(target, 0755); err != nil {
				return err
			}
		case tar.TypeReg:
			if err := os.MkdirAll(filepath.Dir(target), 0755); err != nil {
				return err
			}

			outFile, err := os.OpenFile(target, os.O_CREATE|os.O_RDWR, os.FileMode(header.Mode))
			if err != nil {
				return err
			}
			if _, err := io.Copy(outFile, tr); err != nil {
				outFile.Close()
				return err
			}
			outFile.Close()
		case tar.TypeSymlink:
			if err := os.Symlink(header.Linkname, target); err != nil {
				return err
			}
		}
	}
	return nil
}

func UpdateSymlink(targetPath, linkPath string) error {
	if err := os.MkdirAll(filepath.Dir(linkPath), 0755); err != nil {
		return err
	}
	if _, err := os.Lstat(linkPath); err == nil {
		if err := os.Remove(linkPath); err != nil {
			return fmt.Errorf("failed to remove existing symlink at %s: %w", linkPath, err)
		}
	}
	return os.Symlink(targetPath, linkPath)
}

func UpdateShellProfile() error {
	home, err := os.UserHomeDir()
	if err != nil {
		return err
	}

	zoiBinPath := filepath.Join(home, ".zoi", "vm", "go", "global", "bin")
	exportLine := fmt.Sprintf("export PATH=\"%s:$PATH\"", zoiBinPath)
	commentLine := "# Added by Zoi VM to manage Go versions"

	profileFile := ""
	shell := os.Getenv("SHELL")
	if strings.Contains(shell, "zsh") {
		profileFile = filepath.Join(home, ".zshrc")
	} else if strings.Contains(shell, "bash") {
		profileFile = filepath.Join(home, ".bashrc")
	} else {
		profileFile = filepath.Join(home, ".profile")
	}

	if _, err := os.Stat(profileFile); os.IsNotExist(err) {
		src.PrintInfo("Creating shell profile: %s", profileFile)
		content := fmt.Sprintf("%s\n%s\n", commentLine, exportLine)
		if err := os.WriteFile(profileFile, []byte(content), 0644); err != nil {
			return err
		}
		src.PrintSuccess("Profile updated! Please restart your shell or run 'source %s'", profileFile)
		return nil
	}

	content, err := os.ReadFile(profileFile)
	if err != nil {
		return err
	}

	if !strings.Contains(string(content), zoiBinPath) {
		src.PrintInfo("Updating your shell profile to include Zoi's managed Go.")
		f, err := os.OpenFile(profileFile, os.O_APPEND|os.O_WRONLY, 0644)
		if err != nil {
			return err
		}
		defer f.Close()

		if _, err := f.WriteString(fmt.Sprintf("\n%s\n%s\n", commentLine, exportLine)); err != nil {
			return err
		}
		src.PrintSuccess("Profile updated! Please restart your shell or run 'source %s'", profileFile)
	} else {
		src.PrintInfo("Zoi VM path already exists in your shell profile.")
	}

	return nil
}
