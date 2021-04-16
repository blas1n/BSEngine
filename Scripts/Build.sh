#!/bin/sh

set -e

DIR=$(pwd)
cd $(dirname $0)

cmake --build ../Binaries

cd ${DIR}