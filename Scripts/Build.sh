#!/bin/sh

set -e

cd ..

if [ ! -d Binaries ]; then
	mkdir Binaries
fi

cd Binaries

case "$1" in
	"Debug")
		BUILD_TYPE=Debug;;
	"Develop"|"")
		BUILD_TYPE=RelWithDebInfo;;
	"Shipping")
		BUILD_TYPE=MinSizeRel;;
	*)
		echo "Unknown build type."
		cd ../Scripts
		exit 1;;
esac

cmake .. -DCMAKE_BUILD_TYPE=$BUILD_TYPE
cmake --build .
cd ../Scripts