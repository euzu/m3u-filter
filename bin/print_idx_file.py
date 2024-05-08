#!/usr/bin/env python3
"""
This script reads the index files for xtream file indexes
Each index entry consist of 8 bytes where the first 4 bytes and the last 4 bytes are unsigned 32-bit integers (u32).
@see IndexRecord

The IndexRecord can be use for different purposes, f.e.
   - id -> id mapping
   - id -> index
   - index -> size (in this case the offset for the id is calculated through the "cluster_id_start->cluster mapping" offset shift)
"""

import sys
import struct

def print_u32_values(file_path):
    try:
        with open(file_path, "rb") as file:
            while True:
                chunk = file.read(8)
                if not chunk:
                    break

                if len(chunk) == 8:
                    first_u32, last_u32 = struct.unpack("<II", chunk)
                    print(f"{first_u32} : {last_u32}")
                else:
                    print("Incomplete chunk:", chunk)
    except FileNotFoundError:
            print(f"Error: File '{file_path}' not found")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        script_name = os.path.basename(sys.argv[0])
        print("Usage: {script_name} <file_name>")
        sys.exit(1)

    file_path = sys.argv[1]
    print_u32_values(file_path)

