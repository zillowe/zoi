# Maintainer: ZilloweZ <zillowez@gmail.com>

class Zoi < Formula
  desc "Universal Package Manager & Environment Setup Tool"
  homepage "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi"
  
  version "2.5.0-beta"
  
  _tag = "Prod-Beta-2.5.0"

  license "Apache-2.0"

  if Hardware::CPU.arm?
    url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-macos-arm64.tar.xz"
    sha256 "7f01a764d6612ce2e0a934cdc88168080b290f0452e9cacaf99eab182c4c92e9"
  else
    url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-macos-amd64.tar.xz"
    sha256 "f5e04a0bc7db13393946fad4b86871999c1b1a536d87d3faf3f0220ae8d103a5"
  end

  def install
    bin.install "zoi"

    (bash_completion/"zoi").write `#{bin}/zoi generate-completions bash`

    (zsh_completion/"_zoi").write `#{bin}/zoi generate-completions zsh`

    (fish_completion/"zoi.fish").write `#{bin}/zoi generate-completions fish`
  end

  test do
    assert_match "zoi #{version}", shell_output("#{bin}/zoi --version")
  end
end
