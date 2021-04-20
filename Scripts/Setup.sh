#!/bin/sh

set -e

DIR=$(pwd)
cd $(dirname "$0")

TOOLCHAIN_FILE="$(pwd)"/../vcpkg/scripts/buildsystems/vcpkg.cmake

if [ $# -ge 1 ] ; then
	BUILD_TYPE=$1
else
	BUILD_TYPE=Release
fi

cd ..
mkdir -p Binaries/$BUILD_TYPE
cd Binaries/$BUILD_TYPE

cmake ../.. -DCMAKE_TOOLCHAIN_FILE=$TOOLCHAIN_FILE -DCMAKE_BUILD_TYPE=$BUILD_TYPE -DVCPKG_FEATURE_FLAGS=manifests,registries

cd "${DIR}"