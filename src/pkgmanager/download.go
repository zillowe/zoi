package pkgmanager

import (
	"archive/tar"
	"archive/zip"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"runtime"
	"strings"

	"zoi/src"

	"github.com/ulikunitz/xz"
)

func DownloadAndExtractBinary(recipe PackageRecipe, destDir string) (string, error) {
	replacer := strings.NewReplacer(
		"{version}", recipe.PackageInfo.Version,
		"{os}", runtime.GOOS,
		"{arch}", runtime.GOARCH,
		"{ext}", getPlatformExtension(),
	)
	url := replacer.Replace(recipe.PackageInfo.Bin)

	src.PrintInfo("Downloading pre-compiled binary from: %s", url)
	resp, err := http.Get(url)
	if err != nil {
		return "", fmt.Errorf("failed to start download: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("download failed with HTTP status: %s", resp.Status)
	}

	if err := os.MkdirAll(destDir, 0755); err != nil {
		return "", err
	}
	src.PrintInfo("Extracting binary to: %s", destDir)

	switch getPlatformExtension() {
	case ".zip":
		return extractZip(resp.Body, destDir)
	case ".tar.xz":
		return extractTarXz(resp.Body, destDir)
	default:
		return "", fmt.Errorf("unsupported archive extension: %s", getPlatformExtension())
	}
}

func getPlatformExtension() string {
	if runtime.GOOS == "windows" {
		return ".zip"
	}
	return ".tar.xz"
}

func extractTarXz(r io.Reader, dest string) (string, error) {
	xzr, err := xz.NewReader(r)
	if err != nil {
		return "", err
	}
	tr := tar.NewReader(xzr)

	for {
		header, err := tr.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			return "", err
		}

		target := filepath.Join(dest, filepath.Base(header.Name))

		if header.Typeflag == tar.TypeReg {
			outFile, err := os.OpenFile(target, os.O_CREATE|os.O_RDWR, os.FileMode(header.Mode))
			if err != nil {
				return "", err
			}
			if _, err := io.Copy(outFile, tr); err != nil {
				outFile.Close()
				return "", err
			}
			outFile.Close()

			return target, nil
		}
	}
	return "", fmt.Errorf("no executable file found in the tar.xz archive")
}

func extractZip(r io.Reader, dest string) (string, error) {
	tmpFile, err := os.CreateTemp("", "zoi-*.zip")
	if err != nil {
		return "", err
	}
	defer os.Remove(tmpFile.Name())

	_, err = io.Copy(tmpFile, r)
	if err != nil {
		return "", err
	}
	tmpFile.Close()

	zr, err := zip.OpenReader(tmpFile.Name())
	if err != nil {
		return "", err
	}
	defer zr.Close()

	for _, f := range zr.File {
		target := filepath.Join(dest, f.Name)

		if f.FileInfo().IsDir() {
			continue
		}

		rc, err := f.Open()
		if err != nil {
			return "", err
		}

		outFile, err := os.OpenFile(target, os.O_CREATE|os.O_RDWR, f.Mode())
		if err != nil {
			rc.Close()
			return "", err
		}

		_, err = io.Copy(outFile, rc)
		rc.Close()
		outFile.Close()

		if err != nil {
			return "", err
		}
		return target, nil
	}

	return "", fmt.Errorf("no executable file found in the zip archive")
}
