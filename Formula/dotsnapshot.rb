class Dotsnapshot < Formula
  desc "A CLI utility to create snapshots of dotfiles and configuration"
  homepage "https://github.com/tomerlichtash/dotsnapshot"
  version "1.2.0"
  
  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/tomerlichtash/dotsnapshot/releases/download/v1.2.0/dotsnapshot-macos-arm64.tar.gz"
      sha256 "aa506a303f3eee70e8eb15253f3acea87c6f653216ede98eef8ef4d7eb5ee66f"
    else
      url "https://github.com/tomerlichtash/dotsnapshot/releases/download/v1.2.0/dotsnapshot-macos-x86_64.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256_FOR_X86_64"
    end
  end

  on_linux do
    url "https://github.com/tomerlichtash/dotsnapshot/releases/download/v1.2.0/dotsnapshot-linux-x86_64.tar.gz"
    sha256 "REPLACE_WITH_ACTUAL_SHA256_FOR_LINUX"
  end

  depends_on "rust" => :build

  def install
    # Install the binary
    bin.install "dotsnapshot"
    
    # Generate and install shell completions
    generate_completions_from_executable(bin/"dotsnapshot", "--completions")
    
    # Generate and install man page
    (man1/"dotsnapshot.1").write Utils.safe_popen_read(bin/"dotsnapshot", "--man")
  end

  test do
    # Test version command
    assert_match "dotsnapshot 1.2.0", shell_output("#{bin}/dotsnapshot --version")
    
    # Test info command
    output = shell_output("#{bin}/dotsnapshot --info")
    assert_match "dotsnapshot v1.2.0", output
    assert_match "A CLI utility to create snapshots of dotfiles and configuration", output
    
    # Test list command
    output = shell_output("#{bin}/dotsnapshot --list")
    assert_match "Available plugins:", output
    
    # Test help command
    output = shell_output("#{bin}/dotsnapshot --help")
    assert_match "Usage:", output
    
    # Test completions generation
    output = shell_output("#{bin}/dotsnapshot --completions bash")
    assert_match "_dotsnapshot()", output
    
    # Test man page generation
    output = shell_output("#{bin}/dotsnapshot --man")
    assert_match ".TH dotsnapshot 1", output
  end
end