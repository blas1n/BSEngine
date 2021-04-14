#!/bin/sh

set -e

if [ -d "vcpkg" ]; then
    cd vcpkg
else
    git clone https://github.com/Microsoft/vcpkg.git
    cd vcpkg
    sh bootstrap-vcpkg.sh

    CMAKE_TOOLCHAIN_FILE=./scripts/buildsystems/vcpkg.cmake

fi

CHECK_OS="`uname -s`"

if [[ "$CHECK_OS" = "Darwin"* ]]; then
    VCPKG_DEFAULT_TRIPLET=x64-osx
elif [[ "$CHECK_OS" = "Linux"* ]]; then
    VCPKG_DEFAULT_TRIPLET=x64-linux
elif [[ "$CHECK_OS" = "MINGW64"* ]]; then
    VCPKG_DEFAULT_TRIPLET=x64-windows
elif [[ "$CHECK_OS" = "CYGWIN"* ]]; then
    VCPKG_DEFAULT_TRIPLET=x64-windows
else
    echo "Unknown OS!"
    exit 1
fi

VCPKG_DEFAULT_TRIPLET=x64-windows

vcpkg install SDL2
vcpkg install glew
vcpkg install fmt
vcpkg install spdlog
vcpkg install rapidJSON
vcpkg install utfcpp
vcpkg integrate install