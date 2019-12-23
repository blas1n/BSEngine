@echo off

py -3 -m pip install -r Config/Requirement.txt

cd Scripts
bpm init
bpm install all
Build
cd ..