#!/bin/sh

cd ..

if [ -d build ]; then
	rm -rf build
fi

mkdir build && cd build
cmake .. || exit /b %errorlevel%
make || exit /b %errorlevel%
cd ..
