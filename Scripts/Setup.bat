@echo off

if exist vcpkg\ (
    cd vcpkg
    
) else (
    git clone https://github.com/Microsoft/vcpkg.git
    cd vcpkg
    ./bootstrap-vcpkg
)

set VCPKG_DEFAULT_TRIPLET=x64-windows

vcpkg install fmt
vcpkg install spdlog
vcpkg install rapidJSON
vcpkg install utfcpp
vcpkg integrate install