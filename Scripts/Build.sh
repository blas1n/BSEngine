#!/bin/sh

set -e
cd ..

if [ -d build ]; then
	rm -rf build
fi

mkdir build && cd build
cmake ..
make
cd ..
