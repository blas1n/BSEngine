#!/bin/sh

set -e

vcpkg install fmt
vcpkg install spdlog
vcpkg install rapidJSON
vcpkg install utfcpp
vcpkg integrate install