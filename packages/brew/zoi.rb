# Maintainer: ZilloweZ <zillowez@gmail.com>

class Zoi < Formula
  desc "Universal Package Manager & Environment Setup Tool"
  homepage "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi"
  version "1.12.1"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/Prod-Release-#{version}/downloads/zoi-macos-arm64.tar.zst"
      sha256 "337957d6f9e7239f862412a8f9f9d5b803800683a9ad0f1d308c4743bbf3f835"
    end

    if Hardware::CPU.intel?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/Prod-Release-#{version}/downloads/zoi-macos-amd64.tar.zst"
      sha256 "ae6d3aa2ba0d41e5d98f0beffff7857ee72aee45d96e3d3c5ec1df2dd28817be"
    end

    on_linux do
      if Hardware::CPU.intel? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/Prod-Release-#{version}/downloads/zoi-linux-amd64.tar.zst"
        sha256 "3ed46071fd244243f00739c0e5c7d0a8b329c12e74601b70f02c9e1eee1aa610"
      end

      if Hardware::CPU.arm? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/Prod-Release-#{version}/downloads/zoi-linux-arm64.tar.zst"
        sha256 "e42c9e46000e92739bad3dbd72feaba04848f8b013f17ae26944446b610c1275"
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
