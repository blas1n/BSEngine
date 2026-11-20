#!/usr/bin/env bash
# Builds a project for Android: an APK holding
#
#   lib/<abi>/libbsengine_runtime.so   the engine: the runtime as a shared library
#   assets/game.pak                    the game's data, with its manifest inside
#   AndroidManifest.xml                android.app.NativeActivity, which loads the
#                                      library -- no Java of ours
#
# signed, ready for `adb install`: with the project's release key when its
# `[android]` table names one (a release build only; the passwords come from
# BSENGINE_ANDROID_KEYSTORE_PASSWORD and BSENGINE_ANDROID_KEY_PASSWORD), else
# with the debug key. The same table sets the application id and the version
# (src/android_settings.rs in bsengine-runtime). Like Godot's and Unity's
# Android exports, the engine is native code and the activity is the
# platform's; unlike them there is no Gradle project -- aapt2, zipalign and
# apksigner from the SDK's build-tools make the APK directly.
#
# `--bundle` makes an Android App Bundle (`<name>.aab`) instead: the same
# contents as a `base` module, the form Google Play has required of new apps
# since August 2021 -- it takes no APK. Unity ("Build App Bundle (Google
# Play)"), Unreal ("Generate bundle (AAB)") and Godot (export format AAB) all
# offer it as an export option beside the APK, and so does this. A bundle is
# made by Google's bundletool (fetched once into target/bundletool at the
# version and SHA-256 pinned below, or BUNDLETOOL_JAR names a copy) and signed
# by the JDK's jarsigner with the same key choice as an APK; Play then signs
# the APKs it serves with its own key (Play App Signing) -- the project's key
# is the upload key.
#
# usage: scripts/build_android.sh <project-dir> [out-dir] [--debug] [--bundle] [--abi <abi>[,<abi>...]]
#   out-dir defaults to <project-dir>/dist-android; --abi to arm64-v8a (phones;
#   x86_64 is the emulator's and Chromebooks'). Several, comma-separated
#   (`--abi arm64-v8a,x86_64`), build the engine once for each and carry every
#   one: an APK then installs on all of them, and a bundle lets Play give each
#   device only its own library (its per-ABI split) -- Unity's "Target
#   Architectures", Unreal's "Support arm64 / x86_64" and Godot's export
#   architectures are the same list.
#
# Needs: the Rust target (`rustup target add aarch64-linux-android` or
# `x86_64-linux-android`), the NDK (ANDROID_NDK_HOME, or the newest under
# $ANDROID_HOME/ndk), the SDK's build-tools and a platform (ANDROID_HOME), a
# JDK (JAVA_HOME, else the one on PATH): keytool for the debug key, and for a
# bundle java and jarsigner -- and libclang (LIBCLANG_PATH): QuickJS's
# bindings are generated for the target at build time (bsengine-scripting).
#
# Run:  adb install -r <out>/<name>.apk && adb shell am start -n <package>/android.app.NativeActivity
#       a bundle: the two bundletool commands the build prints (build-apks, install-apks)
# Log:  adb logcat -s bsengine
set -euo pipefail

profile=release
abi_list=arm64-v8a
format=apk
args=()
while [[ $# -gt 0 ]]; do
  case $1 in
    --debug) profile=debug ;;
    --bundle) format=aab ;;
    --abi) abi_list=$2; shift ;;
    *) args+=("$1") ;;
  esac
  shift
done
project=${args[0]:?usage: scripts/build_android.sh <project-dir> [out-dir] [--debug] [--bundle] [--abi <abi>[,<abi>...]]}
out=${args[1]:-$project/dist-android}

# Every ABI checked, and its Rust target found, before anything is built: a
# typo in the second of two must not cost the first one's compile.
abis=()
triples=()
IFS=, read -ra requested <<< "$abi_list"
for abi in "${requested[@]}"; do
  case $abi in
    arm64-v8a) triple=aarch64-linux-android ;;
    x86_64) triple=x86_64-linux-android ;;
    # Older emulator images only; no phone ships it.
    x86) triple=i686-linux-android ;;
    *) echo "unknown --abi $abi (arm64-v8a, x86_64 or x86)" >&2; exit 1 ;;
  esac
  # `${abis[@]+...}`: bash 3.2 (macOS) calls an empty array unbound under set -u.
  for seen in ${abis[@]+"${abis[@]}"}; do
    [[ $seen != "$abi" ]] || { echo "--abi names $abi twice" >&2; exit 1; }
  done
  abis+=("$abi")
  triples+=("$triple")
done
[[ ${#abis[@]} -gt 0 ]] || { echo "--abi names no ABI" >&2; exit 1; }
abis_shown=$(IFS=,; echo "${abis[*]}")
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
clang_resources=$(ls -d "$tc"/lib/clang/* | tail -1)
# The JDK's tools: JAVA_HOME's when it is set, as Gradle and the SDK's own
# wrappers (apksigner) take it -- the `java` on PATH may be an older one
# (a JDK 8 there beside a JDK 17 in JAVA_HOME is a common Windows setup).
jdk_bin=
if [[ -n ${JAVA_HOME:-} ]]; then jdk_bin=${JAVA_HOME//\\//}/bin/; fi
target_dir=$(cargo metadata --format-version 1 --no-deps |
  grep -o '"target_directory":"[^"]*"' | sed 's/.*:"//; s/"$//; s/\\\\/\//g')

# bundletool, for a bundle: resolved before anything is compiled, so a
# machine that cannot get it fails in seconds. Pinned by version and SHA-256
# (the release's own digest on GitHub) -- a download that is cut short or
# replaced is refused, not run.
if [[ $format == aab ]]; then
  bundletool_version=1.18.3
  bundletool_sha256=a099cfa1543f55593bc2ed16a70a7c67fe54b1747bb7301f37fdfd6d91028e29
  bundletool=${BUNDLETOOL_JAR:-$target_dir/bundletool/bundletool-all-$bundletool_version.jar}
  if [[ ! -f $bundletool ]]; then
    [[ -z ${BUNDLETOOL_JAR:-} ]] || { echo "BUNDLETOOL_JAR=$BUNDLETOOL_JAR does not exist" >&2; exit 1; }
    mkdir -p "$(dirname "$bundletool")"
    echo "fetching bundletool $bundletool_version"
    curl -fsSL -o "$bundletool.part" \
      "https://github.com/google/bundletool/releases/download/$bundletool_version/bundletool-all-$bundletool_version.jar"
    if command -v sha256sum > /dev/null; then got=$(sha256sum "$bundletool.part"); else got=$(shasum -a 256 "$bundletool.part"); fi
    got=${got%% *}
    if [[ $got != "$bundletool_sha256" ]]; then
      rm -f "$bundletool.part"
      echo "bundletool $bundletool_version: SHA-256 $got, expected $bundletool_sha256" >&2
      exit 1
    fi
    mv "$bundletool.part" "$bundletool"
  fi
fi

# The cross-compile environment, per target so a host build in the same cargo
# invocation (build scripts, proc macros) keeps the host's compiler. CC is
# NDK clang itself with the API level in --target: cc-rs resolves the
# versioned `.cmd` wrappers to a path without `bin/` on Windows.
for triple in "${triples[@]}"; do
  t=${triple//-/_}
  T=$(echo "$t" | tr a-z A-Z)
  export "CARGO_TARGET_${T}_LINKER=$tc/bin/${triple}${api}-clang${cmd}"
  export "CC_${t}=$tc/bin/clang${exe}"
  export "CXX_${t}=$tc/bin/clang++${exe}"
  export "CFLAGS_${t}=--target=${triple}${api}"
  export "CXXFLAGS_${t}=--target=${triple}${api}"
  export "AR_${t}=$tc/bin/llvm-ar${exe}"
  # bindgen (QuickJS's header) parses with the NDK's sysroot and the NDK
  # clang's builtin headers (stdbool.h, stdint.h), as its resource directory
  # -- replacing libclang's own, not added to it. LLVM's libclang.dll on
  # Windows resolves its resource directory to a path that does not exist
  # ("stdbool.h not found"); Linux's finds its own, and with the NDK's added
  # beside it as -isystem, the two stdint.h chains crossed and `uint32_t` was
  # undeclared.
  export "BINDGEN_EXTRA_CLANG_ARGS_${t}=--sysroot=$tc/sysroot -resource-dir $clang_resources --target=${triple}${api}"
done

flag=--release
[[ $profile == debug ]] && flag=

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT

# The data first: it is quick, and a project that cannot be packaged should
# fail before the engine's compile, not after it.
cargo run $flag -p bsengine-runtime -- --package "$project" --mode android --out "$stage/pak"
mkdir -p "$stage/apk/assets"
cp "$stage/pak/game.pak" "$stage/apk/assets/game.pak"

# `[android]` as `--package` checked and resolved it (src/android_settings.rs):
# the application id, the version, the app's name, and the release key if
# the project names one.
props=$stage/pak/android.properties
prop() { sed -n "s/^$1=//p" "$props" | head -1; }
package=$(prop package)
version_code=$(prop version_code)
version_name_xml=$(prop version_name_xml)
label_xml=$(prop label_xml)
release_keystore=$(prop keystore)
release_alias=$(prop key_alias)

# Which key signs, decided before the long compile so a missing password
# fails in seconds. A release build of a project that names a keystore is
# signed with it, the passwords from the environment -- never the project,
# never the command line (apksigner reads them with `env:`). Godot takes
# them the same way (GODOT_ANDROID_KEYSTORE_RELEASE_PASSWORD). A `--debug`
# build is debug-signed whatever the project says, as Godot's debug export is.
if [[ $profile == release && -n $release_keystore ]]; then
  if [[ -z ${BSENGINE_ANDROID_KEYSTORE_PASSWORD:-} ]]; then
    echo "the project signs with $release_keystore (key '$release_alias'):" \
      "set BSENGINE_ANDROID_KEYSTORE_PASSWORD, and BSENGINE_ANDROID_KEY_PASSWORD" \
      "if the key's differs. Not falling back to the debug key: a store ties an" \
      "app to the key of its first upload." >&2
    exit 1
  fi
  export BSENGINE_ANDROID_KEY_PASSWORD=${BSENGINE_ANDROID_KEY_PASSWORD:-$BSENGINE_ANDROID_KEYSTORE_PASSWORD}
  sign_args=(--ks "$release_keystore" --ks-key-alias "$release_alias"
    --ks-pass env:BSENGINE_ANDROID_KEYSTORE_PASSWORD
    --key-pass env:BSENGINE_ANDROID_KEY_PASSWORD)
  # jarsigner, for a bundle, reads them the same way (`:env`).
  jar_sign_args=(-keystore "$release_keystore"
    -storepass:env BSENGINE_ANDROID_KEYSTORE_PASSWORD
    -keypass:env BSENGINE_ANDROID_KEY_PASSWORD)
  key_alias=$release_alias
  signed_with="the release key '$release_alias' ($release_keystore)"
else
  # The debug key every Android tool uses, made on first need.
  keystore=${ANDROID_DEBUG_KEYSTORE:-$HOME/.android/debug.keystore}
  if [[ ! -f $keystore ]]; then
    mkdir -p "$(dirname "$keystore")"
    "${jdk_bin}keytool${exe}" -genkeypair -keystore "$keystore" -storepass android -keypass android \
      -alias androiddebugkey -keyalg RSA -keysize 2048 -validity 10000 \
      -dname "CN=Android Debug,O=Android,C=US" > /dev/null
  fi
  sign_args=(--ks "$keystore" --ks-pass pass:android --key-pass pass:android)
  jar_sign_args=(-keystore "$keystore" -storepass android -keypass android)
  key_alias=androiddebugkey
  signed_with="the debug key"
  if [[ $profile == release ]]; then
    signed_with="the debug key -- installable, not publishable (name a keystore in [android])"
  fi
fi

# The runtime crate is a library behind its executable; as a `cdylib` it is
# the shared library the activity loads (src/android.rs). One compile per ABI,
# each into its own `target/<triple>/`, so a second ABI costs a second engine
# compile and nothing else.
for i in "${!abis[@]}"; do
  abi=${abis[$i]}; triple=${triples[$i]}
  cargo rustc $flag -p bsengine-runtime --lib --crate-type cdylib --target "$triple"
  mkdir -p "$stage/apk/lib/$abi"
  "$tc/bin/llvm-strip${exe}" -o "$stage/apk/lib/$abi/libbsengine_runtime.so" \
    "$target_dir/$triple/$profile/libbsengine_runtime.so"
done

# The APK's file name: the application id's last part -- for the default id,
# the project's name lower-cased, the name it always had.
id=${package##*.}
target_sdk=${platform##*android-}; target_sdk=${target_sdk%%.*}

cat > "$stage/AndroidManifest.xml" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="$package" android:versionCode="$version_code" android:versionName="$version_name_xml">
  <uses-sdk android:minSdkVersion="$api" android:targetSdkVersion="$target_sdk" />
  <uses-feature android:name="android.hardware.vulkan.version" android:required="false" />
  <uses-permission android:name="android.permission.INTERNET" />
  <application android:label="$label_xml" android:hasCode="false" android:extractNativeLibs="true">
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

mkdir -p "$out"

if [[ $format == aab ]]; then
  # A bundle's module is a zip in its own layout -- manifest/AndroidManifest.xml,
  # resources.pb, assets/, lib/<abi>/ -- with the manifest and the resource
  # table as protocol buffers, not the binary XML an APK carries: aapt2 writes
  # that form with --proto-format, and the files are then moved to where
  # bundletool looks. bundletool builds the bundle; the APKs a device gets are
  # cut from it later (by Play, or by `build-apks` here), which is when native
  # libraries and assets are aligned and compressed -- so no zipalign now.
  "$build_tools/aapt2${exe}" link --proto-format -o "$stage/proto.apk" \
    --manifest "$stage/AndroidManifest.xml" -I "$platform/android.jar" -A "$stage/apk/assets"
  mkdir -p "$stage/base/manifest"
  # The JDK's jar, not zip/unzip: a JDK is already needed here (bundletool,
  # jarsigner), and zip is not on every machine that has one. Relative
  # paths from inside the staging root, so nothing depends on how a Windows
  # shell turns a path into an argument.
  (cd "$stage/base" && "${jdk_bin}jar${exe}" xf ../proto.apk)
  mv "$stage/base/AndroidManifest.xml" "$stage/base/manifest/AndroidManifest.xml"
  cp -r "$stage/apk/lib" "$stage/base/lib"
  (cd "$stage/base" && "${jdk_bin}jar${exe}" cfM ../base.zip .)
  (cd "$stage" && "${jdk_bin}java${exe}" -jar "$bundletool" build-bundle --modules=base.zip --output=unsigned.aab)

  aab="$out/$id.aab"
  # A bundle is signed as a JAR (v1 signing): that is what Play reads for the
  # upload key; apksigner's v2/v3 schemes are for the APKs cut from it.
  # jarsigner reports a wrong password on stdout, not stderr: kept, and shown
  # only on failure (a success prints a timestamp warning worth nothing).
  if ! signed=$("${jdk_bin}jarsigner${exe}" -sigalg SHA256withRSA -digestalg SHA-256 "${jar_sign_args[@]}" \
    -signedjar "$aab" "$stage/unsigned.aab" "$key_alias" 2>&1); then
    echo "$signed" >&2
    rm -f "$aab"
    exit 1
  fi
  # Checked, and the certificate named, as for an APK below. `-verify` exits 0
  # for an unsigned jar too, so its "jar verified." is what is looked for.
  "${jdk_bin}jarsigner${exe}" -verify "$aab" | grep -q "^jar verified" ||
    { echo "$aab: jarsigner -verify did not verify it" >&2; exit 1; }
  # keytool prints AB:CD:..; written as apksigner prints it, so an APK's and a
  # bundle's certificates compare at a glance.
  "${jdk_bin}keytool${exe}" -printcert -jarfile "$aab" | grep -m1 "SHA256:" |
    tr -d '\r' | sed "s/.*SHA256: //; s/://g" | tr A-F a-f | sed "s/^/  certificate SHA-256: /"
  echo "android build: $aab ($package, version $version_code, $abis_shown), signed with $signed_with"
  echo "  to install it here, as Play would cut it for the device:"
  echo "  \"${jdk_bin}java${exe}\" -jar \"$bundletool\" build-apks --bundle=\"$aab\" --output=$id.apks --connected-device"
  echo "  \"${jdk_bin}java${exe}\" -jar \"$bundletool\" install-apks --apks=$id.apks"
  exit 0
fi

"$build_tools/aapt2${exe}" link -o "$stage/unaligned.apk" --manifest "$stage/AndroidManifest.xml" \
  -I "$platform/android.jar" -A "$stage/apk/assets"
# aapt2 takes no native libraries; `aapt add` stores the path as given, so it
# runs from the staging root.
# Every ABI's library, in one call.
libs=()
for abi in "${abis[@]}"; do libs+=("lib/$abi/libbsengine_runtime.so"); done
(cd "$stage/apk" && "$build_tools/aapt${exe}" add ../unaligned.apk "${libs[@]}" > /dev/null)
"$build_tools/zipalign${exe}" -f 4 "$stage/unaligned.apk" "$stage/aligned.apk"

apk="$out/$id.apk"
"$build_tools/apksigner${bat}" sign "${sign_args[@]}" --out "$apk" "$stage/aligned.apk"
# Checked, and the certificate named: which key signed a build is otherwise
# invisible until a store or a device refuses an update.
"$build_tools/apksigner${bat}" verify --print-certs "$apk" | grep -m1 "SHA-256 digest" |
  sed "s/.*SHA-256 digest: /  certificate SHA-256: /"
echo "android build: $apk ($package, version $version_code, $abis_shown), signed with $signed_with"
echo "  adb install -r \"$apk\" && adb shell am start -n $package/android.app.NativeActivity"
