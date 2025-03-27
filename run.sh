#!/bin/bash

set -ex

rustc -g -C opt-level=1 explore.rs
time ./explore
