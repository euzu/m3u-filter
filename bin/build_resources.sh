#!/usr/bin/env bash

if [ -e ./resources/freeze_frame.ts ]; then
    echo "Resource exists, skipping creation"
    exit;
fi


if which ffmpeg > /dev/null 2>&1; then
    ffmpeg -loop 1 -framerate 1 -i ./resources/freeze_frame.jpg -t 1 -c:v mpeg2video -f mpegts ./resources/freeze_frame.ts
else
    echo "ffmpeg not found";
    exit;
fi
