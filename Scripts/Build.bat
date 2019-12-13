@echo off

 cd ..

if exist build (
  rd /s /q build
)

mkdir build && cd build
cmake .. || goto :error
make run || goto :error
cd ..

:error
cd ..
echo Failed install with error #%errorlevel%
pause
exit /b %errorlevel%