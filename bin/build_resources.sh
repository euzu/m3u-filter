#!/usr/bin/env bash


# Function to print usage instructions
print_usage() {
    echo "Usage: $(basename "$0") [-f] [-h]"
    echo
    echo "Options:"
    echo "  -f    Force resource creation"
    echo "  -h    Display this help message"
    exit 0
}

flag_force=false

# parse options
while getopts "fh" opt; do
  case $opt in
    f) flag_force=true ;;
    h) print_usage ;;
    \?) echo "Unknown option: -$OPTARG" >&2 ;;
  esac
done

if [ "$flag_force" = false ]; then
  if [ -e ./resources/freeze_frame.ts ]; then
      echo "Resource exists, skipping creation"
      exit;
  fi
fi


if which ffmpeg > /dev/null 2>&1; then
    ffmpeg -loop 1 -i ./resources/freeze_frame.jpg -t 10 -r 1 -an -vf "scale=1920:1080" -c:v libx264 -preset veryfast -crf 23 -pix_fmt yuv420p ./resources/freeze_frame.ts
else
    echo "ffmpeg not found";
    exit;
fi
