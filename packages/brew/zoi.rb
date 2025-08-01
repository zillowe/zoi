# Maintainer: ZilloweZ <zillowez@gmail.com>

class Zoi < Formula
  desc "Universal Package Manager & Environment Setup Tool"
  homepage "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi"
  version "3.2.5-beta" 
  _tag = "Prod-Beta-3.2.5"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-macos-arm64.tar.zst"
      sha512 "ee60ffbf94c8edba5d55804ebdd1d3783e5a51f3a946e0c0932545c9aa2fc1a79e3163b487de24eabd19d9b149060e6afb7f57d19e94c5d47eeffd1fcda1eb12"
    end

    if Hardware::CPU.intel?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-macos-amd64.tar.zst"
      sha512 "928cbe0c32d99884fdbab3c008c35039cb80ce9bc9e5718f296153d179a2fc3d37701946cd83bf7d6a78f28d12a4e74316b3ff6ca6c70b238c41b0df5466a43b"
    end

    on_linux do
      if Hardware::CPU.intel? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-linux-amd64.tar.zst"
        sha512 "05cbb240dc9fa1c74b8e3b83bb39743ba2944147fff9cef32ec6d7909da4de379a9d19311b0b36a490240e62382dbd6fd38a7d0021f96b4784cc7d83e88d7b29"
      end

      if Hardware::CPU.arm? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-linux-arm64.tar.zst"
        sha512 "8ddc15f0104e5d30aa8076be4f9a79bf10dd0ba147a2f69dbab88db33ff4c5c5a3bfacb47ef86cba876e84231b12ef18cf246f5384870ad11e96203354c972b9"
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
