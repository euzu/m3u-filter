#!/bin/bash
cargo +nightly udeps
cargo +nightly udeps --all-targets
