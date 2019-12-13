#!/bin/sh

set -e

RESULT=$(pip freeze)

while read PACKAGE
do
    if [[ ! "$RESULT" =~ "$PACKAGE" ]]; then
        echo Now install "$PACKAGE"
        pip install "$PACKAGE"
    fi
done < requirement.txt

cd Scripts
python3 DownloadExternal.py
cd ../Scripts
sh Build.sh
