#!/bin/sh

# Source the .env file and export the variables to the current shell environment
if [ -f .env ]; then
  export $(cat .env | grep -v '^#' | xargs)
fi

cargo build
