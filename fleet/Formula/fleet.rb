class Fleet < Formula
  desc "Local macOS CLI for controlled agent worktrees"
  homepage "https://github.com/OWNER/REPOSITORY"
  version "0.1.0"
  url "https://github.com/OWNER/REPOSITORY/releases/download/v#{version}/fleet-v#{version}-darwin-universal.tar.gz"
  sha256 "REPLACE_WITH_RELEASE_SHA256"

  def install
    bin.install "fleet"
  end
end
