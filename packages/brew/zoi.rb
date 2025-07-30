# Maintainer: ZilloweZ <zillowez@gmail.com>

class Zoi < Formula
  desc "Universal Package Manager & Environment Setup Tool"
  homepage "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi"
  version "3.2.1-beta" 
  _tag = "Prod-Beta-3.2.1"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-macos-arm64.tar.zst"
      sha512 "f171b47804d93620345e5eb5d5ec9f46237fc01c7469f72152f118920ad9f4a48ebbf584797085e3d0d3a6283c2b724bb81d01ec88cb3389d39c2ae6be43e24c"
    end

    if Hardware::CPU.intel?
      url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-macos-amd64.tar.zst"
      sha512 "34d697bc70cd0a642d253b40af106d93e81028b2d56a176f136241f52b189bc2a0ad3cac0ffa8676acb3fb97602eb85e1aacca0fef4c028722e3241f4a76a06d"
    end

    on_linux do
      if Hardware::CPU.intel? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-linux-amd64.tar.zst"
        sha512 "ca463d2bf78792aaf3b922cf8685a21b08ec8b0dd8903b298617a1f2af9cec9eb46d8804e2ee4082d99264e84fe7a99eb1144495d7bd074993a87089c9ba2b23"
      end

      if Hardware::CPU.arm? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/#{_tag}/downloads/zoi-linux-arm64.tar.zst"
        sha512 "ad81d8071b83542406cd9c4d0415d7e82e066bc87fe959cba8e61077fd667dbf053bc5b2cecf7bb1badfe29bb9dc642216996a5051774ea7368a7954cc427a48"
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
