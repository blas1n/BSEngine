@echo off

if exist build (
  rd /s /q build
)

mkdir build && cd build
cmake ..
make run
cd ..