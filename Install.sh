#!/bin/sh

set -e

python3 -m pip install -r Config/Requirement.txt

cd Scripts
sh bpm.sh install all
sh Build.sh
cd ..
