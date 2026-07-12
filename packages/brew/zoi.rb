# Maintainer: ZilloweZ <zillowez@gmail.com>

class Zoi < Formula
  desc "Advanced Package Manager & Environment Orchestrator"
  homepage "https://gitlab.com/zillowe/zillwen/zusty/zoi"
  version "__VERSION__"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://gitlab.com/zillowe/zillwen/zusty/zoi/-/releases/Prod-Release-#{version}/downloads/zoi-macos-arm64.tar.zst"
      sha256 "__SHA256_MACOS_ARM64__"
    end

    if Hardware::CPU.intel?
      url "https://gitlab.com/zillowe/zillwen/zusty/zoi/-/releases/Prod-Release-#{version}/downloads/zoi-macos-amd64.tar.zst"
      sha256 "__SHA256_MACOS_AMD64__"
    end

    on_linux do
      if Hardware::CPU.intel? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/zillowe/zillwen/zusty/zoi/-/releases/Prod-Release-#{version}/downloads/zoi-linux-amd64.tar.zst"
        sha256 "__SHA256_LINUX_AMD64__"
      end

      if Hardware::CPU.arm? and Hardware::CPU.is_64_bit?
        url "https://gitlab.com/zillowe/zillwen/zusty/zoi/-/releases/Prod-Release-#{version}/downloads/zoi-linux-arm64.tar.zst"
        sha256 "__SHA256_LINUX_ARM64__"
      end
    end

  end

  def install
    bin.install "zoi"
    (bash_completion/"zoi").write `#{bin}/zoi generate-completions bash`
    (zsh_completion/"_zoi").write `#{bin}/zoi generate-completions zsh`
    (fish_completion/"zoi.fish").write `#{bin}/zoi generate-completions fish`

    system "#{bin}/zoi", "generate-manual"
    man1.install Dir["manuals/*.1"]
  end
end
