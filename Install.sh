#!/bin/sh

set -e

RESULT=$(pip3 freeze)

while read PACKAGE
do
    if [[ ! "$RESULT" =~ "$PACKAGE" ]]; then
        echo Now install "$PACKAGE"
        pip install "$PACKAGE"
    fi
done < Config/Requirement.txt

cd Scripts
sh bpm.sh install all
sh Build.sh
cd ..
