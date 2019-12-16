#!/bin/sh

if [ "$1" == "init" ]; then
    python -c "import bpm;bpm.clear('$2')"
    exit 0
fi
if [ "$1" == "register" ]; then
    python -c "import bpm;bpm.register('$2', '$3')"94
    exit 0
fi
if [ "$1" == "unregister" ]; then
    python -c "import bpm;bpm.unregister('$2')"
    exit 0
fi
if [ "$1" == "install" ]; then
    python -c "import bpm;bpm.install('$2')"
    exit 0
fi
if [ "$1" == "uninstall" ]; then
    python -c "import bpm;bpm.uninstall('$2')"
    exit 0
fi
if [ "$1" == "search" ]; then
    val = $("import bpm;bpm.search('$2')")
    if [ val ]; then
        echo "$2 already installed."
    else
        echo "$2 is not installed."
    fi
    exit 0
fi