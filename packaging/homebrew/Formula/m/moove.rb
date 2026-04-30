class Moove < Formula
  desc "Manipulate file names and locations using a text editor"
  homepage "https://github.com/urin/moove"
  url "https://github.com/urin/moove/archive/refs/tags/v0.4.3.tar.gz"
  sha256 "e93fc9439d1e8a6b26629c0a7efaa8b00afb8709088c6651e917751452353a8b"
  license any_of: ["MIT", "Apache-2.0"]
  head "https://github.com/urin/moove.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/moove --version")
  end
end
