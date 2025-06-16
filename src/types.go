package src

type App struct {
	Packages      map[string]Package `json:"packages"`
	CheckCmd      string             `json:"checkCmd"`
	InstallCmd    string             `json:"installCmd"`
	CreateCommand string             `json:"createCommand"`
}

type Package struct {
	Apt      string `json:"apt"`
	Pacman   string `json:"pacman"`
	Scoop    string `json:"scoop"`
	Brew     string `json:"brew"`
	Yum      string `json:"yum"`
	Dnf      string `json:"dnf"`
	Apk      string `json:"apk"`
	CheckCmd string `json:"checkCmd"`
}
