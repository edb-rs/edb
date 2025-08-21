//! Unified snapshot management for time travel debugging
//!
//! This module provides a unified interface for managing both opcode-level and hook-based
//! snapshots. It merges the two different snapshot types into a coherent structure that
//! maintains execution order and frame relationships for effective debugging.
//!
//! The main structure `Snapshots` combines:
//! - Opcode snapshots: Fine-grained instruction-level state captures
//! - Hook snapshots: Strategic breakpoint-based state captures
//!
//! The merging process prioritizes hook snapshots when available, falling back to
//! opcode snapshots for frames without hooks. This provides a comprehensive view
//! of execution state across the entire transaction.

use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

use revm::{database::CacheDB, Database, DatabaseCommit, DatabaseRef};

use edb_common::types::ExecutionFrameId;
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::{HookSnapshot, HookSnapshots, OpcodeSnapshot, OpcodeSnapshots, USID};

/// Unique identifier for different types of snapshots
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SnapshotId {
    /// Opcode snapshot identified by program counter
    Opcode(usize),
    /// Hook snapshot identified by user-defined snapshot ID
    Hook(USID),
}

/// Union type representing either an opcode or hook snapshot
///
/// This enum allows us to treat both types of snapshots uniformly while preserving
/// their specific characteristics. Hook snapshots are generally preferred as they
/// represent strategic breakpoints, while opcode snapshots provide fine-grained
/// instruction-level details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Snapshot<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    /// Fine-grained opcode execution snapshot
    Opcode(OpcodeSnapshot<DB>),
    /// Strategic hook-based snapshot at instrumentation points
    Hook(HookSnapshot<DB>),
}

impl<DB> Snapshot<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    /// Get the unique identifier for this snapshot
    pub fn id(&self) -> SnapshotId {
        match self {
            Snapshot::Opcode(snapshot) => SnapshotId::Opcode(snapshot.pc),
            Snapshot::Hook(snapshot) => SnapshotId::Hook(snapshot.usid),
        }
    }

    /// Get the contract address associated with this snapshot
    pub fn address(&self) -> alloy_primitives::Address {
        match self {
            Snapshot::Opcode(snapshot) => snapshot.address,
            Snapshot::Hook(snapshot) => snapshot.address,
        }
    }

    /// Check if this is a hook snapshot
    pub fn is_hook(&self) -> bool {
        matches!(self, Snapshot::Hook(_))
    }

    /// Check if this is an opcode snapshot  
    pub fn is_opcode(&self) -> bool {
        matches!(self, Snapshot::Opcode(_))
    }
}

/// Unified collection of execution snapshots organized by execution frame
///
/// This structure maintains a chronological view of all captured snapshots,
/// whether they are fine-grained opcode snapshots or strategic hook snapshots.
/// Multiple snapshots can exist within a single execution frame, representing
/// different points of interest during that frame's execution.
///
/// The collection prioritizes hook snapshots when available, as they represent
/// intentional debugging breakpoints. Opcode snapshots are included for frames
/// that lack hook coverage, ensuring comprehensive state tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshots<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    /// Vector of (frame_id, snapshot) pairs in execution order
    inner: Vec<(ExecutionFrameId, Snapshot<DB>)>,
}

impl<DB> Deref for Snapshots<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    type Target = [(ExecutionFrameId, Snapshot<DB>)];

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<DB> DerefMut for Snapshots<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

// IntoIterator for owned Trace (moves out its contents)
impl<DB> IntoIterator for Snapshots<DB> 
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    type Item = (ExecutionFrameId, Snapshot<DB>);
    type IntoIter = std::vec::IntoIter<(ExecutionFrameId, Snapshot<DB>)>;
    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

// IntoIterator for &Trace (shared iteration)
impl<'a, DB> IntoIterator for &'a Snapshots<DB> 
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    type Item = &'a (ExecutionFrameId, Snapshot<DB>);
    type IntoIter = std::slice::Iter<'a, (ExecutionFrameId, Snapshot<DB>)>;
    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter()
    }
}

// IntoIterator for &mut Trace (mutable iteration)
impl<'a, DB> IntoIterator for &'a mut Snapshots<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    type Item = &'a mut (ExecutionFrameId, Snapshot<DB>);
    type IntoIter = std::slice::IterMut<'a, (ExecutionFrameId, Snapshot<DB>)>;
    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter_mut()
    }
}

impl<DB> Snapshots<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    /// Create a new empty snapshots collection
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }

    /// Merge hook snapshots and opcode snapshots into a unified collection
    ///
    /// This method combines the two snapshot types using the following strategy:
    /// 1. For frames with hook snapshots, use the hook snapshot (strategic breakpoints)
    /// 2. For frames without hook snapshots, include all opcode snapshots (fine-grained detail)
    /// 3. Maintain execution order for proper debugging flow
    ///
    /// Hook snapshots are preferred because they represent intentional instrumentation
    /// points, while opcode snapshots provide comprehensive coverage for uninstrumented code.
    pub fn merge(
        mut opcode_snapshots: OpcodeSnapshots<DB>,
        hook_snapshots: HookSnapshots<DB>,
    ) -> Self {
        let mut inner = Vec::new();

        // Process hook snapshots first (they take priority)
        for (frame_id, snapshot_opt) in hook_snapshots {
            match snapshot_opt {
                Some(snapshot) => {
                    // We have a valid hook snapshot - this takes priority
                    inner.push((frame_id, Snapshot::Hook(snapshot)));
                }
                None => {
                    // No hook snapshot for this frame - try to use opcode snapshots
                    if let Some(opcode_frame_snapshots) = opcode_snapshots.remove(&frame_id) {
                        // Add all opcode snapshots for this frame
                        inner.extend(
                            opcode_frame_snapshots.into_iter().map(|opcode_snapshot| {
                                (frame_id, Snapshot::Opcode(opcode_snapshot))
                            }),
                        );
                    }
                }
            }
        }

        // Include any remaining opcode snapshots for frames not covered by hooks
        if opcode_snapshots.values().any(|snapshots| !snapshots.is_empty()) {
            error!(
                "There are still opcode snapshots left after merging: {:?}",
                opcode_snapshots.keys().collect::<Vec<_>>()
            );
        }

        Self { inner }
    }

    /// Get all snapshots for a specific execution frame
    pub fn get_frame_snapshots(&self, frame_id: ExecutionFrameId) -> Vec<&Snapshot<DB>> {
        self.inner
            .iter()
            .filter_map(|(id, snapshot)| if *id == frame_id { Some(snapshot) } else { None })
            .collect()
    }

    /// Get all unique execution frame IDs that have snapshots
    pub fn get_frame_ids(&self) -> Vec<ExecutionFrameId> {
        let mut frame_ids: Vec<_> = self.inner.iter().map(|(id, _)| *id).collect();
        frame_ids.dedup();
        frame_ids
    }

    /// Get the total number of snapshots across all frames
    pub fn total_snapshot_count(&self) -> usize {
        self.inner.len()
    }

    /// Get the number of unique frames that have snapshots
    pub fn frame_count(&self) -> usize {
        self.get_frame_ids().len()
    }

    /// Get statistics about hook vs opcode snapshot distribution
    pub fn get_snapshot_stats(&self) -> SnapshotStats {
        let mut hook_count = 0;
        let mut opcode_count = 0;
        let mut frames_with_hooks = std::collections::HashSet::new();
        let mut frames_with_opcodes = std::collections::HashSet::new();

        for (frame_id, snapshot) in &self.inner {
            match snapshot {
                Snapshot::Hook(_) => {
                    hook_count += 1;
                    frames_with_hooks.insert(*frame_id);
                }
                Snapshot::Opcode(_) => {
                    opcode_count += 1;
                    frames_with_opcodes.insert(*frame_id);
                }
            }
        }

        SnapshotStats {
            total_snapshots: self.inner.len(),
            hook_snapshots: hook_count,
            opcode_snapshots: opcode_count,
            total_frames: self.frame_count(),
            frames_with_hooks: frames_with_hooks.len(),
            frames_with_opcodes: frames_with_opcodes.len(),
        }
    }
}

/// Statistics about snapshot distribution
#[derive(Debug, Clone)]
pub struct SnapshotStats {
    /// Total number of snapshots
    pub total_snapshots: usize,
    /// Number of hook-based snapshots
    pub hook_snapshots: usize,
    /// Number of opcode-level snapshots
    pub opcode_snapshots: usize,
    /// Total number of unique execution frames
    pub total_frames: usize,
    /// Number of frames that have hook snapshots
    pub frames_with_hooks: usize,
    /// Number of frames that have opcode snapshots
    pub frames_with_opcodes: usize,
}

/// Pretty printing implementation for unified snapshot debugging
impl<DB> Snapshots<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    /// Print comprehensive summary of all snapshots with frame aggregation
    ///
    /// This method provides an integrated view of both hook and opcode snapshots,
    /// organized by execution frame for easier debugging. Multiple snapshots within
    /// the same frame are aggregated and summarized for clarity.
    pub fn print_summary(&self) {
        println!(
            "\n\x1b[36m╔══════════════════════════════════════════════════════════════════╗\x1b[0m"
        );
        println!(
            "\x1b[36m║                    UNIFIED SNAPSHOTS SUMMARY                     ║\x1b[0m"
        );
        println!(
            "\x1b[36m╚══════════════════════════════════════════════════════════════════╝\x1b[0m\n"
        );

        // Get comprehensive statistics
        let stats = self.get_snapshot_stats();

        // Overall statistics section
        println!("\x1b[33m📊 Overall Statistics:\x1b[0m");
        println!("  Total snapshots: \x1b[32m{}\x1b[0m", stats.total_snapshots);
        println!("  Total frames: \x1b[32m{}\x1b[0m", stats.total_frames);
        println!(
            "  └─ Hook snapshots: \x1b[32m{}\x1b[0m ({:.1}%)",
            stats.hook_snapshots,
            if stats.total_snapshots > 0 {
                stats.hook_snapshots as f64 / stats.total_snapshots as f64 * 100.0
            } else {
                0.0
            }
        );
        println!(
            "  └─ Opcode snapshots: \x1b[32m{}\x1b[0m ({:.1}%)",
            stats.opcode_snapshots,
            if stats.total_snapshots > 0 {
                stats.opcode_snapshots as f64 / stats.total_snapshots as f64 * 100.0
            } else {
                0.0
            }
        );

        println!("\n\x1b[33m🎯 Frame Coverage:\x1b[0m");
        println!(
            "  Frames with hooks: \x1b[32m{}\x1b[0m ({:.1}%)",
            stats.frames_with_hooks,
            if stats.total_frames > 0 {
                stats.frames_with_hooks as f64 / stats.total_frames as f64 * 100.0
            } else {
                0.0
            }
        );
        println!(
            "  Frames with opcodes: \x1b[32m{}\x1b[0m ({:.1}%)",
            stats.frames_with_opcodes,
            if stats.total_frames > 0 {
                stats.frames_with_opcodes as f64 / stats.total_frames as f64 * 100.0
            } else {
                0.0
            }
        );

        if self.is_empty() {
            println!("\n\x1b[90m  No snapshots were recorded.\x1b[0m");
            return;
        }

        println!("\n\x1b[33m📋 Frame Details:\x1b[0m");
        println!(
            "\x1b[90m─────────────────────────────────────────────────────────────────\x1b[0m"
        );

        // Group snapshots by frame ID while preserving order
        let mut frame_groups: HashMap<ExecutionFrameId, Vec<&Snapshot<DB>>> = HashMap::new();
        let mut frame_order = Vec::new();

        for (frame_id, snapshot) in &self.inner {
            if !frame_groups.contains_key(frame_id) {
                frame_order.push(*frame_id);
            }
            frame_groups.entry(*frame_id).or_default().push(snapshot);
        }

        // Print frame-by-frame details
        for (display_idx, frame_id) in frame_order.iter().enumerate() {
            let snapshots = frame_groups.get(frame_id).unwrap();

            self.print_frame_summary(display_idx, *frame_id, snapshots);
        }

        println!(
            "\n\x1b[90m─────────────────────────────────────────────────────────────────\x1b[0m"
        );

        // Print legend
        println!("\n\x1b[33m📖 Legend:\x1b[0m");
        println!("  \x1b[92m🎯 Hook\x1b[0m    - Strategic instrumentation breakpoint");
        println!("  \x1b[94m⚙️ Opcode\x1b[0m  - Fine-grained instruction-level snapshot");
    }

    /// Print detailed information for a single frame
    fn print_frame_summary(
        &self,
        display_idx: usize,
        frame_id: ExecutionFrameId,
        snapshots: &[&Snapshot<DB>],
    ) {
        let hook_count = snapshots.iter().filter(|s| s.is_hook()).count();
        let opcode_count = snapshots.iter().filter(|s| s.is_opcode()).count();
        let total_count = snapshots.len();

        // Determine frame type and color
        let (frame_type, color, icon) = if hook_count > 0 && opcode_count > 0 {
            error!("Frame {} has both hook and opcode snapshots, which is unexpected.", frame_id);
            ("Mixed", "\x1b[96m", "📍")
        } else if hook_count > 0 {
            ("Hook", "\x1b[92m", "🎯")
        } else {
            ("Opcode", "\x1b[94m", "⚙️")
        };

        println!(
            "\n  {}[{:3}] {} Frame {}\x1b[0m (trace.{}, re-entry {})",
            color,
            display_idx,
            icon,
            frame_id,
            frame_id.trace_entry_id(),
            frame_id.re_entry_count()
        );

        println!(
            "       └─ Type: \x1b[33m{}\x1b[0m | Snapshots: \x1b[32m{}\x1b[0m",
            frame_type, total_count
        );

        if hook_count > 0 && opcode_count > 0 {
            println!("          ├─ Hook snapshots: \x1b[32m{}\x1b[0m", hook_count);
            println!("          └─ Opcode snapshots: \x1b[32m{}\x1b[0m", opcode_count);
        } else if hook_count > 0 {
            // Show hook details
            self.print_hook_details(snapshots, "          ");
        } else {
            // Show opcode summary
            self.print_opcode_summary(snapshots, "          ");
        }

        // Show address information
        let addresses: std::collections::HashSet<_> =
            snapshots.iter().map(|s| s.address()).collect();
        if addresses.len() == 1 {
            println!("          └─ Address: \x1b[36m{:?}\x1b[0m", addresses.iter().next().unwrap());
        } else if !addresses.is_empty() {
            println!("          └─ Addresses: \x1b[36m{} unique\x1b[0m", addresses.len());
        }
    }

    /// Print details for hook snapshots in a frame
    fn print_hook_details(&self, snapshots: &[&Snapshot<DB>], indent: &str) {
        let hook_snapshots: Vec<_> = snapshots
            .iter()
            .filter_map(|s| if let Snapshot::Hook(hook) = s { Some(hook) } else { None })
            .collect();

        if hook_snapshots.is_empty() {
            return;
        }

        let usids: Vec<_> = hook_snapshots.iter().map(|h| h.usid).collect();

        // Show USIDs with smart formatting (similar to hook_snapshot_inspector)
        if usids.len() == 1 {
            println!("{}└─ USID: \x1b[36m{}\x1b[0m", indent, usids[0]);
        } else if usids.len() <= 10 {
            let usid_list: Vec<String> = usids.iter().map(|u| u.to_string()).collect();
            println!("{}└─ USIDs: \x1b[36m[{}]\x1b[0m", indent, usid_list.join(", "));
        } else {
            let first_few: Vec<String> = usids.iter().take(3).map(|u| u.to_string()).collect();
            let last_few: Vec<String> =
                usids.iter().rev().take(3).rev().map(|u| u.to_string()).collect();

            println!(
                "{}└─ USIDs: \x1b[36m[{}, ... {}, {} total]\x1b[0m",
                indent,
                first_few.join(", "),
                last_few.join(", "),
                usids.len()
            );
        }
    }

    /// Print summary for opcode snapshots in a frame
    fn print_opcode_summary(&self, snapshots: &[&Snapshot<DB>], indent: &str) {
        let opcode_snapshots: Vec<_> = snapshots
            .iter()
            .filter_map(|s| if let Snapshot::Opcode(opcode) = s { Some(opcode) } else { None })
            .collect();

        if opcode_snapshots.is_empty() {
            return;
        }

        let pc_range = if opcode_snapshots.len() == 1 {
            format!("PC {}", opcode_snapshots[0].pc)
        } else {
            let min_pc = opcode_snapshots.iter().map(|s| s.pc).min().unwrap_or(0);
            let max_pc = opcode_snapshots.iter().map(|s| s.pc).max().unwrap_or(0);
            format!("PC {}..{}", min_pc, max_pc)
        };

        let avg_stack: f64 = if !opcode_snapshots.is_empty() {
            opcode_snapshots.iter().map(|s| s.stack.len()).sum::<usize>() as f64
                / opcode_snapshots.len() as f64
        } else {
            0.0
        };

        println!("{}├─ Range: \x1b[36m{}\x1b[0m", indent, pc_range);
        println!("{}└─ Avg stack depth: \x1b[36m{:.1}\x1b[0m", indent, avg_stack);
    }
}
