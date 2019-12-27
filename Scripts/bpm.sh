#!/bin/sh

set -e

case "$1" in
	"init")
		python3 -c "import bpm;bpm.init()";;
	"register")
		python3 -c "import bpm;bpm.register('$2', '$3')";;
	"unregister")
		python3 -c "import bpm;bpm.unregister('$2')";;
	"install")
		python3 -c "import bpm;bpm.install('$2')";;
	"uninstall")
		python3 -c "import bpm;bpm.uninstall('$2')";;
	"search")
		python3 -c "import bpm;bpm.print_search('$2')";;
	*)
		echo "Unknown command."
		exit 1;;
esac