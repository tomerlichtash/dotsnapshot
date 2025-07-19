class Dotsnapshot < Formula
  desc "A CLI utility to create snapshots of dotfiles and configuration"
  homepage "https://github.com/tomerlichtash/dotsnapshot"
  version "1.4.2"
  
  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/tomerlichtash/dotsnapshot/releases/download/v1.4.2/dotsnapshot-macos-arm64.tar.gz"
      sha256 "c880466b2cad2af96f452c57fefa8e5f60936dd21e683f9bb0db62887e134c0b"
    else
      url "https://github.com/tomerlichtash/dotsnapshot/releases/download/v1.4.2/dotsnapshot-macos-x86_64.tar.gz"
      sha256 "313adfb48ffea49a7e94bb82a254b43fc5e1a95d27380b23f1b2f16adfce85ad"
    end
  end

  on_linux do
    url "https://github.com/tomerlichtash/dotsnapshot/releases/download/v1.4.2/dotsnapshot-linux-x86_64.tar.gz"
    sha256 "75f62591596a8950b4e7fa5beb5243c1ac0a89199969d2d2283e5673b6f8c846"
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
    assert_match "dotsnapshot 1.4.2", shell_output("#{bin}/dotsnapshot --version")
    
    # Test info command
    output = shell_output("#{bin}/dotsnapshot --info")
    assert_match "dotsnapshot v1.4.2", output
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