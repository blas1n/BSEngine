#!/bin/sh

set -e

cd Scripts
python3 DownloadExternal.py
cd ../Scripts
sh Build.sh
