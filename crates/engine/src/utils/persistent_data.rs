// EDB - Ethereum Debugger
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

//! Persistent Data Utilities

use alloy_primitives::U256;
use rpds::{HashTrieMap, VectorSync};
use serde::{Deserialize, Serialize};

/// A persistent EVM stack. Each method returns a NEW Stack, sharing
/// structure with the old one. Keep old handles as snapshots.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Stack {
    v: VectorSync<U256>,
}

impl Stack {
    /// Create a new, empty stack.
    pub fn new() -> Self {
        Self { v: VectorSync::new_sync() }
    }

    /// Return the length of the stack.
    pub fn len(&self) -> usize {
        self.v.len()
    }

    /// Check if the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.v.is_empty()
    }

    /// Push a value onto the stack, returning a new stack.
    pub fn push(&self, value: U256) -> Self {
        Self { v: self.v.push_back(value) }
    }

    /// Pop a value from the stack, returning the value and a new stack.
    /// Returns None if the stack is empty.
    pub fn pop(&self) -> Option<(U256, Self)> {
        let value = self.v.last()?;
        let new_stack = Self { v: self.v.drop_last()? };
        Some((*value, new_stack))
    }

    /// Get a value at the given index from the top of the stack.
    /// Index 0 is the top of the stack, index 1 is the next value down, etc.
    pub fn peek(&self, index: usize) -> Option<&U256> {
        if index >= self.len() {
            return None;
        }
        self.v.get(self.len() - 1 - index)
    }

    /// Convert the stack to a Vec<U256>.
    pub fn to_vec(&self) -> Vec<U256> {
        self.v.iter().copied().collect()
    }
}

/// A persistent EVM memory with configurable page size.
/// Uses HashTrieMap to store pages, enabling efficient structural sharing.
/// Each method returns a NEW Memory, sharing structure with the old one.
/// Keep old handles as snapshots.
#[derive(Clone, Debug)]
pub struct Memory<const PAGE_SIZE: usize = 256> {
    pages: HashTrieMap<usize, [u8; PAGE_SIZE]>,
    /// Actual size of memory in bytes (highest byte written + 1)
    size: usize,
}

impl<const PAGE_SIZE: usize> Memory<PAGE_SIZE> {
    /// Create a new, empty memory.
    pub fn new() -> Self {
        Self { pages: HashTrieMap::new(), size: 0 }
    }

    /// Return the actual size of memory in bytes.
    /// This returns the highest byte address + 1 that has been written to.
    pub fn len(&self) -> usize {
        self.size
    }

    /// Check if the memory is empty.
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Store data at the given offset, returning a new memory.
    /// The data can span multiple pages and will be split accordingly.
    pub fn store(&self, offset: usize, data: &[u8]) -> Self {
        if data.is_empty() {
            return self.clone();
        }

        let mut new_pages = self.pages.clone();
        let mut current_offset = offset;
        let mut data_offset = 0;

        while data_offset < data.len() {
            let page_index = current_offset / PAGE_SIZE;
            let offset_in_page = current_offset % PAGE_SIZE;
            let bytes_to_write = (PAGE_SIZE - offset_in_page).min(data.len() - data_offset);

            // Get existing page or create new one
            let mut page = new_pages.get(&page_index).copied().unwrap_or([0u8; PAGE_SIZE]);

            // Write data to page
            page[offset_in_page..offset_in_page + bytes_to_write]
                .copy_from_slice(&data[data_offset..data_offset + bytes_to_write]);

            // Store updated page
            new_pages = new_pages.insert(page_index, page);

            current_offset += bytes_to_write;
            data_offset += bytes_to_write;
        }

        // Update size to the highest byte written + 1
        let new_size = self.size.max(offset + data.len());

        Self { pages: new_pages, size: new_size }
    }

    /// Convert the memory to a Vec<u8>.
    /// Returns a contiguous byte vector from offset 0 to the actual size.
    pub fn to_vec(&self) -> Vec<u8> {
        if self.is_empty() {
            return Vec::new();
        }

        let total_len = self.len();
        let mut result = vec![0u8; total_len];

        for (&page_index, page) in self.pages.iter() {
            let offset = page_index * PAGE_SIZE;
            let end = (offset + PAGE_SIZE).min(total_len);
            let copy_len = end - offset;
            result[offset..end].copy_from_slice(&page[..copy_len]);
        }

        result
    }
}

impl<const PAGE_SIZE: usize> Default for Memory<PAGE_SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // Stack Tests
    // ============================================================================

    #[test]
    fn test_stack_new() {
        let stack = Stack::new();
        assert_eq!(stack.len(), 0);
        assert!(stack.is_empty());
    }

    #[test]
    fn test_stack_push() {
        let stack = Stack::new();
        let stack = stack.push(U256::from(1));
        assert_eq!(stack.len(), 1);
        assert!(!stack.is_empty());

        let stack = stack.push(U256::from(2));
        assert_eq!(stack.len(), 2);
    }

    #[test]
    fn test_stack_pop() {
        let stack = Stack::new();
        let stack = stack.push(U256::from(1));
        let stack = stack.push(U256::from(2));

        let (value, stack) = stack.pop().unwrap();
        assert_eq!(value, U256::from(2));
        assert_eq!(stack.len(), 1);

        let (value, stack) = stack.pop().unwrap();
        assert_eq!(value, U256::from(1));
        assert_eq!(stack.len(), 0);
        assert!(stack.is_empty());
    }

    #[test]
    fn test_stack_pop_empty() {
        let stack = Stack::new();
        assert!(stack.pop().is_none());
    }

    #[test]
    fn test_stack_persistence() {
        let stack1 = Stack::new();
        let stack2 = stack1.push(U256::from(1));
        let stack3 = stack2.push(U256::from(2));

        // Original stacks should be unchanged
        assert_eq!(stack1.len(), 0);
        assert_eq!(stack2.len(), 1);
        assert_eq!(stack3.len(), 2);

        // Verify values
        let (val, _) = stack2.pop().unwrap();
        assert_eq!(val, U256::from(1));

        let (val, stack4) = stack3.pop().unwrap();
        assert_eq!(val, U256::from(2));
        let (val, _) = stack4.pop().unwrap();
        assert_eq!(val, U256::from(1));
    }

    #[test]
    fn test_stack_to_vec() {
        let stack = Stack::new();
        assert_eq!(stack.to_vec(), Vec::<U256>::new());

        let stack = stack.push(U256::from(1));
        let stack = stack.push(U256::from(2));
        let stack = stack.push(U256::from(3));

        let vec = stack.to_vec();
        assert_eq!(vec, vec![U256::from(1), U256::from(2), U256::from(3)]);
    }

    #[test]
    fn test_stack_to_vec_persistence() {
        let stack1 = Stack::new().push(U256::from(1));
        let stack2 = stack1.push(U256::from(2));

        let vec1 = stack1.to_vec();
        let vec2 = stack2.to_vec();

        assert_eq!(vec1, vec![U256::from(1)]);
        assert_eq!(vec2, vec![U256::from(1), U256::from(2)]);
    }

    // ============================================================================
    // Memory Tests
    // ============================================================================

    #[test]
    fn test_memory_new() {
        let mem: Memory = Memory::new();
        assert_eq!(mem.len(), 0);
        assert!(mem.is_empty());
    }

    #[test]
    fn test_memory_store_single_page() {
        let mem: Memory = Memory::new();
        let data = vec![1u8, 2, 3, 4, 5];
        let mem = mem.store(0, &data);

        assert_eq!(mem.len(), 5); // Actual size is 5 bytes
        assert!(!mem.is_empty());

        let result = mem.to_vec();
        assert_eq!(result, vec![1u8, 2, 3, 4, 5]);
    }

    #[test]
    fn test_memory_store_offset() {
        let mem: Memory = Memory::new();
        let data = vec![0xAA, 0xBB, 0xCC];
        let mem = mem.store(10, &data);

        assert_eq!(mem.len(), 13); // Size is 10 + 3 = 13
        let result = mem.to_vec();
        assert_eq!(&result[0..10], &[0u8; 10]);
        assert_eq!(&result[10..13], &[0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn test_memory_store_multiple_pages() {
        let mem: Memory = Memory::new();
        let data = vec![0xFF; 512]; // 2 pages worth
        let mem = mem.store(0, &data);

        assert_eq!(mem.len(), 512);
        let result = mem.to_vec();
        assert_eq!(result.len(), 512);
        assert_eq!(result, vec![0xFF; 512]);
    }

    #[test]
    fn test_memory_store_spanning_pages() {
        let mem: Memory = Memory::new();
        let data = vec![0xAA; 300]; // Spans 2 pages
        let mem = mem.store(200, &data); // Start at offset 200

        assert_eq!(mem.len(), 500); // Actual size is 200 + 300 = 500
        let result = mem.to_vec();
        assert_eq!(&result[0..200], &[0u8; 200]);
        assert_eq!(&result[200..500], &[0xAA; 300]);
    }

    #[test]
    fn test_memory_persistence() {
        let mem1: Memory = Memory::new();
        let mem2 = mem1.store(0, &[1, 2, 3]);
        let mem3 = mem2.store(10, &[4, 5, 6]);

        // Check that old versions are unchanged
        assert_eq!(mem1.to_vec(), Vec::<u8>::new());
        assert_eq!(mem1.len(), 0);

        // mem2 only has bytes 0-2 written
        assert_eq!(mem2.len(), 3);
        let vec2 = mem2.to_vec();
        assert_eq!(vec2, vec![1, 2, 3]);

        // mem3 has bytes 0-2 and 10-12 written (size = 13)
        assert_eq!(mem3.len(), 13);
        let vec3 = mem3.to_vec();
        assert_eq!(&vec3[0..3], &[1, 2, 3]);
        assert_eq!(&vec3[3..10], &[0, 0, 0, 0, 0, 0, 0]); // Zeros in between
        assert_eq!(&vec3[10..13], &[4, 5, 6]);
    }

    #[test]
    fn test_memory_overwrite() {
        let mem: Memory = Memory::new();
        let mem = mem.store(0, &[1, 2, 3, 4, 5]);
        let mem = mem.store(2, &[0xAA, 0xBB]); // Overwrite bytes 2-3

        let result = mem.to_vec();
        assert_eq!(&result[0..5], &[1, 2, 0xAA, 0xBB, 5]);
    }

    #[test]
    fn test_memory_store_empty() {
        let mem1: Memory = Memory::new();
        let mem2 = mem1.store(0, &[]);

        assert_eq!(mem1.len(), mem2.len());
        assert!(mem2.is_empty());
    }

    #[test]
    fn test_memory_custom_page_size() {
        let mem: Memory<32> = Memory::new(); // 32-byte pages
        let data = vec![0xFF; 64];
        let mem = mem.store(0, &data);

        assert_eq!(mem.len(), 64); // 2 pages of 32 bytes
        let result = mem.to_vec();
        assert_eq!(result, vec![0xFF; 64]);
    }

    #[test]
    fn test_memory_to_vec_empty() {
        let mem: Memory = Memory::new();
        assert_eq!(mem.to_vec(), Vec::<u8>::new());
    }

    #[test]
    fn test_memory_sparse_pages() {
        let mem: Memory = Memory::new();
        let mem = mem.store(0, &[0x11]);
        let mem = mem.store(512, &[0x22]); // Skip page 1, write to page 2

        assert_eq!(mem.len(), 513); // Actual size is 512 + 1 = 513
        let result = mem.to_vec();
        assert_eq!(result[0], 0x11);
        assert_eq!(result[512], 0x22);
        // Middle page should be zeros
        assert_eq!(&result[1..512], &[0u8; 511]);
    }

    #[test]
    fn test_memory_large_offset() {
        let mem: Memory = Memory::new();
        let mem = mem.store(1000, &[0xAA, 0xBB]);

        assert_eq!(mem.len(), 1002); // Size is 1000 + 2 = 1002
        let result = mem.to_vec();
        assert_eq!(result[1000], 0xAA);
        assert_eq!(result[1001], 0xBB);
    }
}
