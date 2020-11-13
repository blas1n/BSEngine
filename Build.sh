#!/bin/sh

set -e

if [ -d build ]; then
	rm -rf build
fi

mkdir build && cd build

case "$1" in
	"Debug")
		cmake .. -DCMAKE_BUILD_TYPE=Debug;;
	"Develop"|"")
		cmake .. -DCMAKE_BUILD_TYPE=RelWithDebInfo;;
	"Shipping")
		cmake .. -DCMAKE_BUILD_TYPE=MinSizeRel;;
	*)
		echo "Unknown build type."
		exit 1;;
esac

cmake --build .