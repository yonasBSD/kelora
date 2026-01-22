# Homebrew Formula for Kelora
# This file should be placed in the dloss/homebrew-kelora repository
# at Formula/kelora.rb

class Kelora < Formula
  desc "Command-line log analysis tool with embedded Rhai scripting"
  homepage "https://github.com/dloss/kelora"
  version "1.4.2"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/dloss/kelora/releases/download/v1.4.2/kelora-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_ARM64_SHA256"
    else
      url "https://github.com/dloss/kelora/releases/download/v1.4.2/kelora-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_X86_64_SHA256"
    end
  end

  def install
    bin.install "kelora"
  end

  test do
    # Test basic functionality
    assert_match "kelora #{version}", shell_output("#{bin}/kelora --version")

    # Test help output
    assert_match "Command-line log analysis", shell_output("#{bin}/kelora -h")

    # Test with a simple echo pipe
    output = pipe_output("#{bin}/kelora -f json --filter 'true'", '{"level":"info","msg":"test"}')
    assert_match "test", output
  end
end
