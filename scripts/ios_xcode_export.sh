# The Xcode project export of scripts/build_ios.sh (`--xcode`), sourced by it
# once the game is packaged: it reads the script's variables (`flag`,
# `profile`, `stage`, `out`, `app_name`, `bundle_id`, `name_xml`, `version`,
# `build`, `team_id`, `min_ios`) and its `fail`.
#
# What Unity's, Unreal's and Godot's iOS exports converge on: a generated
# Xcode project around a prebuilt engine library, the game's data as a bundle
# resource, and signing left to Xcode and the team's keychain. The engine
# here is `bsengine-runtime` built as a static library for devices and for the
# Simulator, joined into one XCFramework so the project links whichever the
# destination needs; `main.m` only calls `bsengine_ios_main()` (src/ios.rs).

proj=$out/$app_name
rm -rf "$proj"
mkdir -p "$proj/App"

# The engine, as a static library per target. `--print native-static-libs`
# is rustc's list of what a C link of the library needs besides it -- the
# system frameworks its crates link (Metal, UIKit, AVFoundation, ...) -- and
# it is printed only when rustc runs, so the library crate is rebuilt each
# time (its dependencies are not).
libs=()
for t in aarch64-apple-ios aarch64-apple-ios-sim; do
  touch crates/bsengine-runtime/src/lib.rs
  cargo rustc $flag -p bsengine-runtime --lib --target "$t" --crate-type staticlib \
    -- --print native-static-libs 2> "$stage/rustc-$t.log" ||
    { cat "$stage/rustc-$t.log" >&2; fail "the static library for $t did not build"; }
  libs+=(-library "target/$t/$profile/libbsengine_runtime.a")
done
native=$(sed -n 's/.*native-static-libs: //p' "$stage/rustc-aarch64-apple-ios.log" | tail -1)
[[ -n $native ]] || fail "rustc printed no native-static-libs for the device library"
xcodebuild -create-xcframework "${libs[@]}" -output "$proj/BSEngine.xcframework" > /dev/null

cp "$stage/pak/game.pak" "$proj/App/game.pak"

cat > "$proj/App/main.m" <<'MAIN'
// The app's entry: the engine runs the game from here, on the main thread,
// as UIKit requires (winit's event loop is UIApplicationMain underneath).
// bsengine_ios_main returns only if the game could not start.
extern int bsengine_ios_main(void);

int main(int argc, char *argv[]) {
    (void)argc;
    (void)argv;
    return bsengine_ios_main();
}
MAIN

# Values Xcode fills in from the build settings below, so changing the
# version or bundle id in Xcode's target editor keeps working. The keys are
# build_ios.sh's own .app's: a launch screen (without one iOS letterboxes the
# app), arm64 only, and every orientation but upside-down on an iPhone.
cat > "$proj/App/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleExecutable</key><string>\$(EXECUTABLE_NAME)</string>
  <key>CFBundleIdentifier</key><string>\$(PRODUCT_BUNDLE_IDENTIFIER)</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>\$(PRODUCT_NAME)</string>
  <key>CFBundleDisplayName</key><string>$name_xml</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>\$(MARKETING_VERSION)</string>
  <key>CFBundleVersion</key><string>\$(CURRENT_PROJECT_VERSION)</string>
  <key>LSRequiresIPhoneOS</key><true/>
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
plutil -lint "$proj/App/Info.plist" > /dev/null || fail "the Info.plist written is not valid"

# A plain app icon, so a build is complete without one of the game's: the App
# Store refuses an app without a 1024-point icon, and one with transparency.
# Opaque (no alpha channel), one colour; replace it in Assets.xcassets.
mkdir -p "$proj/App/Assets.xcassets/AppIcon.appiconset"
cat > "$proj/App/Assets.xcassets/Contents.json" <<'JSON'
{ "info" : { "author" : "xcode", "version" : 1 } }
JSON
cat > "$proj/App/Assets.xcassets/AppIcon.appiconset/Contents.json" <<'JSON'
{
  "images" : [
    { "filename" : "icon-1024.png", "idiom" : "universal", "platform" : "ios", "size" : "1024x1024" }
  ],
  "info" : { "author" : "xcode", "version" : 1 }
}
JSON
python3 - "$proj/App/Assets.xcassets/AppIcon.appiconset/icon-1024.png" <<'PY'
import struct, sys, zlib
size, rgb = 1024, bytes((0x2b, 0x3a, 0x55))
row = b"\x00" + rgb * size
def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
png = (b"\x89PNG\r\n\x1a\n"
       + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 2, 0, 0, 0))
       + chunk(b"IDAT", zlib.compress(row * size, 9))
       + chunk(b"IEND", b""))
open(sys.argv[1], "wb").write(png)
PY

# Linker flags: the frameworks rustc listed, one word each as Xcode wants.
ldflags='"$(inherited)"'
for word in $native; do
  ldflags+=", \"$word\""
done

# The project. Object ids are fixed: the file is generated whole each export,
# and stable ids keep a diff between two exports down to what changed.
mkdir -p "$proj/$app_name.xcodeproj"
target_settings() {
  cat <<SETTINGS
				ASSETCATALOG_COMPILER_APPICON_NAME = AppIcon;
				CODE_SIGN_STYLE = Automatic;
				CURRENT_PROJECT_VERSION = "$build";
				DEVELOPMENT_TEAM = "$team_id";
				EXCLUDED_ARCHS = x86_64;
				INFOPLIST_FILE = App/Info.plist;
				IPHONEOS_DEPLOYMENT_TARGET = $min_ios;
				MARKETING_VERSION = "$version";
				OTHER_LDFLAGS = ($ldflags);
				PRODUCT_BUNDLE_IDENTIFIER = "$bundle_id";
				PRODUCT_NAME = "$app_name";
				SUPPORTED_PLATFORMS = "iphoneos iphonesimulator";
				TARGETED_DEVICE_FAMILY = "1,2";
SETTINGS
}
cat > "$proj/$app_name.xcodeproj/project.pbxproj" <<PBX
// !\$*UTF8*\$!
{
	archiveVersion = 1;
	classes = {
	};
	objectVersion = 56;
	objects = {

/* Begin PBXBuildFile section */
		B5E0000000000000000000A1 /* main.m in Sources */ = {isa = PBXBuildFile; fileRef = B5E0000000000000000000B1 /* main.m */; };
		B5E0000000000000000000A2 /* game.pak in Resources */ = {isa = PBXBuildFile; fileRef = B5E0000000000000000000B2 /* game.pak */; };
		B5E0000000000000000000A3 /* Assets.xcassets in Resources */ = {isa = PBXBuildFile; fileRef = B5E0000000000000000000B3 /* Assets.xcassets */; };
		B5E0000000000000000000A4 /* BSEngine.xcframework in Frameworks */ = {isa = PBXBuildFile; fileRef = B5E0000000000000000000B4 /* BSEngine.xcframework */; };
/* End PBXBuildFile section */

/* Begin PBXFileReference section */
		B5E0000000000000000000B1 /* main.m */ = {isa = PBXFileReference; lastKnownFileType = sourcecode.c.objc; path = main.m; sourceTree = "<group>"; };
		B5E0000000000000000000B2 /* game.pak */ = {isa = PBXFileReference; lastKnownFileType = file; path = game.pak; sourceTree = "<group>"; };
		B5E0000000000000000000B3 /* Assets.xcassets */ = {isa = PBXFileReference; lastKnownFileType = folder.assetcatalog; path = Assets.xcassets; sourceTree = "<group>"; };
		B5E0000000000000000000B4 /* BSEngine.xcframework */ = {isa = PBXFileReference; lastKnownFileType = wrapper.xcframework; path = BSEngine.xcframework; sourceTree = "<group>"; };
		B5E0000000000000000000B5 /* Info.plist */ = {isa = PBXFileReference; lastKnownFileType = text.plist.xml; path = Info.plist; sourceTree = "<group>"; };
		B5E0000000000000000000B6 /* $app_name.app */ = {isa = PBXFileReference; explicitFileType = wrapper.application; includeInIndex = 0; path = "$app_name.app"; sourceTree = BUILT_PRODUCTS_DIR; };
/* End PBXFileReference section */

/* Begin PBXFrameworksBuildPhase section */
		B5E0000000000000000000C1 /* Frameworks */ = {
			isa = PBXFrameworksBuildPhase;
			buildActionMask = 2147483647;
			files = (
				B5E0000000000000000000A4 /* BSEngine.xcframework in Frameworks */,
			);
			runOnlyForDeploymentPostprocessing = 0;
		};
/* End PBXFrameworksBuildPhase section */

/* Begin PBXGroup section */
		B5E0000000000000000000D1 = {
			isa = PBXGroup;
			children = (
				B5E0000000000000000000D2 /* App */,
				B5E0000000000000000000B4 /* BSEngine.xcframework */,
				B5E0000000000000000000D3 /* Products */,
			);
			sourceTree = "<group>";
		};
		B5E0000000000000000000D2 /* App */ = {
			isa = PBXGroup;
			children = (
				B5E0000000000000000000B1 /* main.m */,
				B5E0000000000000000000B2 /* game.pak */,
				B5E0000000000000000000B3 /* Assets.xcassets */,
				B5E0000000000000000000B5 /* Info.plist */,
			);
			path = App;
			sourceTree = "<group>";
		};
		B5E0000000000000000000D3 /* Products */ = {
			isa = PBXGroup;
			children = (
				B5E0000000000000000000B6 /* $app_name.app */,
			);
			name = Products;
			sourceTree = "<group>";
		};
/* End PBXGroup section */

/* Begin PBXNativeTarget section */
		B5E0000000000000000000E1 /* App */ = {
			isa = PBXNativeTarget;
			buildConfigurationList = B5E0000000000000000000F2 /* Build configuration list for PBXNativeTarget "App" */;
			buildPhases = (
				B5E0000000000000000000C2 /* Sources */,
				B5E0000000000000000000C1 /* Frameworks */,
				B5E0000000000000000000C3 /* Resources */,
			);
			buildRules = (
			);
			dependencies = (
			);
			name = App;
			productName = App;
			productReference = B5E0000000000000000000B6 /* $app_name.app */;
			productType = "com.apple.product-type.application";
		};
/* End PBXNativeTarget section */

/* Begin PBXProject section */
		B5E000000000000000000010 /* Project object */ = {
			isa = PBXProject;
			attributes = {
				BuildIndependentTargetsInParallel = 1;
				LastUpgradeCheck = 1500;
				TargetAttributes = {
					B5E0000000000000000000E1 = {
						CreatedOnToolsVersion = 15.0;
					};
				};
			};
			buildConfigurationList = B5E0000000000000000000F1 /* Build configuration list for PBXProject */;
			compatibilityVersion = "Xcode 14.0";
			developmentRegion = en;
			hasScannedForEncodings = 0;
			knownRegions = (
				en,
				Base,
			);
			mainGroup = B5E0000000000000000000D1;
			productRefGroup = B5E0000000000000000000D3 /* Products */;
			projectDirPath = "";
			projectRoot = "";
			targets = (
				B5E0000000000000000000E1 /* App */,
			);
		};
/* End PBXProject section */

/* Begin PBXResourcesBuildPhase section */
		B5E0000000000000000000C3 /* Resources */ = {
			isa = PBXResourcesBuildPhase;
			buildActionMask = 2147483647;
			files = (
				B5E0000000000000000000A2 /* game.pak in Resources */,
				B5E0000000000000000000A3 /* Assets.xcassets in Resources */,
			);
			runOnlyForDeploymentPostprocessing = 0;
		};
/* End PBXResourcesBuildPhase section */

/* Begin PBXSourcesBuildPhase section */
		B5E0000000000000000000C2 /* Sources */ = {
			isa = PBXSourcesBuildPhase;
			buildActionMask = 2147483647;
			files = (
				B5E0000000000000000000A1 /* main.m in Sources */,
			);
			runOnlyForDeploymentPostprocessing = 0;
		};
/* End PBXSourcesBuildPhase section */

/* Begin XCBuildConfiguration section */
		B5E000000000000000000011 /* Debug */ = {
			isa = XCBuildConfiguration;
			buildSettings = {
				ALWAYS_SEARCH_USER_PATHS = NO;
				CLANG_ENABLE_OBJC_ARC = YES;
				DEBUG_INFORMATION_FORMAT = dwarf;
				ENABLE_TESTABILITY = YES;
				GCC_OPTIMIZATION_LEVEL = 0;
				IPHONEOS_DEPLOYMENT_TARGET = $min_ios;
				ONLY_ACTIVE_ARCH = YES;
				SDKROOT = iphoneos;
			};
			name = Debug;
		};
		B5E000000000000000000012 /* Release */ = {
			isa = XCBuildConfiguration;
			buildSettings = {
				ALWAYS_SEARCH_USER_PATHS = NO;
				CLANG_ENABLE_OBJC_ARC = YES;
				DEBUG_INFORMATION_FORMAT = "dwarf-with-dsym";
				IPHONEOS_DEPLOYMENT_TARGET = $min_ios;
				SDKROOT = iphoneos;
				VALIDATE_PRODUCT = YES;
			};
			name = Release;
		};
		B5E000000000000000000013 /* Debug */ = {
			isa = XCBuildConfiguration;
			buildSettings = {
$(target_settings)
			};
			name = Debug;
		};
		B5E000000000000000000014 /* Release */ = {
			isa = XCBuildConfiguration;
			buildSettings = {
$(target_settings)
			};
			name = Release;
		};
/* End XCBuildConfiguration section */

/* Begin XCConfigurationList section */
		B5E0000000000000000000F1 /* Build configuration list for PBXProject */ = {
			isa = XCConfigurationList;
			buildConfigurations = (
				B5E000000000000000000011 /* Debug */,
				B5E000000000000000000012 /* Release */,
			);
			defaultConfigurationIsVisible = 0;
			defaultConfigurationName = Release;
		};
		B5E0000000000000000000F2 /* Build configuration list for PBXNativeTarget "App" */ = {
			isa = XCConfigurationList;
			buildConfigurations = (
				B5E000000000000000000013 /* Debug */,
				B5E000000000000000000014 /* Release */,
			);
			defaultConfigurationIsVisible = 0;
			defaultConfigurationName = Release;
		};
/* End XCConfigurationList section */
	};
	rootObject = B5E000000000000000000010 /* Project object */;
}
PBX
plutil -lint "$proj/$app_name.xcodeproj/project.pbxproj" > /dev/null ||
  fail "the project.pbxproj written does not parse"

echo "ios xcode project: $proj/$app_name.xcodeproj ($bundle_id $version ($build), $profile engine, iOS $min_ios+)"
if [[ -n $team_id ]]; then
  echo "  open it in Xcode and Run; it signs with team $team_id"
else
  echo "  open it in Xcode, pick a Team under Signing & Capabilities, and Run"
fi
