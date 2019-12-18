#!/bin/sh

set -e

sudo python3 get-pip.py
pip3 install -r Config/Requirement.txt

cd Scripts
sh bpm.sh install all
sh Build.sh
cd ..
