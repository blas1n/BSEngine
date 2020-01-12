@echo off

cd ..

if exist build (
  rmdir /s /q build
)

mkdir build && cd build

if "%1%" == "Debug" (
	cmake .. -DCMAKE_BUILD_TYPE=Debug
) else if "%1%" == "Normal" (
	cmake .. -DCMAKE_BUILD_TYPE=Normal
) else if "%1%" == "Release" (
	cmake .. -DCMAKE_BUILD_TYPE=MinSizeRel
) else if "%1" == "" (
	cmake .. -DCMAKE_BUILD_TYPE=MinSizeRel
) else (
	echo "Unknown build type."
	exit /b 1
)

cmake --build .
cd ../Scripts