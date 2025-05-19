package commands

import (
	"archive/tar"
	"archive/zip"
	"compress/gzip"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"runtime"
	"strings"
	"time"

	"github.com/fatih/color"
	"github.com/ulikunitz/xz"
)

const (
	currentVersionLink = "current"
)

func VmCommand(args []string) {
	red := color.New(color.FgRed).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()

	if len(args) < 1 {
		printVmUsage()
		return
	}

	subCommand := args[0]
	subArgs := args[1:]

	switch subCommand {
	case "install":
		if len(subArgs) < 1 {
			fmt.Println(yellow("Usage: zoi vm install <language>@<version>"))
			return
		}
		langVersion := subArgs[0]
		parts := strings.Split(langVersion, "@")
		if len(parts) != 2 {
			fmt.Println(red("Error: Invalid format. Use <language>@<version> (e.g. go@1.21.0)"))
			return
		}
		language := strings.ToLower(parts[0])
		version := parts[1]
		installVersion(language, version)
	case "use":
		if len(subArgs) < 1 {
			fmt.Println(yellow("Usage: zoi vm use <language>@<version>"))
			return
		}
		langVersion := subArgs[0]
		parts := strings.Split(langVersion, "@")
		if len(parts) != 2 {
			fmt.Println(red("Error: Invalid format. Use <language>@<version> (e.g. go@1.21.0)"))
			return
		}
		language := strings.ToLower(parts[0])
		version := parts[1]
		useVersion(language, version)
	case "list":
		listInstalledVersions()
	case "current":
		showCurrentVersions()
	case "uninstall":
		if len(subArgs) < 1 {
			fmt.Println(yellow("Usage: zoi vm uninstall <language>@<version>"))
			return
		}
		langVersion := subArgs[0]
		parts := strings.Split(langVersion, "@")
		if len(parts) != 2 {
			fmt.Println(red("Error: Invalid format. Use <language>@<version> (e.g. go@1.21.0)"))
			return
		}
		language := strings.ToLower(parts[0])
		version := parts[1]
		uninstallVersion(language, version)
	default:
		fmt.Printf("%s Unknown vm subcommand: '%s'\n", red("Error:"), yellow(subCommand))
		printVmUsage()
	}
}

func getVmRootPath() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", fmt.Errorf("could not get user home directory: %w", err)
	}
	return filepath.Join(home, ".zoi", "vm"), nil
}

func getLanguageRootPath(language string) (string, error) {
	vmRoot, err := getVmRootPath()
	if err != nil {
		return "", err
	}
	return filepath.Join(vmRoot, language), nil
}

func getVersionInstallPath(language, version string) (string, error) {
	langRoot, err := getLanguageRootPath(language)
	if err != nil {
		return "", err
	}
	return filepath.Join(langRoot, version), nil
}

func getCurrentVersionSymlinkPath(language string) (string, error) {
	langRoot, err := getLanguageRootPath(language)
	if err != nil {
		return "", err
	}
	return filepath.Join(langRoot, currentVersionLink), nil
}

func installVersion(language, version string) {
	red := color.New(color.FgRed).SprintFunc()
	green := color.New(color.FgGreen).SprintFunc()
	cyan := color.New(color.FgCyan).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()

	fmt.Printf("%s Attempting to install %s version %s...\n", cyan("ℹ"), language, version)

	installPath, err := getVersionInstallPath(language, version)
	if err != nil {
		fmt.Printf("%s Error determining install path: %v\n", red("✗"), err)
		return
	}

	installNeeded := true
	if _, err := os.Stat(installPath); err == nil {
		fmt.Printf("%s Found existing directory: %s. Verifying installation...\n", cyan("▸"), installPath)
		verificationErr := verifyInstallation(language, installPath)
		if verificationErr == nil {
			fmt.Printf("%s %s version %s is already installed and verified at %s\n", green("✓"), language, version, installPath)
			installNeeded = false
		} else {
			fmt.Printf("%s Existing installation at %s failed verification: %v\n", yellow("!"), installPath, verificationErr)
			fmt.Printf("%s Proceeding with re-installation.\n", yellow("!"))
			fmt.Printf("%s Removing existing directory before reinstall: %s\n", cyan("▸"), installPath)
			if err := os.RemoveAll(installPath); err != nil {
				fmt.Printf("%s Failed to remove existing directory %s: %v. Aborting.\n", red("✗"), installPath, err)
				return
			}
			fmt.Printf("%s Existing directory removed.\n", cyan("▸"))
		}
	} else if !os.IsNotExist(err) {
		fmt.Printf("%s Error checking install path %s: %v\n", red("✗"), installPath, err)
		return
	}

	if !installNeeded {
		currentVer := getCurrentActiveVersion(language)
		if version != currentVer {
			fmt.Printf("%s To use this version, run: %s\n", cyan("ℹ"), green(fmt.Sprintf("zoi vm use %s@%s", language, version)))
		}
		return
	}

	osName := runtime.GOOS
	archName := runtime.GOARCH
	fmt.Printf("%s Detected OS: %s, Arch: %s\n", cyan("▸"), osName, archName)

	downloadURL, archiveType, err := determineDownloadDetails(language, version, osName, archName)
	if err != nil {
		fmt.Printf("%s Could not determine download details: %v\n", red("✗"), err)
		return
	}
	if downloadURL == "" {
		fmt.Printf("%s No compatible download found for %s@%s on %s/%s.\n", red("✗"), language, version, osName, archName)
		return
	}
	fmt.Printf("%s Determined download URL: %s\n", cyan("▸"), downloadURL)

	if err := os.MkdirAll(installPath, 0750); err != nil {
		fmt.Printf("%s Failed to create installation directory %s: %v\n", red("✗"), installPath, err)
		return
	}

	fmt.Printf("%s Downloading from %s...\n", cyan("⏳"), downloadURL)
	tempDir, err := os.MkdirTemp("", "zoi-vm-download-")
	if err != nil {
		fmt.Printf("%s Failed to create temporary download directory: %v\n", red("✗"), err)
		return
	}
	defer os.RemoveAll(tempDir)

	tempFilePath := filepath.Join(tempDir, filepath.Base(downloadURL))
	err = downloadFile(downloadURL, tempFilePath)
	if err != nil {
		fmt.Printf("%s Download failed: %v\n", red("✗"), err)
		os.RemoveAll(installPath)
		return
	}
	fmt.Printf("%s Download complete (%s).\n", green("✓"), tempFilePath)

	fmt.Printf("%s Extracting %s to %s...\n", cyan("⏳"), archiveType, installPath)
	err = extractArchive(tempFilePath, installPath, archiveType)
	if err != nil {
		fmt.Printf("%s Extraction failed: %v\n", red("✗"), err)
		os.RemoveAll(installPath)
		return
	}
	fmt.Printf("%s Extraction complete.\n", green("✓"))

	fmt.Printf("%s Verifying new installation...\n", cyan("▸"))
	verificationErr := verifyInstallation(language, installPath)
	if verificationErr != nil {
		fmt.Printf("%s Installation at %s failed verification after extract: %v\n", red("✗"), installPath, verificationErr)
		fmt.Printf("%s The downloaded archive might be corrupt or incompatible. Please check the download source.\n", red("✗"))
		os.RemoveAll(installPath)
	} else {
		fmt.Printf("%s New installation verified successfully.\n", green("✓"))
		fmt.Printf("\n%s Successfully installed and verified %s version %s to %s\n", green("✓"), language, version, installPath)
		fmt.Printf("%s To use this version, run: %s\n", cyan("ℹ"), green(fmt.Sprintf("zoi vm use %s@%s", language, version)))
	}
}

func runVersionCheckCommand(language, installPath string) (string, error) {
	binDir := getExpectedBinDirectory(language, installPath)
	exeName := language
	versionArg := "--version"

	switch language {
	case "go":
		exeName = "go"
		versionArg = "version"
	case "python":
		exeName = "python"
	case "ruby":
		exeName = "ruby"
	default:
		return "", fmt.Errorf("unknown language for version check: %s", language)
	}

	if runtime.GOOS == "windows" {
		if _, err := os.Stat(filepath.Join(binDir, exeName+".exe")); err == nil {
			exeName += ".exe"
		}
		if language == "python" {
			if _, err := os.Stat(filepath.Join(binDir, "python.exe")); err != nil {
				// Fallback to check py.exe if python.exe not found? Less common in isolated installs.
			}
		}
	}

	exePath := filepath.Join(binDir, exeName)

	if _, err := os.Stat(exePath); os.IsNotExist(err) {
		return "", fmt.Errorf("executable not found at expected path: %s", exePath)
	} else if err != nil {
		return "", fmt.Errorf("error checking executable path %s: %w", exePath, err)
	}

	cmd := exec.Command(exePath, versionArg)
	// Set working directory? Usually not needed for version check.
	// cmd.Dir = installPath

	output, err := cmd.CombinedOutput()
	if err != nil {
		if len(output) > 0 {
			fmt.Printf("Warning: version command '%s %s' exited with error, but produced output: %s\n", exePath, versionArg, string(output))
		}
		return "", fmt.Errorf("failed to run '%s %s': %w - output: %s", exePath, versionArg, err, string(output))
	}

	if !strings.Contains(strings.ToLower(string(output)), language) {
		fmt.Printf("Warning: Output from version command ('%s') does not contain language name '%s'. Output might be unexpected.\n", string(output), language)
	}

	return string(output), nil
}

func verifyInstallation(language, installPath string) error {
	_, err := runVersionCheckCommand(language, installPath)
	return err
}

func determineDownloadDetails(language, version, osName, archName string) (url string, archiveType string, err error) {
	if language == "go" {
		goOS := osName
		goArch := archName
		switch archName {
		case "x86_64":
			goArch = "amd64"
		case "aarch64":
			goArch = "arm64"
		// Add other mappings if needed (e.g. "x86" -> "386")
		case "amd64", "arm64", "386", "armv6l":
		default:
			fmt.Printf("Warning: Unverified architecture for Go: %s\n", archName)
		}

		ext := "tar.gz"
		switch osName {
		case "windows":
			ext = "zip"
		case "darwin":
			if goArch != "amd64" && goArch != "arm64" {
				return "", "", fmt.Errorf("unsupported architecture for Go on darwin: %s", goArch)
			}
		case "linux":
			if goArch != "amd64" && goArch != "arm64" {
				return "", "", fmt.Errorf("unsupported architecture for Go on darwin: %s", goArch)
			}
		default:
			return "", "", fmt.Errorf("unsupported OS for Go: %s", osName)
		}

		url = fmt.Sprintf("https://go.dev/dl/go%s.%s-%s.%s", version, goOS, goArch, ext)
		archiveType = ext
		client := http.Client{Timeout: 10 * time.Second}
		resp, err := client.Head(url)
		if err != nil {
			return "", "", fmt.Errorf("failed to check URL %s: %w", url, err)
		}
		resp.Body.Close()
		if resp.StatusCode != http.StatusOK {
			return "", "", fmt.Errorf("download URL %s returned status %s", url, resp.Status)
		}
		return url, archiveType, nil

	} else if language == "python" {
		baseURL := fmt.Sprintf("https://www.python.org/ftp/python/%s/", version)
		var filename string
		var pyEmbedArch string

		switch archName {
		case "x86_64", "amd64":
			pyEmbedArch = "amd64"
		case "aarch64", "arm64":
			pyEmbedArch = "arm64"
		case "x86", "i386", "i686", "386":
			pyEmbedArch = "win32"
		default:
			pyEmbedArch = archName
		}

		primaryArchiveType := ""
		fallbackArchiveType := ""

		switch osName {
		case "windows":
			primaryArchiveType = "zip"
			switch pyEmbedArch {
			case "amd64", "arm64", "win32":
				filename = fmt.Sprintf("python-%s-embed-%s.zip", version, pyEmbedArch)
			default:
				return "", "", fmt.Errorf("unsupported architecture '%s' for Python Windows embeddable zip (expected amd64, arm64, or win32 based on common patterns)", pyEmbedArch)
			}

		case "darwin", "linux":
			primaryArchiveType = "tar.xz"
			fallbackArchiveType = "tgz"
			filename = fmt.Sprintf("Python-%s.tar.xz", version)

		default:
			return "", "", fmt.Errorf("unsupported OS for Python downloads from python.org: %s", osName)
		}

		url = baseURL + filename
		archiveType = primaryArchiveType

		client := http.Client{Timeout: 15 * time.Second}
		resp, err := client.Head(url)

		shouldTryFallback := (osName == "linux" || osName == "darwin") && fallbackArchiveType != ""
		headFailed := err != nil || resp.StatusCode != http.StatusOK

		if headFailed && shouldTryFallback {
			if resp != nil {
				resp.Body.Close()
			}
			altFilename := fmt.Sprintf("Python-%s.tgz", version)
			altUrl := baseURL + altFilename
			archiveType = fallbackArchiveType

			resp, err = client.Head(altUrl)

			if err != nil {
				return "", "", fmt.Errorf("failed to check primary URL %s and fallback URL %s: %w", url, altUrl, err)
			}
			if resp.StatusCode != http.StatusOK {
				resp.Body.Close()
				return "", "", fmt.Errorf("primary URL %s failed or not OK; fallback URL %s returned status %s", url, altUrl, resp.Status)
			}
			url = altUrl

		} else if headFailed {
			status := "unknown"
			if resp != nil {
				status = resp.Status
				resp.Body.Close()
			}
			errMsg := fmt.Sprintf("download check failed for URL %s", url)
			if resp != nil {
				errMsg += fmt.Sprintf(" (status: %s)", status)
			}
			if err != nil {
				return "", "", fmt.Errorf("%s: %w", errMsg, err)
			}
			return "", "", fmt.Errorf("%s", errMsg)

		}
		resp.Body.Close()
		return url, archiveType, nil

	} else if language == "ruby" {
		// Language not supported yet.
		return "", "", fmt.Errorf("ruby install not implemented: direct binary downloads are inconsistent across platforms. Consider using RVM, rbenv/ruby-build, or system package manager")
	}

	return "", "", fmt.Errorf("unsupported language for version management: %s", language)
}

func downloadFile(url, filepath string) error {
	out, err := os.Create(filepath)
	if err != nil {
		return fmt.Errorf("failed to create file %s: %w", filepath, err)
	}
	defer out.Close()

	client := http.Client{Timeout: 300 * time.Second}
	resp, err := client.Get(url)
	if err != nil {
		return fmt.Errorf("failed to start download from %s: %w", url, err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		out.Close()
		os.Remove(filepath)
		return fmt.Errorf("bad status from %s: %s", url, resp.Status)
	}

	_, err = io.Copy(out, resp.Body)
	if err != nil {
		out.Close()
		os.Remove(filepath)
		return fmt.Errorf("failed to write download to %s: %w", filepath, err)
	}

	return nil
}

func extractArchive(sourcePath, destPath, archiveType string) error {
	switch archiveType {
	case "tar.gz":
		return extractTarGz(sourcePath, destPath)
	case "tar.xz":
		return extractTarXz(sourcePath, destPath)
	case "zip":
		return extractZip(sourcePath, destPath)
	// Add cases for .tar.xz, .pkg, .exe if needed later (more complex)
	default:
		return fmt.Errorf("unsupported archive type: %s", archiveType)
	}
}

func extractTarGz(sourcePath, destPath string) error {
	file, err := os.Open(sourcePath)
	if err != nil {
		return fmt.Errorf("failed to open archive %s: %w", sourcePath, err)
	}
	defer file.Close()

	gzr, err := gzip.NewReader(file)
	if err != nil {
		return fmt.Errorf("failed to create gzip reader for %s: %w", sourcePath, err)
	}
	defer gzr.Close()

	tr := tar.NewReader(gzr)

	for {
		header, err := tr.Next()

		switch {
		case err == io.EOF:
			return nil
		case err != nil:
			return fmt.Errorf("error reading tar header: %w", err)
		case header == nil:
			continue
		}

		target := filepath.Join(destPath, header.Name)
		cleanTarget, err := SanitizeArchivePath(destPath, target)
		if err != nil {
			return err
		}
		target = cleanTarget

		switch header.Typeflag {
		case tar.TypeDir:
			if _, err := os.Stat(target); err != nil {
				if err := os.MkdirAll(target, 0755); err != nil {
					return fmt.Errorf("failed to create directory %s: %w", target, err)
				}
			}
		case tar.TypeReg:
			if err := os.MkdirAll(filepath.Dir(target), 0755); err != nil {
				return fmt.Errorf("failed to create parent directory for %s: %w", target, err)
			}
			f, err := os.OpenFile(target, os.O_CREATE|os.O_RDWR, os.FileMode(header.Mode))
			if err != nil {
				return fmt.Errorf("failed to create file %s: %w", target, err)
			}
			if _, err := io.Copy(f, tr); err != nil {
				f.Close()
				return fmt.Errorf("failed to copy file contents to %s: %w", target, err)
			}
			f.Close()
		case tar.TypeSymlink:
			if err := os.MkdirAll(filepath.Dir(target), 0755); err != nil {
				return fmt.Errorf("failed to create parent directory for symlink %s: %w", target, err)
			}
			if err := os.Symlink(header.Linkname, target); err != nil {
				fmt.Printf("Warning: failed to create symlink %s -> %s: %v\n", target, header.Linkname, err)
				// return fmt.Errorf("failed to create symlink %s -> %s: %w", target, header.Linkname, err)
			}
		default:
			fmt.Printf("Warning: Unable to extract type %c in %s\n", header.Typeflag, header.Name)
		}
	}
}

func extractTarXz(sourcePath, destPath string) error {
	if err := os.MkdirAll(destPath, 0755); err != nil {
		return fmt.Errorf("failed to create destination directory %s: %w", destPath, err)
	}

	file, err := os.Open(sourcePath)
	if err != nil {
		return fmt.Errorf("failed to open archive %s: %w", sourcePath, err)
	}
	defer file.Close()

	xzr, err := xz.NewReader(file)
	if err != nil {
		return fmt.Errorf("failed to create xz reader for %s: %w", sourcePath, err)
	}

	tr := tar.NewReader(xzr)

	for {
		header, err := tr.Next()

		switch {
		case err == io.EOF:
			return nil

		case err != nil:
			return fmt.Errorf("error reading tar header from %s: %w", sourcePath, err)

		case header == nil:
			continue
		}

		target := filepath.Join(destPath, header.Name)

		cleanTarget, err := SanitizeArchivePath(destPath, target)
		if err != nil {
			return fmt.Errorf("path sanitization failed for '%s': %w", header.Name, err)
		}
		target = cleanTarget

		switch header.Typeflag {
		case tar.TypeDir:
			if _, err := os.Stat(target); err != nil {
				if err := os.MkdirAll(target, 0755); err != nil {
					return fmt.Errorf("failed to create directory %s: %w", target, err)
				}
			}

		case tar.TypeReg:
			parentDir := filepath.Dir(target)
			if err := os.MkdirAll(parentDir, 0755); err != nil {
				return fmt.Errorf("failed to create parent directory %s for file %s: %w", parentDir, target, err)
			}

			f, err := os.OpenFile(target, os.O_CREATE|os.O_RDWR|os.O_TRUNC, os.FileMode(header.Mode))
			if err != nil {
				return fmt.Errorf("failed to create file %s: %w", target, err)
			}

			if _, err := io.Copy(f, tr); err != nil {
				f.Close()
				return fmt.Errorf("failed to copy file contents to %s: %w", target, err)
			}

			if err := f.Close(); err != nil {
				fmt.Printf("Warning: failed to close file %s: %v\n", target, err)
			}

		case tar.TypeSymlink:
			parentDir := filepath.Dir(target)
			if err := os.MkdirAll(parentDir, 0755); err != nil {
				return fmt.Errorf("failed to create parent directory %s for symlink %s: %w", parentDir, target, err)
			}

			if err := os.Symlink(header.Linkname, target); err != nil {
				fmt.Printf("Warning: failed to create symlink %s -> %s: %v\n", target, header.Linkname, err)
				// return fmt.Errorf("failed to create symlink %s -> %s: %w", target, header.Linkname, err)
			}

		default:
			fmt.Printf("Warning: Skipping unsupported tar entry type %c for entry %s\n", header.Typeflag, header.Name)
		}
	}
}

func extractZip(sourcePath, destPath string) error {
	r, err := zip.OpenReader(sourcePath)
	if err != nil {
		return fmt.Errorf("failed to open zip archive %s: %w", sourcePath, err)
	}
	defer r.Close()

	if err := os.MkdirAll(destPath, 0755); err != nil {
		return fmt.Errorf("failed to create destination directory %s: %w", destPath, err)
	}

	extractAndWriteFile := func(f *zip.File) error {
		rc, err := f.Open()
		if err != nil {
			return fmt.Errorf("failed to open file in zip %s: %w", f.Name, err)
		}
		defer rc.Close()

		path := filepath.Join(destPath, f.Name)

		cleanPath, err := SanitizeArchivePath(destPath, path)
		if err != nil {
			return err
		}
		path = cleanPath

		if f.FileInfo().IsDir() {
			if err := os.MkdirAll(path, f.Mode()); err != nil {
				return fmt.Errorf("failed to create directory %s: %w", path, err)
			}
		} else {
			if err := os.MkdirAll(filepath.Dir(path), f.Mode()|0111); err != nil {
				return fmt.Errorf("failed to create parent directory for %s: %w", path, err)
			}
			w, err := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, f.Mode())
			if err != nil {
				return fmt.Errorf("failed to create file %s: %w", path, err)
			}
			defer w.Close()

			_, err = io.Copy(w, rc)
			if err != nil {
				return fmt.Errorf("failed to write file %s: %w", path, err)
			}
		}
		return nil
	}

	for _, f := range r.File {
		err := extractAndWriteFile(f)
		if err != nil {
			return err
		}
	}
	return nil
}

func SanitizeArchivePath(dest, target string) (string, error) {
	absDest, err := filepath.Abs(dest)
	if err != nil {
		return "", fmt.Errorf("failed to get absolute path for destination '%s': %w", dest, err)
	}

	absTarget, err := filepath.Abs(target)
	if err != nil {
		return "", fmt.Errorf("failed to get absolute path for target '%s': %w", target, err)
	}

	cleanDest := filepath.Clean(absDest)
	cleanTarget := filepath.Clean(absTarget)

	if !strings.HasPrefix(cleanTarget, cleanDest) {
		return "", fmt.Errorf("path traversal detected: '%s' attempts to escape destination '%s'", target, dest)
	}
	return target, nil
}

func useVersion(language, version string) {
	red := color.New(color.FgRed).SprintFunc()
	green := color.New(color.FgGreen).SprintFunc()
	cyan := color.New(color.FgCyan).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()

	fmt.Printf("%s Attempting to switch to %s version %s...\n", cyan("ℹ"), language, version)

	installPath, err := getVersionInstallPath(language, version)
	if err != nil {
		fmt.Printf("%s Error determining install path: %v\n", red("✗"), err)
		return
	}

	if _, err := os.Stat(installPath); os.IsNotExist(err) {
		fmt.Printf("%s Error: %s version %s is not installed at %s.\n", red("✗"), language, version, installPath)
		fmt.Printf("%s Install it first using: %s\n", cyan("ℹ"), green(fmt.Sprintf("zoi vm install %s@%s", language, version)))
		return
	} else if err != nil {
		fmt.Printf("%s Error checking installation path %s: %v\n", red("✗"), installPath, err)
		return
	} else {
		fmt.Printf("%s Found installation directory: %s. Verifying...\n", cyan("▸"), installPath)
		verificationErr := verifyInstallation(language, installPath)
		if verificationErr != nil {
			fmt.Printf("%s Error: Installation at %s failed verification: %v\n", red("✗"), installPath, verificationErr)
			fmt.Printf("%s Cannot use this version. Try reinstalling: %s\n", red("✗"), green(fmt.Sprintf("zoi vm install %s@%s", language, version)))
			return
		}
		fmt.Printf("%s Installation verified.\n", green("✓"))
	}

	err = setEnvironmentSymlink(language, installPath)
	if err != nil {
		fmt.Printf("%s Failed to set environment symlink: %v\n", red("✗"), err)
		fmt.Printf("%s Manual configuration might be required.\n", yellow("!"))
		return
	}

	symlinkPath, _ := getCurrentVersionSymlinkPath(language)
	binDir := getExpectedBinDirectory(language, symlinkPath)

	fmt.Printf("%s Successfully configured %s version %s for use via symlink (%s).\n", green("✓"), language, version, symlinkPath)
	fmt.Printf("%s IMPORTANT: Ensure '%s' is in your system's PATH.\n", yellow("!"), binDir)
	fmt.Printf("%s You may need to add it once manually to your shell profile (e.g., ~/.bashrc, ~/.zshrc, System Environment Variables on Windows) and restart your terminal.\n", yellow("!"))

	if language == "ruby" {
		fmt.Printf("%s RUBY NOTE: To ensure gems are installed within this version's directory,\n", yellow("!"))
		fmt.Printf("%s add the following to your shell profile (e.g. ~/.bashrc, ~/.zshrc):\n", yellow("!"))
		gemHome := filepath.Join(symlinkPath, "lib", "ruby", "gems", getRubyApiVersion(installPath))
		gemPath := gemHome
		fmt.Printf("%s   export GEM_HOME=\"%s\"\n", yellow("!"), gemHome)
		fmt.Printf("%s   export GEM_PATH=\"%s\"\n", yellow("!"), gemPath)
		fmt.Printf("%s   export PATH=\"$GEM_HOME/bin:$PATH\" # Ensure gem executables are in PATH\n", yellow("!"))
		fmt.Printf("%s (The exact GEM_HOME/PATH structure might vary slightly based on Ruby compilation/installation.)\n", yellow("!"))
	}
}

func getRubyApiVersion(rubyInstallPath string) string {
	libRubyPath := filepath.Join(rubyInstallPath, "lib", "ruby")
	entries, err := os.ReadDir(libRubyPath)
	if err != nil {
		return "?.?.0"
	}
	versionRegex := regexp.MustCompile(`^\d+\.\d+\.\d+$`)
	for _, entry := range entries {
		if entry.IsDir() && versionRegex.MatchString(entry.Name()) {
			return entry.Name()
		}
	}
	return "?.?.0"
}

func setEnvironmentSymlink(language, versionInstallPath string) error {
	cyan := color.New(color.FgCyan).SprintFunc()

	symlinkPath, err := getCurrentVersionSymlinkPath(language)
	if err != nil {
		return fmt.Errorf("could not get symlink path: %w", err)
	}

	fmt.Printf("%s Updating '%s' symlink -> %s\n", cyan("▸"), symlinkPath, versionInstallPath)

	if err := os.MkdirAll(filepath.Dir(symlinkPath), 0750); err != nil {
		return fmt.Errorf("failed to create directory for symlink %s: %w", filepath.Dir(symlinkPath), err)
	}

	if _, err := os.Lstat(symlinkPath); err == nil {
		if err := os.Remove(symlinkPath); err != nil {
			return fmt.Errorf("failed to remove existing symlink %s: %w", symlinkPath, err)
		}
		fmt.Printf("%s Removed existing symlink.\n", cyan("▸"))
	} else if !os.IsNotExist(err) {
		return fmt.Errorf("failed to check existing symlink %s: %w", symlinkPath, err)
	}

	if err := os.Symlink(versionInstallPath, symlinkPath); err != nil {
		if runtime.GOOS == "windows" {
			return fmt.Errorf("failed to create symlink %s -> %s: %w. (On Windows, this might require admin privileges or Developer Mode)", symlinkPath, versionInstallPath, err)
		}
		return fmt.Errorf("failed to create symlink %s -> %s: %w", symlinkPath, versionInstallPath, err)
	}

	fmt.Printf("%s Symlink updated successfully.\n", cyan("▸"))
	return nil
}

func getExpectedBinDirectory(language, basePath string) string {
	if language == "go" {
		goBinPath := filepath.Join(basePath, "go", "bin")
		if _, err := os.Stat(goBinPath); err == nil {
			return goBinPath
		}
		return filepath.Join(basePath, "bin")
	}
	if language == "python" {
		if runtime.GOOS == "windows" {
			scriptsPath := filepath.Join(basePath, "Scripts")
			if _, err := os.Stat(scriptsPath); err == nil {
				return scriptsPath
			}
		}
		return filepath.Join(basePath, "bin")
	}
	if language == "ruby" {
		return filepath.Join(basePath, "bin")
	}

	return filepath.Join(basePath, "bin")
}

func listInstalledVersions() {
	cyan := color.New(color.FgCyan).SprintFunc()
	green := color.New(color.FgGreen).SprintFunc()

	fmt.Println(cyan("Installed Versions:"))
	vmRoot, err := getVmRootPath()
	if err != nil {
		fmt.Printf("  Error reading VM root directory: %v\n", err)
		return
	}

	langs, err := os.ReadDir(vmRoot)
	if err != nil {
		if os.IsNotExist(err) {
			fmt.Println("  No languages installed yet.")
			return
		}
		fmt.Printf("  Error reading languages directory %s: %v\n", vmRoot, err)
		return
	}

	foundAny := false
	for _, langEntry := range langs {
		if !langEntry.IsDir() {
			continue
		}
		langName := langEntry.Name()
		langPath := filepath.Join(vmRoot, langName)

		versions, err := os.ReadDir(langPath)
		if err != nil {
			fmt.Printf("  Error reading versions for %s: %v\n", langName, err)
			continue
		}

		fmt.Printf("  %s:\n", green(langName))
		langHasVersions := false
		currentVersion := getCurrentActiveVersion(langName)

		for _, verEntry := range versions {
			if !verEntry.IsDir() || verEntry.Name() == currentVersionLink {
				continue
			}
			versionName := verEntry.Name()
			if versionName == currentVersion {
				fmt.Printf("    * %s (active)\n", versionName)
			} else {
				fmt.Printf("    - %s\n", versionName)
			}
			langHasVersions = true
			foundAny = true
		}
		if !langHasVersions {
			fmt.Println("    (No versions installed)")
		}
	}

	if !foundAny {
		fmt.Println("  No versions installed in ~/.zoi/vm")
	}
}

func showCurrentVersions() {
	cyan := color.New(color.FgCyan).SprintFunc()
	green := color.New(color.FgGreen).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()

	fmt.Println(cyan("Active Versions (via symlink):"))
	vmRoot, err := getVmRootPath()
	if err != nil {
		fmt.Printf("  Error reading VM root directory: %v\n", err)
		return
	}

	langs, err := os.ReadDir(vmRoot)
	if err != nil {
		if os.IsNotExist(err) {
			fmt.Println("  No languages managed yet.")
			return
		}
		fmt.Printf("  Error reading languages directory %s: %v\n", vmRoot, err)
		return
	}

	foundAny := false
	for _, langEntry := range langs {
		if !langEntry.IsDir() {
			continue
		}
		langName := langEntry.Name()
		activeVersion := getCurrentActiveVersion(langName)

		if activeVersion != "" {
			fmt.Printf("  %s: %s\n", green(langName), activeVersion)
			foundAny = true
		} else {
			symlinkPath, _ := getCurrentVersionSymlinkPath(langName)
			if _, err := os.Lstat(symlinkPath); err == nil {
				fmt.Printf("  %s: %s (symlink exists but might be broken or unresolvable)\n", green(langName), yellow("unknown"))
			}
		}
	}

	if !foundAny {
		fmt.Println("  No active versions detected (no 'current' symlinks found).")
	}
	fmt.Printf("\n%s Note: This shows the target of the '~/.zoi/vm/<lang>/current' symlink.\n", yellow("ℹ"))
	fmt.Printf("%s Ensure the relevant '.../current/bin' path is in your main PATH environment variable.\n", yellow("ℹ"))
}

func getCurrentActiveVersion(language string) string {
	symlinkPath, err := getCurrentVersionSymlinkPath(language)
	if err != nil {
		return ""
	}

	targetPath, err := os.Readlink(symlinkPath)
	if err != nil {
		return ""
	}

	return filepath.Base(targetPath)
}

func printVmUsage() {
	cyan := color.New(color.FgCyan).SprintFunc()
	green := color.New(color.FgGreen).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()
	red := color.New(color.FgRed).SprintFunc()

	fmt.Printf("%s Manage language versions for your projects (Go, Python, etc.).\n\n", yellow("zoi vm:"))
	fmt.Printf("%s\n", yellow("Usage:"))
	fmt.Printf("  zoi %s <subcommand> [arguments...]\n\n", green("vm"))
	fmt.Printf("%s\n", cyan("Subcommands:"))

	fmt.Printf("  %s <language>@<version>\n", green("install"))
	fmt.Printf("    Downloads and installs a specific language version.\n")
	fmt.Printf("    Example: %s\n\n", cyan("zoi vm install go@1.21.5"))

	fmt.Printf("  %s <language>@<version>\n", green("use"))
	fmt.Printf("    Sets a specific installed language version as the active version for the current environment.\n")
	fmt.Printf("    This updates the 'current' symlink (e.g. ~/.zoi/vm/<language>/current).\n")
	fmt.Printf("    Example: %s\n\n", cyan("zoi vm use python@3.10.7"))

	fmt.Printf("  %s <language>@<version>\n", green("uninstall"))
	fmt.Printf("    Removes a specific language version from your system.\n")
	fmt.Printf("    Example: %s\n\n", cyan("zoi vm uninstall go@1.20.0"))

	fmt.Printf("  %s\n", green("list"))
	fmt.Printf("    Lists all language versions installed by zoi vm.\n")
	fmt.Printf("    Example: %s\n\n", cyan("zoi vm list"))

	fmt.Printf("  %s\n", green("current"))
	fmt.Printf("    Shows the currently active (symlinked) version for each managed language.\n")
	fmt.Printf("    Example: %s\n\n", cyan("zoi vm current"))

	fmt.Printf("\n%s\n", yellow("Notes:"))
	fmt.Printf("  - Supported languages currently include: Go, Python. (Ruby support is in progress).\n")
	fmt.Printf("  - The 'use' command makes a version active by updating a symlink. You need to ensure the\n")
	fmt.Printf("    relevant 'current/bin' directory (e.g. '~/.zoi/vm/<language>/current/bin') is in your\n")
	fmt.Printf("    system's PATH. This setup is typically done once manually.\n")
	fmt.Printf("  - For Ruby (when fully supported), additional GEM_HOME/GEM_PATH setup might be required as\n")
	fmt.Printf("    indicated by the 'use' command output.\n")
	fmt.Printf(red("  - VM is still in early development\n"))
}

func uninstallVersion(language, version string) {
	red := color.New(color.FgRed).SprintFunc()
	green := color.New(color.FgGreen).SprintFunc()
	cyan := color.New(color.FgCyan).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()

	fmt.Printf("%s Attempting to uninstall %s version %s...\n", cyan("ℹ"), language, version)

	installPath, err := getVersionInstallPath(language, version)
	if err != nil {
		fmt.Printf("%s Error determining install path: %v\n", red("✗"), err)
		return
	}

	if _, err := os.Stat(installPath); os.IsNotExist(err) {
		fmt.Printf("%s Error: %s version %s is not installed at %s.\n", red("✗"), language, version, installPath)
		return
	} else if err != nil {
		fmt.Printf("%s Error checking installation path %s: %v\n", red("✗"), installPath, err)
		return
	}

	currentActiveVer := getCurrentActiveVersion(language)
	symlinkPath, _ := getCurrentVersionSymlinkPath(language)

	if version == currentActiveVer {
		fmt.Printf("%s This version is currently active. Removing symlink '%s'.\n", yellow("!"), symlinkPath)
		if err := os.Remove(symlinkPath); err != nil && !os.IsNotExist(err) {
			fmt.Printf("%s Failed to remove symlink %s: %v. Please remove it manually.\n", red("✗"), symlinkPath, err)
			// Continue with uninstalling the version directory
		} else {
			fmt.Printf("%s Symlink removed.\n", cyan("▸"))
		}
	}

	fmt.Printf("%s Removing installation directory: %s\n", cyan("▸"), installPath)
	if err := os.RemoveAll(installPath); err != nil {
		fmt.Printf("%s Failed to remove installation directory %s: %v\n", red("✗"), installPath, err)
		return
	}

	fmt.Printf("%s Successfully uninstalled %s version %s.\n", green("✓"), language, version)
}
