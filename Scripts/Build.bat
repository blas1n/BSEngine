@echo off

cd ..

if not exist Binaries (
  mkdir Binaries
)

cd Binaries

if "%1%" == "Debug" (
	set BUILD_TYPE=Debug
) else if "%1%" == "Develop" (
	set BUILD_TYPE=RelWithDebInfo
) else if "%1%" == "Shipping" (
	set BUILD_TYPE=MinSizeRel
) else if "%1" == "" (
	set BUILD_TYPE=RelWithDebInfo
) else (
	echo "Unknown build type."
	cd ../Scripts
	exit /b 1
)

cmake .. -DCMAKE_BUILD_TYPE=%BUILD_TYPE%
cmake --build .
cd ../Scripts