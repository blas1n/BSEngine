@echo off

for /f "delims=" %%i in (requirement.txt) do (
    echo %%i
    if not "x!pip freeze:%%i=!"=="x%pip freeze%"  (
        echo Now install %%i
        pip install requests
    )
)

cd Scripts
py DownloadExternal.py
Build.bat
cd ..