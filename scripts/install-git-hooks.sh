#!/bin/sh

if [ ! $(which git) ]; then
    echo "'git' is not installed. Exiting."
    exit 1
fi

if [ $(git rev-parse --is-inside-work-tree) ]; then
    GIT_ROOT=$(git rev-parse --show-toplevel)
else
    echo "You are not inside a git repository. Exiting."
    exit 1
fi

HOOKS="$GIT_ROOT/.git/hooks"

## Pre-commit
if ! cargo make --version 2&>/dev/null; then
    echo "============================="
    echo "=== Installing cargo-make ==="
    echo "=============================\n"
    if ! cargo install cargo-make; then
        echo "\nCould not install cargo-make"
        exit 1
    fi
fi

echo "============================"
echo "=== Copy pre-commit hook ==="
echo "============================\n"
cp $GIT_ROOT/pre-commit.sh "$HOOKS/pre-commit"
chmod u+x "$HOOKS/pre-commit"

## Commit-msg
echo "========================================"
echo "=== Installing sailr commit-msg hook ==="
echo "========================================\n"

# sailr requires `jq` to be installed
SCRIPT_FILE="https://raw.githubusercontent.com/apollodao/sailr/master/sailr.sh"

if curl $SCRIPT_FILE -o "$HOOKS/commit-msg"; then
    chmod u+x "$HOOKS/commit-msg"
    echo "\nInstalled Sailr as commit-msg hook in $HOOKS."
    echo "For usage see https://github.com/apollodao/sailr#usage\n"
else
    echo "\nCould not install Sailr."
    exit 1
fi

# Reinitialize git repo
git init

## Finish
echo "\n---------\n| Done! |\n---------"
