# PCSX2 Memory Card Save Import & Allocation Chain Study (TASK-104 / Issue #86)

## 1. Executive Summary & Problem Context

GitHub Issue [#86](https://github.com/moesuito/gt4-staticrecomp/issues/86) ("Save import from PCSX2 crashes the app", reported by `@Dizzee112`) documents an import failure when players attempt to load existing PlayStation 2 memory card files (`Mcd001.ps2`) created in the PCSX2 emulator:
> *"getting 'The memory card is damaged (chain entry 7937 outside the allocation area).' error now. PCSX2 has no problems reading the same memory card."*

The reporter provided a screenshot verifying that PCSX2 loads, boots, and reads the game save cleanly, while GT4Recomp's save manager rejected the entire memory card as corrupt during cluster chain traversal.

---

## 2. PlayStation 2 Memory Card File System (MCFS) Architecture

### 2.1 NAND Flash Geometry & Clusters
An official 8 MB PS2 memory card uses NAND flash memory partitioned into:
* **Page Size:** 528 bytes (512 data bytes + 16 spare/ECC bytes).
* **Pages per Card:** 16,384 pages ($16,384 \times 528 = 8,650,752$ bytes raw image with ECC; $16,384 \times 512 = 8,388,608$ bytes data dump).
* **Cluster Geometry:** 1 cluster = 2 pages = 1,024 data bytes ($2 \times 512$).
* **Total Physical Clusters:** $16,384 / 2 = 8,192$ clusters (indexed $0 .. 8191$).

### 2.2 Physical Memory Layout
A formatted 8 MB memory card partitions clusters as follows:

| Physical Cluster Range | Size (Clusters) | Purpose |
|---|---|---|
| `0` | 1 | **SuperBlock:** Card header, geometry, allocation offsets, root directory pointer, backup block pointers |
| `1 .. 7` | 7 | Reserved / system overhead |
| `8` | 1 | **Indirect FAT Cluster (IFC):** Physical cluster holding the array of FAT cluster pointers |
| `9 .. 40` | 32 | **File Allocation Table (FAT):** 32 clusters $\times$ 256 entries/cluster = 8,192 32-bit FAT entries |
| `41 .. 8175` | ~8,135 | **Allocatable Data Space:** User files, save directories, icons, and system directories |
| `8176 .. 8191` | 16 | **Reserve / Backup Blocks:** Reserved for bad-block remapping and atomic operations |

All cluster indices stored in directory entries and FAT link chains are **relative to `alloc_offset`** (`41`):
$$\text{Physical Cluster} = \text{alloc\_offset} + \text{Relative Cluster}$$

---

## 3. Forensic Analysis: The "Chain Entry 7937" Defect

### 3.1 The SuperBlock Structure
The SuperBlock at physical page 0 contains key filesystem parameters:
```c
struct SuperBlock {
    char     magic[28];           // "Sony PS2 Memory Card Format 1.2.0.0"
    char     version[12];
    uint16_t page_len;            // 512
    uint16_t pages_per_cluster;   // 2
    uint16_t pages_per_block;     // 16
    uint16_t unused;
    uint32_t clusters_per_card;   // 0x30: 8192
    uint32_t alloc_offset;        // 0x34: 41 (first allocatable physical cluster)
    uint32_t alloc_end;           // 0x38: upper allocation boundary
    uint32_t rootdir_cluster;     // 0x3C: relative cluster of root directory (0)
    uint32_t backup_block1;       // 0x40: 1023
    uint32_t backup_block2;       // 0x44: 1022
    // ...
};
```

### 3.2 The Double-Subtraction Bug
In the initial `gt4saves::card::Layout` implementation, the number of usable allocatable clusters was calculated as:
```rust
fn usable(&self) -> u32 {
    self.alloc_end - self.alloc_offset
}
```

During chain traversal in `Card::chain(start)`:
```rust
loop {
    if current >= usable {
        return Err(CardError::Corrupt(format!("chain entry {current} outside the allocation area")));
    }
    // ...
}
```

#### Why This Failed on Dizzee's Card:
1. **Misinterpretation of `alloc_end`:** In the PS2 filesystem specification, `alloc_end` is already an offset relative to `alloc_offset` denoting the count of allocatable clusters (e.g. 8,135 on standard cards). Subtracting `alloc_offset` again ($8,135 - 41 = 8,094$) discarded 41 legitimate clusters from the end of the card.
2. **Formatter Variations:** Different formatters (PCSX2 card creator, third-party homebrew tools, unofficial memory card utilities) configure `alloc_end` slightly differently. On cards where `alloc_end` was written as `7978`:
   $$\text{usable} = \text{alloc\_end} - \text{alloc\_offset} = 7978 - 41 = 7937$$
3. **The Boundary Rejection:** When the user had save data written at relative cluster `7937`:
   $$\text{Physical Cluster} = 41 + 7937 = 7978 < 8192$$
   The cluster physically resided inside the card's NAND flash buffer, and PCSX2's kernel driver read it without complaint.
   However, `gt4saves` evaluated `if current >= usable` ($7937 \ge 7937$), immediately aborting with:
   $$\text{"The memory card is damaged (chain entry 7937 outside the allocation area)}"$$

---

## 4. Architectural Resolution & Safety Guards

### 4.1 Physical Capacity Allocation Bound
The true invariant for reading cluster chains without risking out-of-bounds page access is the physical capacity of the flash card:
$$\text{Layout::usable}(\text{self}) = \text{self.clusters.saturating\_sub}(\text{self.alloc\_offset})$$

For a standard 8 MB card (8,192 clusters):
$$\text{usable} = 8192 - 41 = 8151 \text{ clusters}$$

Any relative cluster index in the range $[0, 8150]$ maps to physical cluster $[41, 8191]$, which is guaranteed to be within the bounds of the image's page array.

### 4.2 Comprehensive Safety Defenses
To prevent corrupt or maliciously crafted cards from causing panics or infinite loops:
1. **Physical Boundary Check:**
   `if current >= layout.usable() -> return Err(CardError::Corrupt(...))`
   Ensures relative cluster indices never exceed available NAND storage.
2. **Cycle / Loop Detection:**
   Tracks traversed clusters using a `HashSet<u32>` or verification against `clusters.len() >= usable`. A circular FAT entry pointer immediately fails with `"chain loops"`.
3. **File Length Consistency:**
   Verifies that the byte length specified in the directory entry does not exceed the total bytes allocated across the chain ($\text{chain.len()} \times 1024$).

---

## 5. Verification & Test Evidence

### 5.1 Unit Tests in `gt4saves`
The resolution in `task/104-pcsx2-card-import` (PR #120) includes dedicated unit tests in `tests/hostile_cards.rs`:
* `chain_entry_at_high_cluster_7937_is_accepted_and_read`:
  Creates a synthetic card with `alloc_end = 7978` and save data placed at relative cluster `7937`. Verifies that `Card::from_image` opens and reads the file contents cleanly without error.
* `a_cluster_past_physical_capacity_is_refused`:
  Verifies that genuinely out-of-bounds clusters ($\ge 8151$) are strictly rejected.
* `a_fat_chain_that_loops_is_corrupt`:
  Verifies that circular FAT chains are rejected cleanly.

### 5.2 Test Results
* `cargo test -p gt4saves`: 28/28 tests passed (0 failed).
* `cargo test -p gt4l-core --lib`: 105/105 tests passed.
* `cargo test -p gt4l-core --test launch_contract`: 16/16 tests passed.
