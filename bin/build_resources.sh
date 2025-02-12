#!/usr/bin/env bash

if [ -e ./resources/freeze_frame.ts ]; then
    echo "Resource exists, skipping creation"
    exit;
fi


if which ffmpeg > /dev/null 2>&1; then
    ffmpeg -loop 1 -i ./resources/freeze_frame.jpg -t 10 -r 1 -an -c:v libx264 -preset veryfast -crf 23 -pix_fmt yuv420p ./resources/freeze_frame.ts
else
    echo "ffmpeg not found";
    exit;
fi
