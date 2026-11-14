#!/usr/bin/env bash
# Builds a project for Android: an APK holding
#
#   lib/<abi>/libbsengine_runtime.so   the engine: the runtime as a shared library
#   assets/game.pak                    the game's data, with its manifest inside
#   AndroidManifest.xml                android.app.NativeActivity, which loads the
#                                      library -- no Java of ours
#
# signed with the debug key, ready for `adb install`. Like Godot's and Unity's
# Android exports, the engine is native code and the activity is the
# platform's; unlike them there is no Gradle project -- aapt2, zipalign and
# apksigner from the SDK's build-tools make the APK directly.
#
# usage: scripts/build_android.sh <project-dir> [out-dir] [--debug] [--abi arm64-v8a|x86_64]
#   out-dir defaults to <project-dir>/dist-android; --abi to arm64-v8a (phones;
#   x86_64 is the emulator's).
#
# Needs: the Rust target (`rustup target add aarch64-linux-android` or
# `x86_64-linux-android`), the NDK (ANDROID_NDK_HOME, or the newest under
# $ANDROID_HOME/ndk), the SDK's build-tools and a platform (ANDROID_HOME), a
# JDK's keytool for the debug key, and libclang (LIBCLANG_PATH) -- QuickJS's
# bindings are generated for the target at build time (bsengine-scripting).
#
# Run:  adb install -r <out>/<name>.apk && adb shell am start -n <package>/android.app.NativeActivity
# Log:  adb logcat -s bsengine
set -euo pipefail

profile=release
abi=arm64-v8a
args=()
while [[ $# -gt 0 ]]; do
  case $1 in
    --debug) profile=debug ;;
    --abi) abi=$2; shift ;;
    *) args+=("$1") ;;
  esac
  shift
done
project=${args[0]:?usage: scripts/build_android.sh <project-dir> [out-dir] [--debug] [--abi arm64-v8a|x86_64]}
out=${args[1]:-$project/dist-android}

case $abi in
  arm64-v8a) triple=aarch64-linux-android ;;
  x86_64) triple=x86_64-linux-android ;;
  # Older emulator images only; no phone ships it.
  x86) triple=i686-linux-android ;;
  *) echo "unknown --abi $abi (arm64-v8a, x86_64 or x86)" >&2; exit 1 ;;
esac
# The lowest Android the build runs on: AAudio (cpal's backend) is 26+.
api=26

sdk=${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}
[[ -n $sdk && -d $sdk ]] || { echo "set ANDROID_HOME to the Android SDK" >&2; exit 1; }
sdk=${sdk//\\//}
ndk=${ANDROID_NDK_HOME:-${ANDROID_NDK_LATEST_HOME:-}}
if [[ -z $ndk ]]; then
  ndk=$(ls -d "$sdk"/ndk/*/ 2>/dev/null | sort -V | tail -1)
fi
[[ -n $ndk && -d $ndk ]] || { echo "no NDK: set ANDROID_NDK_HOME" >&2; exit 1; }
ndk=${ndk//\\//}; ndk=${ndk%/}
build_tools=$(ls -d "$sdk"/build-tools/*/ | sort -V | tail -1); build_tools=${build_tools%/}
platform=$(ls -d "$sdk"/platforms/android-*/ | sort -V | tail -1); platform=${platform%/}

case $(uname -s) in
  MINGW*|MSYS*|CYGWIN*) host=windows-x86_64; exe=.exe; cmd=.cmd; bat=.bat ;;
  Darwin) host=darwin-x86_64; exe=; cmd=; bat= ;;
  *) host=linux-x86_64; exe=; cmd=; bat= ;;
esac
tc=$ndk/toolchains/llvm/prebuilt/$host
clang_include=$(ls -d "$tc"/lib/clang/*/include | tail -1)

# The cross-compile environment, per target so a host build in the same cargo
# invocation (build scripts, proc macros) keeps the host's compiler. CC is
# NDK clang itself with the API level in --target: cc-rs resolves the
# versioned `.cmd` wrappers to a path without `bin/` on Windows.
t=${triple//-/_}
T=$(echo "$t" | tr a-z A-Z)
export "CARGO_TARGET_${T}_LINKER=$tc/bin/${triple}${api}-clang${cmd}"
export "CC_${t}=$tc/bin/clang${exe}"
export "CXX_${t}=$tc/bin/clang++${exe}"
export "CFLAGS_${t}=--target=${triple}${api}"
export "CXXFLAGS_${t}=--target=${triple}${api}"
export "AR_${t}=$tc/bin/llvm-ar${exe}"
# bindgen (QuickJS's header) parses with the NDK's sysroot and the NDK clang's
# own builtin headers (stdbool.h and the like), which a libclang from
# elsewhere does not find for a foreign target.
export "BINDGEN_EXTRA_CLANG_ARGS_${t}=--sysroot=$tc/sysroot -isystem $clang_include --target=${triple}${api}"

flag=--release
[[ $profile == debug ]] && flag=

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT

# The data first: it is quick, and a project that cannot be packaged should
# fail before the engine's compile, not after it.
cargo run $flag -p bsengine-runtime -- --package "$project" --mode android --out "$stage/pak"
mkdir -p "$stage/apk/assets" "$stage/apk/lib/$abi"
cp "$stage/pak/game.pak" "$stage/apk/assets/game.pak"

# The runtime crate is a library behind its executable; as a `cdylib` it is
# the shared library the activity loads (src/android.rs).
cargo rustc $flag -p bsengine-runtime --lib --crate-type cdylib --target "$triple"
target_dir=$(cargo metadata --format-version 1 --no-deps |
  grep -o '"target_directory":"[^"]*"' | sed 's/.*:"//; s/"$//; s/\\\\/\//g')
"$tc/bin/llvm-strip${exe}" -o "$stage/apk/lib/$abi/libbsengine_runtime.so" \
  "$target_dir/$triple/$profile/libbsengine_runtime.so"

# The package name: the project's name, lower-cased to what Java allows.
name=$(sed -n 's/^name *= *"\(.*\)"/\1/p' "$project/project.toml" | head -1)
name=${name:-$(basename "$project")}
id=$(echo "$name" | tr 'A-Z' 'a-z' | tr -c 'a-z0-9\n' '_' | sed 's/^\([0-9]\)/_\1/')
package=com.bsengine.$id
target_sdk=${platform##*android-}; target_sdk=${target_sdk%%.*}

cat > "$stage/AndroidManifest.xml" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="$package" android:versionCode="1" android:versionName="1.0">
  <uses-sdk android:minSdkVersion="$api" android:targetSdkVersion="$target_sdk" />
  <uses-feature android:name="android.hardware.vulkan.version" android:required="false" />
  <uses-permission android:name="android.permission.INTERNET" />
  <application android:label="$name" android:hasCode="false" android:extractNativeLibs="true">
    <activity android:name="android.app.NativeActivity" android:exported="true"
        android:configChanges="orientation|screenSize|screenLayout|keyboardHidden|keyboard|uiMode"
        android:launchMode="singleTask">
      <meta-data android:name="android.app.lib_name" android:value="bsengine_runtime" />
      <intent-filter>
        <action android:name="android.intent.action.MAIN" />
        <category android:name="android.intent.category.LAUNCHER" />
      </intent-filter>
    </activity>
  </application>
</manifest>
EOF

"$build_tools/aapt2${exe}" link -o "$stage/unaligned.apk" --manifest "$stage/AndroidManifest.xml" \
  -I "$platform/android.jar" -A "$stage/apk/assets"
# aapt2 takes no native libraries; `aapt add` stores the path as given, so it
# runs from the staging root.
(cd "$stage/apk" && "$build_tools/aapt${exe}" add ../unaligned.apk "lib/$abi/libbsengine_runtime.so" > /dev/null)
"$build_tools/zipalign${exe}" -f 4 "$stage/unaligned.apk" "$stage/aligned.apk"

# The debug key every Android tool uses, made on first need.
keystore=${ANDROID_DEBUG_KEYSTORE:-$HOME/.android/debug.keystore}
if [[ ! -f $keystore ]]; then
  mkdir -p "$(dirname "$keystore")"
  keytool -genkeypair -keystore "$keystore" -storepass android -keypass android \
    -alias androiddebugkey -keyalg RSA -keysize 2048 -validity 10000 \
    -dname "CN=Android Debug,O=Android,C=US" > /dev/null
fi
mkdir -p "$out"
apk="$out/$id.apk"
"$build_tools/apksigner${bat}" sign --ks "$keystore" --ks-pass pass:android \
  --key-pass pass:android --out "$apk" "$stage/aligned.apk"
echo "android build: $apk ($package, $abi)"
echo "  adb install -r \"$apk\" && adb shell am start -n $package/android.app.NativeActivity"
