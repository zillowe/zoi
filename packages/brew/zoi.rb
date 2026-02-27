# Maintainer: ZilloweZ <zillowez@gmail.com>

class Zoi < Formula
  desc "Universal Package Manager & Environment Setup Tool"
  homepage "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi"
  version "1.7.0"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/Prod-Release-#{version}/downloads/zoi-macos-arm64.tar.zst"
      sha256 "32533f9dc556199bdf6c1f04fb80cbd3ebb63d68a693f6d9926cc6b5d93f56cb"
    end

    if Hardware::CPU.intel?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/Prod-Release-#{version}/downloads/zoi-macos-amd64.tar.zst"
      sha256 "99e6bd5ce6b74aabc15bba00d2ed07060f399f52c59edf47e19743908b4547d3"
    end

    on_linux do
      if Hardware::CPU.intel? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/Prod-Release-#{version}/downloads/zoi-linux-amd64.tar.zst"
        sha256 "a51ef068526941f4781aab2d26d940a93478e6d8e8e544a413869477f1a16344"
      end

      if Hardware::CPU.arm? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/Prod-Release-#{version}/downloads/zoi-linux-arm64.tar.zst"
        sha256 "1fd8a6d5cfabad8b6afcc71e95039b96abd541c079e10be18fba62a617d1cb1a"
      end
    end

  end

  def install
    bin.install "zoi"
    (bash_completion/"zoi").write `#{bin}/zoi generate-completions bash`
    (zsh_completion/"_zoi").write `#{bin}/zoi generate-completions zsh`
    (fish_completion/"zoi.fish").write `#{bin}/zoi generate-completions fish`
  end
end
