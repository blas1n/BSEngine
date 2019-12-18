@echo off

py -3 -m pip install -r Config/Requirement.txt

cd Scripts
bpm install all
Build.bat
cd ..