#!/bin/bash
set -e

cargo ws publish --no-git-commit "$@"
