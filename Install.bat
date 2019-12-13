@echo off

for /f "delims=" %%i in (requirement.txt) do (
    if "x!pip freeze:%%i=!"=="x%pip freeze%"  (
        echo Now install %%i
        pip install requests
    )
)

cd Scripts
py DownloadExternal.py || goto :error
Build.bat || goto :error
cd ..
goto :EOF

:error
cd ..
echo Failed install with error #%errorlevel%
exit /b %errorlevel%