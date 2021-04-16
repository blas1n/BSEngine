@echo off

pushd %~dp0
cmake -S .. -B ../Binaries/ -DCMAKE_TOOLCHAIN_FILE=../ThirdParty/vcpkg/scripts/buildsystems/vcpkg.cmake
popd