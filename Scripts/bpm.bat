@echo off

if "%1" == "init" (
    python3 -c "import bpm;bpm.init()"
    goto :EOF
)
if "%1" == "register" (
    python3 -c "import bpm;bpm.register('%2', '%3')"
    goto :EOF
)
if "%1" == "unregister" (
    python3 -c "import bpm;bpm.unregister('%2')"
    goto :EOF
)
if "%1" == "install" (
    python3 -c "import bpm;bpm.install('%2')"
    goto :EOF
)
if "%1" == "uninstall" (
    python3 -c "import bpm;bpm.uninstall('%2')"
    goto :EOF
)
if "%1" == "search" (
    python3 -c "import bpm;bpm.print_search('%2')"
    goto :EOF
)