# Maintainer: ZilloweZ <zillowez@gmail.com>

class Zoi < Formula
  desc "Universal Package Manager & Environment Setup Tool"
  homepage "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi"
  version "2.5.5-beta" 
  _tag = "Prod-Beta-2.5.5"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-macos-arm64.tar.xz"
      sha256 "137c5e1ae31e33a979cd85b1c46369f4024e249cdacf19fde9699d2499297f35"
    end

    if Hardware::CPU.intel?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-macos-amd64.tar.xz"
      sha256 "53c0d1f0ee81b4f05c2c97af98ea7197d365a3b37de0fa34227d6ea88b28ceda"
    end

    on_linux do
      if Hardware::CPU.intel? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-linux-amd64.tar.xz"
        sha256 "6e45e7783fd4b7f1e272d66bf6ebf6257be9402e29ab89aa86fa86e1ae6b44bc"
      end

      if Hardware::CPU.arm? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-linux-arm64.tar.xz"
        sha256 "1ba206a2ef3b9cd63f1e9f6680cf51a7ea77d566100981363be187eee9ce7228"
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
