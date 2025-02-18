#!/bin/bash

# Set the hostname for the remarkable based on $1 and fall back to `remarkable`
remarkable="10.11.99.1"

if [ "$1" == "local" ]; then
    cargo build --release
else
    # export PKG_CONFIG_SYSROOT_DIR="<toolchain_install_path>/sysroots/cortexa7hf-neon-remarkable-linux-gnueabi"
    # export PKG_CONFIG_PATH="<toolchain_install_path>/sysroots/cortexa7hf-neon-remarkable-linux-gnueabi/usr/lib/pkgconfig"
    # export PKG_CONFIG_ALLOW_CROSS=1
    cross build --release --target=aarch64-unknown-linux-gnu && scp target/aarch64-unknown-linux-gnu/release/ghostwriter root@$remarkable:
fi