@echo off

cd ..

if exist build (
  rmdir /s /q build
)

mkdir build && cd build

if "%1%" == "Debug" (
	cmake .. -DCMAKE_BUILD_TYPE=Debug
) else if "%1%" == "Develop" (
	cmake .. -DCMAKE_BUILD_TYPE=RelWithDebInfo
) else if "%1%" == "Shipping" (
	cmake .. -DCMAKE_BUILD_TYPE=MinSizeRel
) else if "%1" == "" (
	cmake .. -DCMAKE_BUILD_TYPE=RelWithDebInfo
) else (
	echo "Unknown build type."
	exit /b 1
)

cmake --build .