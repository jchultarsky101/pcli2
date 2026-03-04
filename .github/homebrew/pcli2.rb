# Homebrew formula for PCLI2
# Install with: brew install pcli2
# Or from tap: brew install jchultarsky101/pcli2/pcli2

class Pcli2 < Formula
  desc "Physna Command Line Interface v2 - Advanced 3D Geometry Search and Analysis"
  homepage "https://github.com/jchultarsky101/pcli2"
  url "https://github.com/jchultarsky101/pcli2/archive/refs/tags/v0.2.35.tar.gz"
  sha256 :no_check # Replace with actual SHA256 checksum for release
  version "0.2.35"
  license "Apache-2.0"

  head "https://github.com/jchultarsky101/pcli2.git", branch: "main"

  depends_on "rust" => :build
  depends_on "pkg-config" => :build
  depends_on "openssl@3"

  def install
    # Build the project
    system "cargo", "install", *std_cargo_args

    # Generate shell completions
    bin.mkpath
    system "#{bin}/pcli2", "completions", "bash", ">", "#{bash_completion}/pcli2"
    system "#{bin}/pcli2", "completions", "zsh", ">", "#{zsh_completion}/_pcli2"
    system "#{bin}/pcli2", "completions", "fish", ">", "#{fish_completion}/pcli2.fish"
  end

  test do
    # Test version output
    output = shell_output("#{bin}/pcli2 --version")
    assert_match "pcli2", output

    # Test help output
    output = shell_output("#{bin}/pcli2 --help")
    assert_match "Commands:", output

    # Test that main subcommands exist
    assert_match "tenant", output
    assert_match "folder", output
    assert_match "asset", output
    assert_match "auth", output
    assert_match "environment", output
  end
end
