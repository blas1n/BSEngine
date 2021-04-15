#!/bin/sh

set -e

cd ../ThirdParty/vcpkg

sh bootstrap-vcpkg.sh
vcpkg install SDL2
vcpkg install glew
vcpkg install fmt
vcpkg install spdlog
vcpkg install rapidJSON
vcpkg install utfcpp
vcpkg integrate install

cd ../../Scripts