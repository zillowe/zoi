# Maintainer: ZilloweZ <zillowez@gmail.com>

class Zoi < Formula
  desc "Universal Package Manager & Environment Setup Tool"
  homepage "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi"
  version "ZOI_VERSION"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/Prod-Release-#{version}/downloads/zoi-macos-arm64.tar.zst"
      sha256 "MACOS_ARM64_SHA256"
    end

    if Hardware::CPU.intel?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/Prod-Release-#{version}/downloads/zoi-macos-amd64.tar.zst"
      sha256 "MACOS_AMD64_SHA256"
    end

    on_linux do
      if Hardware::CPU.intel? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/Prod-Release-#{version}/downloads/zoi-linux-amd64.tar.zst"
        sha256 "LINUX_AMD64_SHA256"
      end

      if Hardware::CPU.arm? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/Prod-Release-#{version}/downloads/zoi-linux-arm64.tar.zst"
        sha256 "LINUX_ARM64_SHA256"
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
