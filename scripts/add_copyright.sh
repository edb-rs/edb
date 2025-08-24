#!/bin/bash

# EDB - Ethereum Debugger
# Copyright (C) 2024 Zhuo Zhang and Wuqi Zhang
#
# This script adds copyright headers to all Rust source files
# Run with: ./add_copyright.sh

COPYRIGHT_HEADER="// EDB - Ethereum Debugger
// Copyright (C) 2024 Zhuo Zhang and Wuqi Zhang
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.
"

# Function to add copyright to a file
add_copyright() {
    local file="$1"
    
    # Check if file already has copyright
    if head -n 1 "$file" | grep -q "// EDB - Ethereum Debugger"; then
        echo "Skipping $file (already has copyright)"
        return
    fi
    
    # Create temp file with copyright header
    temp_file=$(mktemp)
    echo "$COPYRIGHT_HEADER" > "$temp_file"
    echo "" >> "$temp_file"
    cat "$file" >> "$temp_file"
    
    # Replace original file
    mv "$temp_file" "$file"
    echo "Added copyright to $file"
}

# Find all Rust source files
echo "Adding copyright headers to all Rust source files..."

# Process all .rs files in crates directory
find crates -name "*.rs" -type f | while read -r file; do
    add_copyright "$file"
done

echo "Copyright headers added successfully!"