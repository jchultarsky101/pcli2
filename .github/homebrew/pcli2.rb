# Homebrew formula for PCLI2
# Install with:
#   brew tap jchultarsky101/pcli2
#   brew install pcli2
#
# Or directly:
#   brew install jchultarsky101/pcli2/pcli2

class Pcli2 < Formula
  desc "Physna Command Line Interface v2 - Advanced 3D Geometry Search and Analysis"
  homepage "https://github.com/jchultarsky101/pcli2"
  url "https://github.com/jchultarsky101/pcli2/archive/refs/tags/v1.0.0.tar.gz"
  sha256 "ef1ebda08e92fee175b437ace43c9dcb0916906ee18ebf681f2759c063317c7a"
  license "Apache-2.0"

  head "https://github.com/jchultarsky101/pcli2.git", branch: "main"

  depends_on "rust" => :build
  depends_on "pkg-config" => :build
  depends_on "openssl@3"

  def install
    # Build the project
    system "cargo", "install", *std_cargo_args, "--locked"

    # Generate shell completions
    bin.mkpath
    system "#{bin}/pcli2", "completions", "bash", ">", "#{bash_completion}/pcli2"
    system "#{bin}/pcli2", "completions", "zsh", ">", "#{zsh_completion}/_pcli2"
    system "#{bin}/pcli2", "completions", "fish", ">", "#{fish_completion}/pcli2.fish"
  end

  test do
    # Test version output
    output = shell_output("#{bin}/pcli2 --version")
    assert_match "pcli2 1.0.0", output

    # Test help output
    output = shell_output("#{bin}/pcli2 --help")
    assert_match "Commands:", output

    # Test that main subcommands exist
    assert_match "tenant", output
    assert_match "folder", output
    assert_match "asset", output
    assert_match "auth", output
    assert_match "environment", output
    
    # Test config validate command
    output = shell_output("#{bin}/pcli2 config validate --help")
    assert_match "Validate configuration", output
  end
end
