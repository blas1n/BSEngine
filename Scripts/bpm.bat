@echo off

if "%1" == "init" (
    py -3 -c "import bpm;bpm.init()"
    goto :EOF
)
if "%1" == "register" (
    py -3 -c "import bpm;bpm.register('%2', '%3')"
    goto :EOF
)
if "%1" == "unregister" (
    py -3 -c "import bpm;bpm.unregister('%2')"
    goto :EOF
)
if "%1" == "install" (
    py -3 -c "import bpm;bpm.install('%2')"
    goto :EOF
)
if "%1" == "uninstall" (
    py -3 -c "import bpm;bpm.uninstall('%2')"
    goto :EOF
)
if "%1" == "search" (
    py -3 -c "import bpm;bpm.print_search('%2')"
    goto :EOF
)