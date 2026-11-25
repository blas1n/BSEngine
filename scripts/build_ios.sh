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
# Unity's, Unreal's and Godot's iOS exports; an Xcode project export comes
# after this (see the iOS design).
#
# The bundle id is `com.bsengine.<name>` and the version 1 / "1.0", as an
# Android build's were before its `[android]` table (`ios_settings.rs` in
# bsengine-runtime works them out). The oldest iOS it runs on is
# IPHONEOS_DEPLOYMENT_TARGET from .cargo/config.toml (iOS 15).
#
# usage: scripts/build_ios.sh <project-dir> [out-dir] [--sim|--device] [--debug]
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
  sim) triple=aarch64-apple-ios-sim; platform=iPhoneSimulator ;;
  device) triple=aarch64-apple-ios; platform=iPhoneOS ;;
esac
rustup target list --installed | grep -qx "$triple" ||
  fail "the Rust target is missing: rustup target add $triple"

flag=
[[ $profile == release ]] && flag=--release

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT

# The game's data, packaged by the host's runtime: the target's cannot run here.
cargo run $flag -p bsengine-runtime -- --package "$project" --mode ios --out "$stage/pak"
prop() { sed -n "s/^$1=//p" "$stage/pak/ios.properties"; }
bundle_id=$(prop bundle_id)
name_xml=$(prop name_xml)
[[ -n $bundle_id ]] || fail "--mode ios wrote no bundle id"

cargo build $flag -p bsengine-runtime --target "$triple"
exe=target/$triple/$profile/bsengine-runtime
min_ios=$(sed -n 's/^IPHONEOS_DEPLOYMENT_TARGET *= *"\(.*\)"/\1/p' .cargo/config.toml)
[[ -n $min_ios ]] || fail ".cargo/config.toml sets no IPHONEOS_DEPLOYMENT_TARGET"

# The bundle's directory name is the project's folder name: a file name, which
# the display name (with spaces, `&`, ...) need not be.
app_name=$(basename "$(cd "$project" && pwd)")
app=$out/$app_name.app
rm -rf "$app"
mkdir -p "$app"
cp "$exe" "$app/bsengine-runtime"
# MUTATION: no game.pak in the bundle

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
  <key>CFBundleShortVersionString</key><string>1.0</string>
  <key>CFBundleVersion</key><string>1</string>
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

echo "ios build: $app ($bundle_id, $triple, $profile, iOS $min_ios+)"
if [[ $dest == sim ]]; then
  echo "  xcrun simctl install booted \"$app\" && xcrun simctl launch --console-pty booted $bundle_id"
else
  echo "  unsigned: sign it for a device in Xcode (an Xcode project export is coming)"
fi
