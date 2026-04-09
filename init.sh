#!/usr/bin/env bash
# NBS QR Bot - Dev Environment Init

echo "Building nbs-qr-bot..."
cargo build --release

if [ $? -eq 0 ]; then
    echo "Build successful."
    echo ""
    echo "Start bot:"
    echo '  TELOXIDE_TOKEN="YOUR_TOKEN" ./target/release/nbs-qr-bot bot'
    echo ""
    echo "CLI usage:"
    echo '  ./target/release/nbs-qr-bot gen "paste invoice dump here" -o qr.png'
else
    echo "Build failed!"
    exit 1
fi
