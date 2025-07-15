cargo install cross --git https://github.com/cross-rs/cross

sudo pacman -S docker
sudo systemctl enable --now docker.service
sudo usermod -aG docker $USER
