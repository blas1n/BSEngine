#!/bin/sh

set -e

while read package
    do
        if [[ ! "$(pip freeze)" =~ "$(package)" ]]; then
            echo "Now install $(package)"
            pip install $(package)
        fi
    done < requirement.txt

cd Scripts
python3 DownloadExternal.py
cd ../Scripts
sh Build.sh
