class Zoi < Formula
  desc "Universal Package Manager & Environment Setup Tool"
  homepage "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi"
  
  version "2.4.0-beta"
  
  _tag = "Prod-Beta-2.4.0"

  license "Apache-2.0"

  if Hardware::CPU.arm?
    url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-darwin-arm64.tar.xz"
    sha256 "0ca063ddbc81d1fdb615ae4fb5e514e7deb8cea92192a8d5d65931aebbed449a"
  else
    url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-darwin-amd64.tar.xz"
    sha256 "fb5fc2513feefd8e0819424909bfa4d262dbfbd10e082ab94ae4e00fa760aae4"
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
