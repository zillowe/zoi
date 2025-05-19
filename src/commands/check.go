package commands

import (
	"fmt"
	"os/exec"
	"regexp"
	"strconv"
	"strings"

	"github.com/fatih/color"
)

func CheckCommand(requiredTools map[string]string) {
	cyan := color.New(color.FgCyan).SprintFunc()

	fmt.Println(cyan("🏥 Running Zoi Environment Health Check\n"))

	fmt.Println(cyan("🔧 Essential Tools:"))
	if len(requiredTools) == 0 {
		fmt.Println("  No essential tools defined.")
	} else {
		for tool, constraint := range requiredTools {
			checkTool(tool, constraint)
		}
	}

	fmt.Println("\n" + cyan("🌐 Network Connectivity:"))
	checkNetwork()
}

func checkConstraint(version, constraint string) bool {
	yellow := color.New(color.FgYellow).SprintFunc()

	if constraint == "" || version == "" || version == "unknown" {
		return true
	}

	re := regexp.MustCompile(`^\s*(==|>=|<=|~|\^)?\s*([\d.]+(?:[.-][\w.-]+)?)\s*$`)
	matches := re.FindStringSubmatch(constraint)

	if len(matches) < 3 {
		fmt.Printf("  %s Invalid constraint format: '%s'. Supported formats: >=1.2, <=3.0.1, ==2.0, ~1.1.4, ^2.3.0\n", yellow("Warning:"), constraint)
		return true
	}

	operator := matches[1]
	constraintVersion := matches[2]

	comparisonResult := compareSemver(constraintVersion, version)

	switch operator {
	case "==", "":
		return comparisonResult == 0
	case ">=":
		return comparisonResult <= 0
	case "<=":
		return comparisonResult >= 0
	case "~":
		if comparisonResult > 0 {
			return false
		}
		constraintParts := strings.Split(constraintVersion, ".")
		versionParts := strings.Split(version, ".")
		if len(constraintParts) < 2 || len(versionParts) < 2 {
			return false
		}
		constraintMajor, _ := strconv.Atoi(constraintParts[0])
		constraintMinor, _ := strconv.Atoi(constraintParts[1])
		versionMajor, _ := strconv.Atoi(versionParts[0])
		versionMinor, _ := strconv.Atoi(versionParts[1])
		return comparisonResult <= 0 && versionMajor == constraintMajor && versionMinor == constraintMinor

	case "^":
		if comparisonResult > 0 {
			return false
		}
		constraintParts := strings.Split(constraintVersion, ".")
		versionParts := strings.Split(version, ".")
		if len(constraintParts) < 1 || len(versionParts) < 1 {
			return false
		}
		constraintMajor, _ := strconv.Atoi(constraintParts[0])
		versionMajor, _ := strconv.Atoi(versionParts[0])

		if constraintMajor == 0 {
			if len(constraintParts) > 1 && len(versionParts) > 1 {
				constraintMinor, _ := strconv.Atoi(constraintParts[1])
				versionMinor, _ := strconv.Atoi(versionParts[1])
				if constraintMinor == 0 {
					return comparisonResult <= 0 && versionMajor == 0 && versionMinor == 0
				}
				return comparisonResult <= 0 && versionMajor == 0 && versionMinor == constraintMinor
			}
			return comparisonResult <= 0 && versionMajor == 0
		}
		return comparisonResult <= 0 && versionMajor == constraintMajor
	default:
		fmt.Printf("  %s Unsupported operator in constraint: '%s'\n", yellow("Warning:"), operator)
		return true
	}
}

func checkTool(tool, constraint string) {
	cyan := color.New(color.FgCyan).SprintFunc()
	green := color.New(color.FgGreen).SprintFunc()
	red := color.New(color.FgRed).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()

	path, err := exec.LookPath(tool)
	if err != nil {
		fmt.Printf("  %s %s: %s\n", red("✗"), tool, "Not found in PATH")
		return
	}

	rawVersionOutput, directErr := runCommandOutput(fmt.Sprintf("%s --version", tool))
	if directErr != nil {
		rawVersionOutput, directErr = runCommandOutput(fmt.Sprintf("%s version", tool))
	}

	if directErr != nil {
		fmt.Printf("  %s %s: %s (at: %s, check failed: %v)\n",
			red("✗"), tool, "Version check failed", cyan(path), directErr)
		return
	}

	version := extractVersion(rawVersionOutput)

	if version == "" {
		fmt.Printf("  %s %s: %s (at: %s, couldn't parse version from: '%s')\n",
			yellow("?"), tool, "Installed, version unknown", cyan(path), strings.TrimSpace(rawVersionOutput))
	} else {
		fmt.Printf("  %s %s: Version %s (at: %s)\n",
			green("✓"), tool, green(version), cyan(path))
		if !checkConstraint(version, constraint) {
			fmt.Printf("    %s Version %s does not meet constraint '%s'\n", yellow("!"), version, constraint)
		}
	}
}
