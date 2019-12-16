#!/bin/sh

set -e

RESULT=$(pip freeze)

while read PACKAGE
do
    if [[ ! "$RESULT" =~ "$PACKAGE" ]]; then
        echo Now install "$PACKAGE"
        pip install "$PACKAGE"
    fi
done < Requirement.txt

cd Scripts
bpm install all
cd ../Scripts
sh Build.sh
