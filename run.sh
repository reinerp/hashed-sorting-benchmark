#!/bin/bash

set -e
set -x

RUSTFLAGS="-C target-cpu=native" cargo run --release
