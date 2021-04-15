@echo off

cd ../ThirdParty/vcpkg

bootstrap-vcpkg
vcpkg install SDL2
vcpkg install glew
vcpkg install fmt
vcpkg install spdlog
vcpkg install rapidJSON
vcpkg install utfcpp

cd ../../Scripts