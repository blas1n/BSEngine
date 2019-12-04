#!/usr/bin/env

if [ -d build ]; then
	rm build
fi

mkdir build && cd build
cmake ..
make
cd ..