cask "stitchcraft" do
  version "__VERSION__"
  sha256 "__CHECKSUM__"

  url "https://github.com/jbcaleb/stitchcraft/releases/download/__TAG__/stitchcraft-macos-arm64.app.zip"
  name "Stitchcraft"
  desc "Build Minecraft mods for Fabric and NeoForge with blocks"
  homepage "https://github.com/jbcaleb/stitchcraft"

  depends_on arch: :arm64

  app "Stitchcraft.app"

  # The app is only ad-hoc signed (no Developer ID), so drop the quarantine flag
  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-dr", "com.apple.quarantine", "#{appdir}/Stitchcraft.app"]
  end

  zap trash: [
    "~/Library/Application Support/Stitchcraft",
    "~/Library/Caches/Stitchcraft",
  ]

  caveats <<~EOS
    Stitchcraft is not notarized by Apple. This cask removes the quarantine
    attribute after install so it can launch.
  EOS
end
