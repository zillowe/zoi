echo "--- Updating package manager files ---"
mkdir -p ~/.ssh
echo "$SSH_PRIVATE_KEY" | base64 -d > ~/.ssh/id_rsa
chmod 600 ~/.ssh/id_rsa
ssh-keyscan -H aur.archlinux.org >> ~/.ssh/known_hosts
ssh-keyscan -H github.com >> ~/.ssh/known_hosts
git config --global user.email "contact@zillowe.qzz.io"
git config --global user.name "Zillowe CI/CD"
VERSION=$(echo "$CI_COMMIT_MESSAGE" | sed 's/Release: Bump packages version to //')

echo "--- AUR ---"
git clone "ssh://aur@aur.archlinux.org/zoi-bin.git" aur_zoi_bin
cd aur_zoi_bin
curl -o PKGBUILD https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/packages/aur/zoi-bin/PKGBUILD
curl -o .SRCINFO https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/packages/aur/zoi-bin/.SRCINFO
if [[ -n $(git status --porcelain) ]]; then
  echo "Committing and pushing package updates..."
  git add .
  git commit -m "Release: Bump package version to ${VERSION}"
  git push origin master
else
  echo "No changes detected, skipping commit"
fi
cd ..
git clone "ssh://aur@aur.archlinux.org/zoi.git" aur_zoi
cd aur_zoi
curl -o PKGBUILD https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/packages/aur/zoi/PKGBUILD
curl -o .SRCINFO https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/packages/aur/zoi/.SRCINFO
if [[ -n $(git status --porcelain) ]]; then
  echo "Committing and pushing package updates..."
  git add .
  git commit -m "Release: Bump package version to ${VERSION}"
  git push origin master
else
  echo "No changes detected, skipping commit"
fi
cd ..

echo "--- Homebrew ---"
git clone "ssh://git@github.com/Zillowe/homebrew-tap" brew_zoi
cd brew_zoi
curl -o zoi.rb https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/packages/brew/zoi.rb
if [[ -n $(git status --porcelain) ]]; then
  echo "Committing and pushing package updates..."
  git add .
  git commit -m "Release: Bump package version to ${VERSION}"
  git push origin main
else
  echo "No changes detected, skipping commit"
fi
cd ..

echo "--- Scoop ---"
git clone "ssh://git@github.com/Zillowe/scoop.git" scoop_zoi
cd scoop_zoi
cd bucket
curl -o zoi.json https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/packages/scoop/zoi.json
if [[ -n $(git status --porcelain) ]]; then
  echo "Committing and pushing package updates..."
  git add .
  git commit -m "Release: Bump package version to ${VERSION}"
  git push origin main
  else
  echo "No changes detected, skipping commit"
fi
cd ../..
