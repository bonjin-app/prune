# Edited here, in the Prune repository. The copy in bonjin-app/homebrew-tap is written by
# scripts/update-cask.sh --publish; a change made there is overwritten by the next release.
cask "prune" do
  arch arm: "aarch64", intel: "x64"

  version "0.1.3"
  sha256 arm:   "0a18597b5711178bd5deb3f1fe9d1f5b04d4cc08b403babbd437fc107bdd3945",
         intel: "3d4d6f98d11df5a51e399270b98cbf56780298670bd0fbecee46779eef9a747e"

  url "https://github.com/bonjin-app/prune/releases/download/v#{version}/Prune_#{version}_#{arch}.dmg"
  name "Prune"
  desc "Local-first cleaner for caches, developer leftovers and disk space"
  homepage "https://github.com/bonjin-app/prune"

  livecheck do
    url :url
    strategy :github_latest
  end

  # The bundle asks for 12.0, so an older system would install a copy it cannot open.
  depends_on macos: :monterey

  app "Prune.app"

  # Everything Prune leaves behind. The first is the one that matters: settings.json and the
  # operation log, which is the record of what was removed. `brew uninstall` keeps it; only
  # `brew zap` throws it away.
  zap trash: [
    "~/Library/Application Support/app.bonjin.prune",
    "~/Library/Caches/app.bonjin.prune",
    "~/Library/HTTPStorages/app.bonjin.prune",
    "~/Library/Preferences/app.bonjin.prune.plist",
    "~/Library/Saved Application State/app.bonjin.prune.savedState",
    "~/Library/WebKit/app.bonjin.prune",
  ]

  caveats do
    <<~EOS
      Prune is not signed with a Developer ID yet, so `spctl` rejects this bundle and macOS
      quarantines it on download. The first launch is refused. Try to open it, then allow it
      in System Settings -> Privacy & Security. If macOS calls the app damaged instead of
      unverified, that dialog offers no way through, and the quarantine flag has to go:

        xattr -d com.apple.quarantine /Applications/Prune.app

      Neither message is a statement about what the app does. Both mean nobody has paid for a
      certificate that vouches for it.

      The `prune` command line ships separately, from the same release:
        https://github.com/bonjin-app/prune/releases/latest
    EOS
  end
end
