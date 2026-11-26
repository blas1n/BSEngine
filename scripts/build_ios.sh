#!/usr/bin/env bash
# Builds a project for iOS: an app bundle holding
#
#   <Name>.app/bsengine-runtime   the engine: the runtime, built for the target
#   <Name>.app/game.pak           the game's data, with its manifest inside
#   <Name>.app/Info.plist         what iOS reads to launch it
#
# for the Simulator on Apple silicon (`--sim`, the default), signed ad hoc --
# all a Simulator asks -- or for a device (`--device`), unsigned: an app for a
# phone has to be signed with an Apple team's certificate and profile, which
# this script does not hold. Signing for a device is Xcode's job, as it is for
# Unity's, Unreal's and Godot's iOS exports -- which is what `--xcode` makes:
#
#   <out>/<Name>/<Name>.xcodeproj   the project: open it, pick a Team, Run
#   <out>/<Name>/BSEngine.xcframework  the engine, a static library for
#                                   devices and for the Simulator
#   <out>/<Name>/App/main.m         `return bsengine_ios_main();`
#   <out>/<Name>/App/game.pak       the game's data, a bundle resource
#   <out>/<Name>/App/Info.plist     filled in by Xcode's build settings
#   <out>/<Name>/App/Assets.xcassets   a plain default app icon to replace
#
# The `[ios]` table becomes the project's build settings: bundle id, version
# (MARKETING_VERSION), build (CURRENT_PROJECT_VERSION) and team_id
# (DEVELOPMENT_TEAM; without it Xcode asks for a team before a device build).
#
# The bundle id, version and build are `project.toml`'s `[ios]` table, or
# `com.bsengine.<name>`, "1.0" and "1" without one (`ios_settings.rs` in
# bsengine-runtime resolves and checks them). The oldest iOS it runs on is
# IPHONEOS_DEPLOYMENT_TARGET from .cargo/config.toml (iOS 15).
#
# usage: scripts/build_ios.sh <project-dir> [out-dir] [--sim|--device|--xcode] [--debug]
#   out-dir defaults to <project-dir>/dist-ios
#
# Needs: macOS with Xcode (the iOS SDK, its clang and libclang -- QuickJS's
# bindings are generated for the target at build time), and the Rust target
# (`rustup target add aarch64-apple-ios-sim` or `aarch64-apple-ios`).
#
# Run in a booted Simulator:
#   xcrun simctl install booted <out>/<Name>.app
#   xcrun simctl launch --console-pty booted <bundle id> [--frames N]
set -euo pipefail

profile=release
dest=sim
args=()
while [[ $# -gt 0 ]]; do
  case $1 in
    --debug) profile=debug ;;
    --sim) dest=sim ;;
    --device) dest=device ;;
    --xcode) dest=xcode ;;
    *) args+=("$1") ;;
  esac
  shift
done
if [[ ${#args[@]} -lt 1 || ${#args[@]} -gt 2 ]]; then
  sed -n 's/^# usage: //p' "$0" >&2
  exit 2
fi
project=${args[0]}
out=${args[1]:-$project/dist-ios}

fail() { echo "build_ios: $*" >&2; exit 1; }
[[ $(uname -s) == Darwin ]] || fail "needs macOS with Xcode: the iOS SDK exists nowhere else"
[[ -f $project/project.toml ]] || fail "$project has no project.toml"

case $dest in
  sim) triples=(aarch64-apple-ios-sim); platform=iPhoneSimulator ;;
  device) triples=(aarch64-apple-ios); platform=iPhoneOS ;;
  xcode) triples=(aarch64-apple-ios aarch64-apple-ios-sim) ;;
esac
for t in "${triples[@]}"; do
  rustup target list --installed | grep -qx "$t" ||
    fail "the Rust target is missing: rustup target add $t"
done

flag=
[[ $profile == release ]] && flag=--release

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT

# The game's data, packaged by the host's runtime: the target's cannot run here.
cargo run $flag -p bsengine-runtime -- --package "$project" --mode ios --out "$stage/pak"
prop() { sed -n "s/^$1=//p" "$stage/pak/ios.properties"; }
bundle_id=$(prop bundle_id)
name_xml=$(prop name_xml)
version=$(prop version)
build=$(prop build)
[[ -n $bundle_id && -n $version && -n $build ]] ||
  fail "--mode ios wrote no bundle id, version or build"

team_id=$(prop team_id)
min_ios=$(sed -n 's/^IPHONEOS_DEPLOYMENT_TARGET *= *"\(.*\)"/\1/p' .cargo/config.toml)
[[ -n $min_ios ]] || fail ".cargo/config.toml sets no IPHONEOS_DEPLOYMENT_TARGET"

# The bundle's directory name is the project's folder name: a file name, which
# the display name (with spaces, `&`, ...) need not be.
app_name=$(basename "$(cd "$project" && pwd)")

if [[ $dest == xcode ]]; then
  source "$(dirname "$0")/ios_xcode_export.sh"
  exit 0
fi

triple=${triples[0]}
cargo build $flag -p bsengine-runtime --target "$triple"
exe=target/$triple/$profile/bsengine-runtime
app=$out/$app_name.app
rm -rf "$app"
mkdir -p "$app"
cp "$exe" "$app/bsengine-runtime"
cp "$stage/pak/game.pak" "$app/game.pak"

# `UILaunchScreen` (even empty) is what makes iOS give the app the whole
# screen: without a launch screen an app runs letterboxed at an old iPhone's
# size. `UIRequiredDeviceCapabilities` arm64: the engine is 64-bit only.
cat > "$app/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleExecutable</key><string>bsengine-runtime</string>
  <key>CFBundleIdentifier</key><string>$bundle_id</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>$name_xml</string>
  <key>CFBundleDisplayName</key><string>$name_xml</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$build</string>
  <key>CFBundleSupportedPlatforms</key><array><string>$platform</string></array>
  <key>MinimumOSVersion</key><string>$min_ios</string>
  <key>LSRequiresIPhoneOS</key><true/>
  <key>UIDeviceFamily</key><array><integer>1</integer><integer>2</integer></array>
  <key>UILaunchScreen</key><dict/>
  <key>UIRequiredDeviceCapabilities</key><array><string>arm64</string></array>
  <key>UISupportedInterfaceOrientations</key>
  <array>
    <string>UIInterfaceOrientationPortrait</string>
    <string>UIInterfaceOrientationLandscapeLeft</string>
    <string>UIInterfaceOrientationLandscapeRight</string>
  </array>
  <key>UISupportedInterfaceOrientations~ipad</key>
  <array>
    <string>UIInterfaceOrientationPortrait</string>
    <string>UIInterfaceOrientationPortraitUpsideDown</string>
    <string>UIInterfaceOrientationLandscapeLeft</string>
    <string>UIInterfaceOrientationLandscapeRight</string>
  </array>
</dict>
</plist>
PLIST
plutil -lint "$app/Info.plist" >/dev/null || fail "the Info.plist written is not valid"

if [[ $dest == sim ]]; then
  codesign --force --sign - "$app"
fi

echo "ios build: $app ($bundle_id $version ($build), $triple, $profile, iOS $min_ios+)"
if [[ $dest == sim ]]; then
  echo "  xcrun simctl install booted \"$app\" && xcrun simctl launch --console-pty booted $bundle_id"
else
  echo "  unsigned: sign it for a device in Xcode (an Xcode project export is coming)"
fi
