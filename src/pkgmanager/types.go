package pkgmanager

type PackageManagerConfig struct {
	Endpoint string             `json:"endpoint"`
	Revision string             `json:"revision"`
	Packages map[string]Package `json:"packages"`
}

type Package struct {
	Name    string `json:"name"`
	Desc    string `json:"desc"`
	PkgFile string `json:"pkgFile"`
	Version string `json:"version"`
}

type PackageRecipe struct {
	PackageInfo PackageDetails `yaml:"package"`
	Build       BuildInfo      `yaml:"build"`
	Depends     []Dependency   `yaml:"depends"`
}

type PackageDetails struct {
	Name    string `yaml:"name"`
	Desc    string `yaml:"desc"`
	Handle  string `yaml:"handle"`
	Website string `yaml:"website"`
	Repo    string `yaml:"repo"`
	Version string `yaml:"version"`
	Bin       string `yaml:"bin,omitempty"`
	Installer string `yaml:"installer,omitempty"`
}

type BuildInfo struct {
	Cmd     string       `yaml:"cmd"`
	Bin     string       `yaml:"bin"`
	Depends []Dependency `yaml:"depends"`
}

type Dependency struct {
	Handle  string        `yaml:"handle"`
	Version string        `yaml:"version"`
	Install InstallMethod `yaml:"install"`
}

type InstallMethod struct {
	PM     string `yaml:"pm"`
	Handle string `yaml:"handle"`
}
