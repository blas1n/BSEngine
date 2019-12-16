@echo off

for /f "delims=" %%i in (Config/Requirement.txt) do (
    if "x!pip freeze:%%i=!"=="x%pip freeze%"  (
        echo Now install %%i
        py -m pip install %%i
    )
)

cd Scripts
bpm install all
Build.bat
cd ..