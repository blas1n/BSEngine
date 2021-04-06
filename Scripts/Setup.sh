#!/bin/sh

set -e

vcpkg install fmt
vcpkg install spdlog
vcpkg install rapidJSON
vcpkg integrate install