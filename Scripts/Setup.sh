#!/bin/sh

set -e

DIR=$(pwd)
cd $(dirname $0)

cmake -S .. -B ../Binaries/ -DCMAKE_TOOLCHAIN_FILE=../ThirdParty/vcpkg/scripts/buildsystems/vcpkg.cmake

cd ${DIR}