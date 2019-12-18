#!/bin/sh

set -e

curl 'https://bootstrap.pypa.io/get-pip.py' > get-pip.py && sudo python get-pip.py
python3 -m pip install -r Config/Requirement.txt

cd Scripts
sh bpm.sh install all
sh Build.sh
cd ..
