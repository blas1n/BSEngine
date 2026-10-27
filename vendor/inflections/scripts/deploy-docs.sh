#!/bin/bash

rev=$(git rev-parse --short HEAD)

cargo doc

cd target/doc

git init
git remote add origin https://github.com/calebmer/inflections.git
git fetch origin gh-pages
git reset origin/gh-pages

git add --all
git commit -m "chore(*): rebuild docs at $rev"
git push -q origin HEAD:gh-pages
