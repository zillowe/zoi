# Maintainer: ZilloweZ <zillowez@gmail.com>

class Zoi < Formula
  desc "Universal Package Manager & Environment Setup Tool"
  homepage "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi"
  version "3.1.8-beta" 
  _tag = "Prod-Beta-3.1.8"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-macos-arm64.tar.zst"
      sha512 "df0b92a889db759ba9724100f75a4aabbc4c8fbb4cf737f4799f580b244556c1ea4a05bd1647d290853c218762f25d16f5fb66dc3ca0fc3fa397e70b664e4843"
    end

    if Hardware::CPU.intel?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-macos-amd64.tar.zst"
      sha512 "08b287ab3779798897ac514705b91d7e398d7498cf0885818d8853576d9a8db8100e194530e99301c221771dfb6f16e404cccae5735fc67c5d602ca641311946"
    end

    on_linux do
      if Hardware::CPU.intel? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-linux-amd64.tar.zst"
        sha512 "dbb93e64fe5735fa76453737d4314cf7bc6a8e4614bc8009502d22235d337c0afd93ee7da43249377be8ea6d17d63604ac09929cff39d74dda19b4176dc4664d"
      end

      if Hardware::CPU.arm? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-linux-arm64.tar.zst"
        sha512 "f44ae9c1849ec3aa16eec870fd2db831ef9faacc50f9daab64f2d62e5e3d558aa3dbc7802d5721c69df38b61a1bb62b69fe90a81a7c406473602d5cb637ea816"
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
