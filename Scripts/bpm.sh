#!/bin/sh

set -e

if [[ "$1" == "init" ]]; then
    python3 -c "import bpm;bpm.init('$2')"
fi
if [[ "$1" == "register" ]]; then
    python3 -c "import bpm;bpm.register('$2', '$3')"
fi
if [[ "$1" == "unregister" ]]; then
    python3 -c "import bpm;bpm.unregister('$2')"
fi
if [[ "$1" == "install" ]]; then
    python3 -c "import bpm;bpm.install('$2')"
fi
if [[ "$1" == "uninstall" ]]; then
    python3 -c "import bpm;bpm.uninstall('$2')"
fi
if [[ "$1" == "search" ]]; then
    python3 -c "import bpm;bpm.print_search('$2')"
fi
