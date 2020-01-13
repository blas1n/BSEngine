#!/bin/sh

set -e

git clone https://github.com/Microsoft/vcpkg.git
cd vcpkg
sh bootstrap-vcpkg.sh
./vcpkg integrate install
./vcpkg install Eigen3
./vcpkg install SDL2