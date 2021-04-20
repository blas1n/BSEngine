@echo off

pushd %~dp0

if "%1%" == "" (
	set BUILD_TYPE=Release
) else (
	set BUILD_TYPE=%1
)

cmake --build ../Binaries/%BUILD_TYPE% --config %BUILD_TYPE%

popd