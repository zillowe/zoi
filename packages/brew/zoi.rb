# Maintainer: ZilloweZ <zillowez@gmail.com>

class Zoi < Formula
  desc "Universal Package Manager & Environment Setup Tool"
  homepage "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi"
  version "3.2.2-beta" 
  _tag = "Prod-Beta-3.2.2"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-macos-arm64.tar.zst"
      sha512 "51d26c15e8a43b7cdc6b2282e649ff9e7cecd282f8c672852cf8ac742ba6d15d1909d76c826afd94154c28be0731d3c819469b03b9b2678b561c570eff303d03"
    end

    if Hardware::CPU.intel?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-macos-amd64.tar.zst"
      sha512 "78414772273910c3e951a28d4937bcbf5f81cc84df4b0be515d44b8c7ec9680026bfe9292009fad3a9b07a6f582286f8d55659d12ab8cbaa8cc7e90df0901b38"
    end

    on_linux do
      if Hardware::CPU.intel? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-linux-amd64.tar.zst"
        sha512 "6ad4ec82769eca5e8ccee2fbd8169ca9c13843d008baf0ff50f807fc4d5d199c848ac7b3c8af159b51229a4b55b57f887bc5ef7f2f0002632f4d3d75d78b70eb"
      end

      if Hardware::CPU.arm? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-linux-arm64.tar.zst"
        sha512 "7e09fa8c580c0b2bc1bac693d1e16d1890615d0ce1933b636d4b25ca1f7762ee2743e157e942dc1300e1c3c9f3b45ac0b87710279bbf6894fc1dac80ef066d78"
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
