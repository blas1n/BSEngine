@echo off

if not exist vcpkg (
  git clone https://github.com/Microsoft/vcpkg.git
)

cd vcpkg
./bootstrap-vcpkg.bat
vcpkg integrate install
vcpkg install Eigen3
vcpkg install SDL2