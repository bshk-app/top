require "json"

manifest = JSON.parse(File.read(ARGV.fetch(0)))
version = manifest.fetch("releaseVersion")
abort "Only stable versions can update the Homebrew cask." unless version.match?(/\A\d+\.\d+\.\d+(?:\+[0-9A-Za-z.-]+)?\z/)
digest = manifest.fetch("sha256")
abort "Invalid artifact digest." unless digest.match?(/\A[0-9a-f]{64}\z/)
template = <<~'CASK'
  cask "top" do
    version @VERSION@
    sha256 @SHA256@

    url "https://github.com/bshk-app/top/releases/download/v#{version}/Top-#{version}-macos-arm64.dmg"
    name "Top"
    desc "Apple Silicon system monitor"
    homepage "https://github.com/bshk-app/top"

    livecheck do
      url "https://raw.githubusercontent.com/bshk-app/top/updates/appcast-stable.xml"
      strategy :sparkle, &:short_version
    end

    auto_updates true
    depends_on arch: :arm64
    depends_on macos: :sonoma

    app "Top.app"

    uninstall quit: "app.bshk.top"

    zap trash: [
      "~/Library/Application Support/Top",
      "~/Library/Caches/app.bshk.top",
      "~/Library/HTTPStorages/app.bshk.top",
      "~/Library/Preferences/app.bshk.top.plist",
    ]
  end
CASK
puts template.sub("@VERSION@", version.inspect).sub("@SHA256@", digest.inspect)
