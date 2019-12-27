#!/bin/sh

set -e

cd ..

if [ -d build ]; then
	rm -rf build
fi

mkdir build && cd build

case "$1" in
	"Debug")
		cmake .. -DCMAKE_BUILD_TYPE=Debug;;
	"Normal")
		cmake .. -DCMAKE_BUILD_TYPE=RelWithDebInfo;;
	"Release"|"")
		cmake .. -DCMAKE_BUILD_TYPE=MinSizeRel;;
	*)
		echo "Unknown build type."
		exit 1;;
esac

cmake --build .
cd ../Scripts