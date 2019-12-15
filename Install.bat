@echo off

for /f "delims=" %%i in (requirement.txt) do (
    if "x!pip freeze:%%i=!"=="x%pip freeze%"  (
        echo Now install %%i
        py -m pip install %%i
    )
)

cd Scripts
py DownloadExternal.py
pause
Build.bat
cd ..