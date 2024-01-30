#! /usr/bin/env bash

set -xu

REMOTE_HOST=$1
REMOTE_USER=pi
REMOTE_DIR=/home/pi/rump

~/.cargo/bin/cross build --target armv7-unknown-linux-gnueabihf --release

ssh $REMOTE_USER@$REMOTE_HOST sudo systemctl stop rump
ssh $REMOTE_USER@$REMOTE_HOST mkdir -p $REMOTE_DIR

scp -r target/armv7-unknown-linux-gnueabihf/release/rump $REMOTE_USER@$REMOTE_HOST:REMOTE_DIR
scp -r assets/ $REMOTE_USER@$REMOTE_HOST:REMOTE_DIR
scp rump.service $REMOTE_USER@$REMOTE_HOST:REMOTE_DIR

ssh $REMOTE_USER@$REMOTE_HOST sudo mv $REMOTE_DIR/rump.service /etc/systemd/system/

ssh $REMOTE_USER@$REMOTE_HOST sudo systemctl daemon-reload
ssh $REMOTE_USER@$REMOTE_HOST sudo systemctl enable --now rump
ssh $REMOTE_USER@$REMOTE_HOST sudo systemctl status rump
