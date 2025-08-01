# Maintainer: ZilloweZ <zillowez@gmail.com>

class Zoi < Formula
  desc "Universal Package Manager & Environment Setup Tool"
  homepage "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi"
  version "3.2.6-beta" 
  _tag = "Prod-Beta-3.2.6"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-macos-arm64.tar.zst"
      sha512 "e7de1225f79db1a9bba1079fdabf4a01382a69a2b150dbea8d508eecc19c5127548b1ec7d438c889012bae29b04ad49d6e9f16012aefd8a3bd286f37ff50ec2b"
    end

    if Hardware::CPU.intel?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-macos-amd64.tar.zst"
      sha512 "e6ae1c35539314aed7288e09e7b21d78276127369c094e77a7dae685d41e6e6934da06ec2efea3c57cc6f710b0d8cb8c7bad137ccac9c0a5302d3fd2cd0d01a1"
    end

    on_linux do
      if Hardware::CPU.intel? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-linux-amd64.tar.zst"
        sha512 "06d82cd2ebcdf82bfd98e10e5ce47eeb9cf343a51305f0f1af8593cc3de667eb223155cda25b6ee480d47f762e67b9da7e4a213a1da029edd668f0715f65fff8"
      end

      if Hardware::CPU.arm? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-linux-arm64.tar.zst"
        sha512 "bbd32d27a169f5716ce84040afe019ddc152e6d7572a627889107e4975d2fd4eb0d346654c1cead344398a79e3f062d20cc262ca020821e697fe5e50b1721b80"
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
