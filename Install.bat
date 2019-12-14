@echo off

for /f "delims=" %%i in (requirement.txt) do (
    echo %%i
    if not "x!pip freeze:%%i=!"=="x%pip freeze%"  (
        echo Now install %%i
        py -m pip install %%i
    )
)

cd Scripts
py DownloadExternal.py
Build.bat
cd ..