package src

import (
	"encoding/json"
	"net/http"

	"github.com/hashicorp/go-version"
)

const VersionJSONURL = "https://zusty.codeberg.page/Zoi/@main/app/version.json"

type RemoteVersionConfig struct {
	Latest LatestInfo `json:"latest"`
}
type LatestInfo struct {
	Production  VersionDetails `json:"production"`
	Development VersionDetails `json:"development"`
}
type VersionDetails struct {
	Status  string `json:"status"`
	Version string `json:"version"`
}

var statusOrder = map[string]int{
	"Pre-Alpha":    0,
	"Alpha":        1,
	"Pre-Beta":     2,
	"Beta":         3,
	"Pre-Release":  4,
	"Early-Access": 5,
	"Demo":         6,
	"Release":      7,
}

func FetchRemoteVersionInfo() (*RemoteVersionConfig, error) {
	resp, err := http.Get(VersionJSONURL)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	var config RemoteVersionConfig
	if err := json.NewDecoder(resp.Body).Decode(&config); err != nil {
		return nil, err
	}
	return &config, nil
}

func IsUpdateAvailable(currentBranch, currentStatus, currentVersion string, remote VersionDetails) (bool, error) {
	currentVer, err := version.NewVersion(currentVersion)
	if err != nil {
		return false, err
	}
	remoteVer, err := version.NewVersion(remote.Version)
	if err != nil {
		return false, err
	}

	if remoteVer.GreaterThan(currentVer) {
		return true, nil
	}

	if remoteVer.Equal(currentVer) {
		currentStatusValue, ok1 := statusOrder[currentStatus]
		remoteStatusValue, ok2 := statusOrder[remote.Status]

		if !ok1 || !ok2 {
			return false, nil
		}

		if remoteStatusValue > currentStatusValue {
			return true, nil
		}
	}

	return false, nil
}
