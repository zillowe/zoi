# Maintainer: ZilloweZ <zillowez@gmail.com>

class Zoi < Formula
  desc "Universal Package Manager & Environment Setup Tool"
  homepage "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi"
  version "3.2.7-beta" 
  _tag = "Prod-Beta-3.2.7"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-macos-arm64.tar.zst"
      sha512 "a7cfeff099ce7ddf667ec97624eda0f2cef7cb602705f07720b4b9834c836d8de10db08cca178519b72704f5eb4b543cbc3bf3f9c48c943e7fbbb92705767a27"
    end

    if Hardware::CPU.intel?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-macos-amd64.tar.zst"
      sha512 "0a0d98670f162561be0005cb244dc5fa63cef188e2c2835c7b2b2b6a4a4da662319c80f38a22e083f4febf2f8d3fdef05a8adf25e80e15b0b3f65a5e9ff36623"
    end

    on_linux do
      if Hardware::CPU.intel? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-linux-amd64.tar.zst"
        sha512 "80893110edaef73b80d06d98e076e24a8022134e13256b5d452c5a3b36d5154541fbabdeab4a59cb47e64a74ea4e84e4acf24bdeaf7f89e47e77063bdadcdd93"
      end

      if Hardware::CPU.arm? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-linux-arm64.tar.zst"
        sha512 "a023b4a3c74ef7a651b441e95b6031f4ada91e9e0cd70cc06aa9df0583ee6531b6848c5cefb465ebf1c37a9ec2796902dc77a147a6cb9215857c93a4162111be"
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
