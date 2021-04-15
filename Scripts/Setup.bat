@echo off

cd ../ThirdParty

if exist vcpkg\ (
    cd vcpkg
) else (
    git clone https://github.com/Microsoft/vcpkg.git
    cd vcpkg

    setx VCPKG_ROOT %CD%\vcpkg
    bootstrap-vcpkg
)

set VCPKG_DEFAULT_TRIPLET=x64-windows

vcpkg install SDL2
vcpkg install glew
vcpkg install fmt
vcpkg install spdlog
vcpkg install rapidJSON
vcpkg install utfcpp
vcpkg integrate install

cd ../../Scripts