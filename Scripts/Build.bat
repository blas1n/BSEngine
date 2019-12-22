@echo off

 cd ..

if exist build (
  rd /s /q build
)

mkdir build && cd build
cmake .. || goto :error
make || goto :error
cd ../Scripts

:error
cd ../Scripts
echo Failed build with error #%errorlevel%
exit /b %errorlevel%