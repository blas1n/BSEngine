@echo off

for /f "delims=" %%i in (requirement.txt) do (
    if "x!pip freeze:%%i=!"=="x%pip freeze%"  (
        echo Now install %%i
        pip install requests
    )
)

cd Scripts
py DownloadExternal.py
Build.bat
cd ..