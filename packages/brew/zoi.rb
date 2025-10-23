# Maintainer: ZilloweZ <zillowez@gmail.com>

class Zoi < Formula
  desc "Universal Package Manager & Environment Setup Tool"
  homepage "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi"
  version "1.1.1"
  _tag = "Prod-Release-1.1.1"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-macos-arm64.tar.zst"
      sha256 "f0d391e17366fb3df65bd64c65be09539539cf12d8d8df0328da64cfa548213b"
    end

    if Hardware::CPU.intel?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-macos-amd64.tar.zst"
      sha256 "8c4d98b9ac3c387f388bfe0d7001cb8007f463f984c74b9a053e7385449159fc"
    end

    on_linux do
      if Hardware::CPU.intel? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-linux-amd64.tar.zst"
        sha256 "4cfa680e7052cb18d35518c84f7158e483c054946c3ee8b030ff0369e55b0814"
      end

      if Hardware::CPU.arm? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-linux-arm64.tar.zst"
        sha256 "2e8b32e2a928bead9e8bed156562bcf571c950a7537e64bce7f20715ee1dda91"
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
