package src

import (
	"testing"
	"zoi/src"
)

func TestIsUpdateAvailable(t *testing.T) {
	testCases := []struct {
		name           string
		currentBranch  string
		currentStatus  string
		currentVersion string
		remoteDetails  src.VersionDetails
		want           bool
		wantErr        bool
	}{
		{
			name:           "Major version bump",
			currentVersion: "1.5.0",
			remoteDetails:  src.VersionDetails{Version: "2.0.0", Status: "Alpha"},
			want:           true,
			wantErr:        false,
		},
		{
			name:           "Minor version bump",
			currentVersion: "1.2.0",
			remoteDetails:  src.VersionDetails{Version: "1.3.0", Status: "Alpha"},
			want:           true,
			wantErr:        false,
		},
		{
			name:           "Patch version bump",
			currentVersion: "1.2.0",
			remoteDetails:  src.VersionDetails{Version: "1.2.1", Status: "Alpha"},
			want:           true,
			wantErr:        false,
		},
		{
			name:           "No version change",
			currentVersion: "1.2.0",
			remoteDetails:  src.VersionDetails{Version: "1.2.0", Status: "Alpha"},
			want:           false,
			wantErr:        false,
		},
		{
			name:           "Version downgrade",
			currentVersion: "2.0.0",
			remoteDetails:  src.VersionDetails{Version: "1.9.9", Status: "Release"},
			want:           false,
			wantErr:        false,
		},
		{
			name:           "Status promotion (Alpha to Beta)",
			currentStatus:  "Alpha",
			currentVersion: "1.2.0",
			remoteDetails:  src.VersionDetails{Version: "1.2.0", Status: "Beta"},
			want:           true,
			wantErr:        false,
		},
		{
			name:           "Status demotion (Beta to Alpha)",
			currentStatus:  "Beta",
			currentVersion: "1.2.0",
			remoteDetails:  src.VersionDetails{Version: "1.2.0", Status: "Alpha"},
			want:           false,
			wantErr:        false,
		},
		{
			name:           "Same status",
			currentStatus:  "Release",
			currentVersion: "1.2.0",
			remoteDetails:  src.VersionDetails{Version: "1.2.0", Status: "Release"},
			want:           false,
			wantErr:        false,
		},
		{
			name:           "Invalid current version string",
			currentVersion: "invalid-version",
			remoteDetails:  src.VersionDetails{Version: "1.0.0"},
			want:           false,
			wantErr:        true,
		},
		{
			name:           "Invalid remote version string",
			currentVersion: "1.0.0",
			remoteDetails:  src.VersionDetails{Version: "invalid-version"},
			want:           false,
			wantErr:        true,
		},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			if tc.currentStatus == "" {
				tc.currentStatus = "Alpha"
			}

			got, err := src.IsUpdateAvailable(tc.currentBranch, tc.currentStatus, tc.currentVersion, tc.remoteDetails)

			if (err != nil) != tc.wantErr {
				t.Fatalf("IsUpdateAvailable() error = %v, wantErr %v", err, tc.wantErr)
			}

			if !tc.wantErr && got != tc.want {
				t.Errorf("IsUpdateAvailable() = %v, want %v", got, tc.want)
			}
		})
	}
}
