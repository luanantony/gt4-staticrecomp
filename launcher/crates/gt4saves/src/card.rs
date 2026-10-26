//! PS2 memory card image: 8 MB of 528-byte pages (512 data + 16 spare, ECC in the first 12).
//!
//! Layout used here (fitted to the owner's card, see tests/card_owner.rs):
//! * cluster = two pages (1024 bytes); physical cluster c holds pages 2c and 2c+1;
//! * superblock at page 0; the indirect FAT list at superblock offset 0x50 names the
//!   physical cluster that lists the FAT clusters (8 on a fresh card);
//! * the FAT maps cluster numbers relative to `alloc_offset` (41) to the next relative
//!   cluster of a chain, or `FAT_END`; unused entries are `FAT_FREE`;
//! * the root directory is relative cluster 0; a directory holds 512-byte entries.
//!
//! Only what the launcher needs is implemented: read, list, read files, and build a new
//! card from a tree. Repairing damaged cards is out of scope.

use crate::ecc;
use std::collections::HashSet;
use std::fmt;

pub const PAGE_DATA_BYTES: usize = 512;
pub const PAGE_SPARE_BYTES: usize = 16;
pub const PAGE_BYTES: usize = PAGE_DATA_BYTES + PAGE_SPARE_BYTES;
pub const PAGE_COUNT: usize = 16384;
/// Card image with spare bytes (what the game and PCSX2 write). 8,650,752 bytes.
pub const IMAGE_WITH_ECC: usize = PAGE_BYTES * PAGE_COUNT;
/// Card dump without spare bytes. 8,388,608 bytes.
pub const IMAGE_WITHOUT_ECC: usize = PAGE_DATA_BYTES * PAGE_COUNT;
pub const CLUSTER_BYTES: usize = 1024;
pub const DIRENT_BYTES: usize = 512;

pub const MAGIC: &[u8] = b"Sony PS2 Memory Card Format 1.2.0.0";
/// FAT values (verified on the owner's card): a used entry is 0x8000_0000 | next cluster,
/// the last cluster of a chain holds FAT_END, an unused entry holds FAT_FREE.
pub const FAT_END: u32 = 0xFFFF_FFFF;
pub const FAT_FREE: u32 = 0x7FFF_FFFF;
const FAT_IN_USE: u32 = 0x8000_0000;
/// Mode of the ".." entry on the owner's card (it carries the 0x2000 flag).
pub const MODE_DOTDOT: u16 = 0xA426;

pub const MODE_EXISTS: u16 = 0x8000;
pub const MODE_FILE: u16 = 0x0010;
pub const MODE_SUBDIR: u16 = 0x0020;
pub const MODE_DIR_DEFAULT: u16 = 0x8427;
pub const MODE_FILE_DEFAULT: u16 = 0x8497;

/// Physical cluster that lists the FAT clusters on a card the launcher builds (fresh 8 MB cards, the owner's card).
const IFC_CLUSTER: u32 = 8;
/// Layout of the 8 MB cards the launcher builds (the owner's card has the same values in its superblock).
const ALLOC_OFFSET: u32 = 41;
const ALLOC_END: u32 = 8135;
const FAT_CLUSTERS: u32 = 32;
const FAT_ENTRIES_PER_CLUSTER: u32 = (CLUSTER_BYTES / 4) as u32;

/// Clusters per card the reader accepts: 8, 16, 32 and 64 MB (1024-byte clusters).
pub const SUPPORTED_CLUSTERS: [u32; 4] = [8192, 16384, 32768, 65536];
/// Deepest directory the reader follows (GT4 saves sit two levels down at most).
pub const MAX_DIRECTORY_DEPTH: usize = 8;
/// Entries of the superblock's indirect FAT list (each names a cluster of 256 FAT cluster numbers).
const INDIRECT_LIST_ENTRIES: usize = 32;
/// Superblock field offsets (page 0).
const SB_PAGE_SIZE: usize = 0x28;
const SB_PAGE_PER_CLUSTER: usize = 0x2A;
const SB_CLUSTERS: usize = 0x30;
const SB_ALLOC_OFFSET: usize = 0x34;
const SB_ALLOC_END: usize = 0x38;
const SB_INDIRECT_LIST: usize = 0x50;

/// Where the allocation area of a card lies. Read from the superblock; the launcher's own 8 MB cards use
/// [`Layout::EIGHT_MB`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    /// Clusters on the whole card (page count / 2).
    pub clusters: u32,
    /// Physical cluster of relative cluster 0 (the root directory).
    pub alloc_offset: u32,
    /// One past the last physical cluster of the allocation area (from the superblock).
    pub alloc_end: u32,
}

impl Layout {
    /// The layout of the 8 MB cards the launcher builds.
    pub const EIGHT_MB: Layout = Layout { clusters: 8192, alloc_offset: ALLOC_OFFSET, alloc_end: ALLOC_END };

    /// Relative clusters that physically fit in the card's allocation space.
    pub fn max_clusters(&self) -> u32 {
        self.clusters.saturating_sub(self.alloc_offset)
    }

    /// Relative clusters of the allocation area (the FAT has one entry for each).
    ///
    /// In the Sony PS2 SuperBlock, `alloc_end` is the relative cluster limit (e.g. 8135 on an
    /// 8 MB card with 41 alloc_offset + 8135 allocatable + 16 backup blocks = 8192 total clusters).
    /// Some third-party or homebrew formatters record `alloc_end` as a physical cluster boundary,
    /// or write files into clusters past `alloc_end` up to the card's physical flash limit (`clusters - alloc_offset`).
    ///
    /// Bounding by the physical capacity `clusters - alloc_offset` ensures all valid clusters
    /// present on the card (such as cluster 7937 on PCSX2 cards, issue #86) can be read safely
    /// while still strictly preventing out-of-bounds cluster reads.
    pub fn usable(&self) -> u32 {
        self.max_clusters()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardError {
    /// The image length fits no memory card form (not a PS2 memory card file).
    BadSize(usize),
    /// The image is a card of a size the reader does not support (for example 4 MB or 128 MB).
    Unsupported(String),
    /// The superblock magic is missing: not a PS2 memory card.
    NotACard,
    /// A chain, directory or FAT entry points outside the card, loops, or a length does not match.
    Corrupt(String),
    /// The tree does not fit the free clusters of an 8 MB card.
    Full,
    /// A name cannot be stored (empty, longer than 32 bytes or not ASCII).
    BadName(String),
}

impl fmt::Display for CardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CardError::BadSize(size) => write!(f, "not a PS2 card image: {size} bytes"),
            CardError::Unsupported(what) => write!(f, "unsupported card size: {what}"),
            CardError::NotACard => write!(f, "not a PS2 memory card: superblock magic missing"),
            CardError::Corrupt(what) => write!(f, "corrupt card: {what}"),
            CardError::Full => write!(f, "the save does not fit on an 8 MB card"),
            CardError::BadName(name) => write!(f, "name cannot be stored on a card: {name:?}"),
        }
    }
}

impl std::error::Error for CardError {}

/// One 512-byte directory entry. The raw bytes are kept so that fields this crate does not
/// interpret (padding, attributes) survive a round trip unchanged.
#[derive(Clone, PartialEq, Eq)]
pub struct DirEntry {
    raw: Vec<u8>,
}

impl fmt::Debug for DirEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DirEntry")
            .field("name", &self.name())
            .field("mode", &format_args!("{:#06x}", self.mode()))
            .field("length", &self.length())
            .field("cluster", &self.cluster())
            .finish()
    }
}

impl DirEntry {
    pub fn blank() -> Self {
        DirEntry { raw: vec![0u8; DIRENT_BYTES] }
    }

    /// Builds an entry from its interpreted fields; padding stays zero.
    pub fn new(name: &str, mode: u16, created: [u8; 8], modified: [u8; 8]) -> Result<Self, CardError> {
        validate_name(name)?;
        let mut entry = DirEntry::blank();
        entry.set_mode(mode);
        entry.raw[0x08..0x10].copy_from_slice(&created);
        entry.raw[0x18..0x20].copy_from_slice(&modified);
        entry.raw[0x40..0x40 + name.len()].copy_from_slice(name.as_bytes());
        Ok(entry)
    }

    pub fn from_raw(raw: &[u8]) -> Result<Self, CardError> {
        if raw.len() != DIRENT_BYTES {
            return Err(CardError::Corrupt("directory entry is not 512 bytes".into()));
        }
        Ok(DirEntry { raw: raw.to_vec() })
    }

    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    pub fn mode(&self) -> u16 {
        u16::from_le_bytes([self.raw[0], self.raw[1]])
    }

    pub fn set_mode(&mut self, mode: u16) {
        self.raw[0..2].copy_from_slice(&mode.to_le_bytes());
    }

    pub fn length(&self) -> u32 {
        u32::from_le_bytes(self.raw[4..8].try_into().unwrap())
    }

    pub fn set_length(&mut self, length: u32) {
        self.raw[4..8].copy_from_slice(&length.to_le_bytes());
    }

    /// Raw 8-byte timestamp of creation: unused, sec, min, hour, day, month, year (u16).
    pub fn created(&self) -> [u8; 8] {
        self.raw[0x08..0x10].try_into().unwrap()
    }

    pub fn modified(&self) -> [u8; 8] {
        self.raw[0x18..0x20].try_into().unwrap()
    }

    /// Relative cluster of the first cluster of the entry's data.
    pub fn cluster(&self) -> u32 {
        u32::from_le_bytes(self.raw[0x10..0x14].try_into().unwrap())
    }

    pub fn set_cluster(&mut self, cluster: u32) {
        self.raw[0x10..0x14].copy_from_slice(&cluster.to_le_bytes());
    }

    /// In a sub-directory's "." entry: the directory's own index in its parent directory.
    /// GT4 refuses to load a save whose "." entry does not hold it ("Load failed!").
    pub fn dir_entry(&self) -> u32 {
        u32::from_le_bytes(self.raw[0x14..0x18].try_into().unwrap())
    }

    pub fn set_dir_entry(&mut self, index: u32) {
        self.raw[0x14..0x18].copy_from_slice(&index.to_le_bytes());
    }

    pub fn is_dir(&self) -> bool {
        self.mode() & MODE_SUBDIR != 0
    }

    pub fn exists(&self) -> bool {
        self.mode() & MODE_EXISTS != 0
    }

    pub fn name(&self) -> String {
        let field = &self.raw[0x40..0x60];
        let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
        String::from_utf8_lossy(&field[..end]).into_owned()
    }

    /// Entries with names "." and ".." are skipped by listings.
    pub fn is_dot(&self) -> bool {
        matches!(self.name().as_str(), "." | "..")
    }
}

pub fn validate_name(name: &str) -> Result<(), CardError> {
    if name.is_empty() || name.len() > 32 || !name.is_ascii() || name.contains('/') {
        return Err(CardError::BadName(name.to_string()));
    }
    Ok(())
}

/// A card directory or file with its children or data, ready to be written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub entry: DirEntry,
    /// Children of a directory (empty for files).
    pub children: Vec<Node>,
    /// File contents (empty for directories).
    pub data: Vec<u8>,
}

impl Node {
    pub fn file(entry: DirEntry, data: Vec<u8>) -> Self {
        debug_assert!(!entry.is_dir());
        Node { entry, children: Vec::new(), data }
    }

    pub fn dir(entry: DirEntry, children: Vec<Node>) -> Self {
        debug_assert!(entry.is_dir());
        Node { entry, children, data: Vec::new() }
    }

    pub fn name(&self) -> String {
        self.entry.name()
    }

    pub fn is_dir(&self) -> bool {
        self.entry.is_dir()
    }

    pub fn is_file(&self) -> bool {
        !self.entry.is_dir()
    }

    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name() == name)
    }
}

/// A PS2 memory card held in memory in its 528-byte page form.
#[derive(Clone, PartialEq, Eq)]
pub struct Card {
    pages: Vec<u8>,
    layout: Layout,
}

impl fmt::Debug for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Card({} bytes, {:?})", self.pages.len(), self.layout)
    }
}

/// The form of a card image from its length: `(pages, with spare bytes)`. A length fits only one form, because
/// 528 and 512 share no supported page count (528/512 is 33/32, never a power of two).
fn page_form(length: usize) -> Option<(usize, bool)> {
    let supported = |pages: usize| SUPPORTED_CLUSTERS.iter().any(|&clusters| clusters as usize * 2 == pages);
    if length % PAGE_BYTES == 0 && supported(length / PAGE_BYTES) {
        return Some((length / PAGE_BYTES, true));
    }
    if length % PAGE_DATA_BYTES == 0 && supported(length / PAGE_DATA_BYTES) {
        return Some((length / PAGE_DATA_BYTES, false));
    }
    None
}

impl Card {
    /// Opens a card image with spare bytes, or a data-only dump (the spare bytes are then computed). The size and the
    /// layout come from the superblock and are checked against the image before anything else is read.
    pub fn from_image(bytes: &[u8]) -> Result<Self, CardError> {
        let Some((pages, with_spare)) = page_form(bytes.len()) else {
            // A card by its magic, of a size the reader does not take: say so; anything else is not a card.
            if bytes.starts_with(MAGIC) {
                return Err(CardError::Unsupported(format!("{} bytes", bytes.len())));
            }
            return Err(CardError::BadSize(bytes.len()));
        };
        let pages_bytes = if with_spare {
            bytes.to_vec()
        } else {
            let mut pages_bytes = Vec::with_capacity(pages * PAGE_BYTES);
            for page in bytes.chunks_exact(PAGE_DATA_BYTES) {
                pages_bytes.extend_from_slice(page);
                if page.iter().all(|&b| b == 0xFF) {
                    pages_bytes.extend_from_slice(&[0xFF; PAGE_SPARE_BYTES]); // erased page
                } else {
                    pages_bytes.extend_from_slice(&ecc::page_spare(page));
                }
            }
            pages_bytes
        };
        let mut card = Card { pages: pages_bytes, layout: Layout::EIGHT_MB };
        if !card.superblock_ok() {
            return Err(CardError::NotACard);
        }
        card.layout = card.superblock_layout()?;
        Ok(card)
    }

    /// Size of the card in bytes (its clusters times 1024).
    fn card_bytes(&self) -> usize {
        self.layout.clusters as usize * CLUSTER_BYTES
    }

    /// Reads and checks the superblock's layout against the image size. Every value the reader uses later is either
    /// checked here or checked where it is used.
    fn superblock_layout(&self) -> Result<Layout, CardError> {
        let image_clusters = (self.pages.len() / PAGE_BYTES / 2) as u32;
        let page_size = self.read_u16(0, SB_PAGE_SIZE);
        if page_size as usize != PAGE_DATA_BYTES {
            return Err(CardError::Unsupported(format!("page size {page_size} bytes")));
        }
        let pages_per_cluster = self.read_u16(0, SB_PAGE_PER_CLUSTER);
        if pages_per_cluster != 2 {
            return Err(CardError::Corrupt(format!("{pages_per_cluster} pages per cluster")));
        }
        let clusters = self.read_u32(0, SB_CLUSTERS);
        if clusters != image_clusters {
            return Err(CardError::Corrupt(format!("superblock names {clusters} clusters, the image holds {image_clusters}")));
        }
        if !SUPPORTED_CLUSTERS.contains(&clusters) {
            return Err(CardError::Unsupported(format!("{} MB", clusters as u64 * CLUSTER_BYTES as u64 / (1024 * 1024))));
        }
        let alloc_offset = self.read_u32(0, SB_ALLOC_OFFSET);
        let alloc_end = self.read_u32(0, SB_ALLOC_END);
        if alloc_offset >= clusters || alloc_end == 0 {
            return Err(CardError::Corrupt(format!("allocation area {alloc_offset}..{alloc_end} outside {clusters} clusters")));
        }
        let layout = Layout { clusters, alloc_offset, alloc_end };
        // Every FAT cluster the allocation area needs must name a cluster on this card.
        let fat_clusters = layout.usable().div_ceil(FAT_ENTRIES_PER_CLUSTER);
        for fat_index in 0..fat_clusters {
            let physical = self.fat_table_cluster_with(layout, fat_index)?;
            if physical >= clusters {
                return Err(CardError::Corrupt(format!("FAT cluster {physical} outside the card")));
            }
        }
        Ok(layout)
    }

    /// The image with spare bytes, as PCSX2 and the game write it.
    pub fn to_image(&self) -> Vec<u8> {
        self.pages.clone()
    }

    /// The image without spare bytes (data-only dump).
    pub fn to_image_without_ecc(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pages.len() / PAGE_BYTES * PAGE_DATA_BYTES);
        for page in self.pages.chunks_exact(PAGE_BYTES) {
            out.extend_from_slice(&page[..PAGE_DATA_BYTES]);
        }
        out
    }

    /// The card's layout as read from its superblock.
    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// Returns true when every page's ECC bytes match the data. Erased pages (all 0xFF,
    /// spare bytes included) are valid as they are.
    pub fn ecc_ok(&self) -> bool {
        for page in self.pages.chunks_exact(PAGE_BYTES) {
            if page.iter().all(|&b| b == 0xFF) {
                continue;
            }
            let data = &page[..PAGE_DATA_BYTES];
            let spare = &page[PAGE_DATA_BYTES..];
            if spare[..12] != ecc::page_spare(data) {
                return false;
            }
        }
        true
    }

    pub fn superblock_ok(&self) -> bool {
        self.page(0).starts_with(MAGIC)
    }

    fn page(&self, index: usize) -> &[u8] {
        &self.pages[index * PAGE_BYTES..index * PAGE_BYTES + PAGE_DATA_BYTES]
    }

    fn set_page(&mut self, index: usize, data: &[u8]) {
        debug_assert_eq!(data.len(), PAGE_DATA_BYTES);
        let start = index * PAGE_BYTES;
        self.pages[start..start + PAGE_DATA_BYTES].copy_from_slice(data);
        let spare = ecc::page_spare(data);
        self.pages[start + PAGE_DATA_BYTES..start + PAGE_DATA_BYTES + 12].copy_from_slice(&spare);
        self.pages[start + PAGE_DATA_BYTES + 12..start + PAGE_BYTES].copy_from_slice(&[0; 4]);
    }

    fn read_u32(&self, page: usize, offset: usize) -> u32 {
        u32::from_le_bytes(self.page(page)[offset..offset + 4].try_into().unwrap())
    }

    fn read_u16(&self, page: usize, offset: usize) -> u16 {
        u16::from_le_bytes(self.page(page)[offset..offset + 2].try_into().unwrap())
    }

    /// Data of a physical cluster (two pages, 1024 bytes).
    fn cluster(&self, physical: u32) -> Result<Vec<u8>, CardError> {
        if physical >= self.layout.clusters {
            return Err(CardError::Corrupt(format!("cluster {physical} outside the card")));
        }
        let first = physical as usize * 2;
        let mut data = Vec::with_capacity(CLUSTER_BYTES);
        data.extend_from_slice(self.page(first));
        data.extend_from_slice(self.page(first + 1));
        Ok(data)
    }

    fn set_cluster(&mut self, physical: u32, data: &[u8]) {
        debug_assert_eq!(data.len(), CLUSTER_BYTES);
        let first = physical as usize * 2;
        self.set_page(first, &data[..PAGE_DATA_BYTES]);
        self.set_page(first + 1, &data[PAGE_DATA_BYTES..]);
    }

    /// Physical cluster of FAT cluster `fat_index` (the FAT cluster number, 256 entries each). The superblock's
    /// indirect list names one cluster per 256 FAT clusters; each of those lists its FAT clusters.
    fn fat_table_cluster_with(&self, layout: Layout, fat_index: u32) -> Result<u32, CardError> {
        let list_slot = (fat_index / FAT_ENTRIES_PER_CLUSTER) as usize;
        if list_slot >= INDIRECT_LIST_ENTRIES {
            return Err(CardError::Corrupt("FAT beyond the indirect FAT list".into()));
        }
        let indirect = self.read_u32(0, SB_INDIRECT_LIST + list_slot * 4);
        if indirect >= layout.clusters {
            return Err(CardError::Corrupt(format!("indirect FAT list names cluster {indirect} outside the card")));
        }
        let indirect_data = self.cluster(indirect)?;
        let offset = (fat_index % FAT_ENTRIES_PER_CLUSTER) as usize * 4;
        Ok(u32::from_le_bytes(indirect_data[offset..offset + 4].try_into().unwrap()))
    }

    fn fat_table_cluster(&self, fat_index: u32) -> Result<u32, CardError> {
        self.fat_table_cluster_with(self.layout, fat_index)
    }

    /// FAT entry for a relative cluster.
    pub fn fat_entry(&self, relative: u32) -> Result<u32, CardError> {
        let table = self.fat_table_cluster(relative / FAT_ENTRIES_PER_CLUSTER)?;
        let data = self.cluster(table)?;
        let offset = (relative % FAT_ENTRIES_PER_CLUSTER) as usize * 4;
        Ok(u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()))
    }

    /// Follows a chain from `start` and returns its clusters (relative numbers). The walk stops at the first entry
    /// that is not a cluster of the allocation area, and never visits more clusters than the area holds.
    pub fn chain(&self, start: u32) -> Result<Vec<u32>, CardError> {
        let usable = self.layout.usable();
        let mut clusters = Vec::new();
        let mut visited = HashSet::new();
        let mut current = start;
        loop {
            if current >= usable {
                return Err(CardError::Corrupt(format!("chain entry {current} outside the allocation area")));
            }
            if !visited.insert(current) {
                return Err(CardError::Corrupt(format!("FAT loop at cluster {current}")));
            }
            clusters.push(current);
            let raw = self.fat_entry(current)?;
            if raw == FAT_END {
                return Ok(clusters);
            }
            if raw & FAT_IN_USE == 0 {
                return Err(CardError::Corrupt(format!(
                    "chain reaches unused cluster {current} (FAT entry 0x{raw:08X})"
                )));
            }
            current = raw & !FAT_IN_USE;
        }
    }

    /// Reads `length` bytes of the chain that starts at `start`. The length is checked against the chain's clusters
    /// before anything is allocated, so a corrupt length cannot ask for more memory than the chain holds.
    pub fn read_chain(&self, start: u32, length: usize) -> Result<Vec<u8>, CardError> {
        let clusters = self.chain(start)?;
        let needed = length.div_ceil(CLUSTER_BYTES);
        if needed > clusters.len() {
            return Err(CardError::Corrupt(format!(
                "a {length}-byte item is longer than its {}-cluster chain",
                clusters.len()
            )));
        }
        let mut out = Vec::with_capacity(needed * CLUSTER_BYTES);
        for &relative in &clusters[..needed] {
            out.extend_from_slice(&self.cluster(self.layout.alloc_offset + relative)?);
        }
        out.truncate(length);
        Ok(out)
    }

    /// The `count` entries of the directory at relative cluster `start`, including "." and "..".
    /// The count is the length of the directory's entry in its parent (the root has none:
    /// its own "." entry carries it, see `root`).
    pub fn read_dir(&self, start: u32, count: usize) -> Result<Vec<DirEntry>, CardError> {
        if count.saturating_mul(DIRENT_BYTES) > self.card_bytes() {
            return Err(CardError::Corrupt(format!("directory of {count} entries is larger than the card")));
        }
        let raw = self.read_chain(start, count * DIRENT_BYTES)?;
        raw.chunks_exact(DIRENT_BYTES).map(DirEntry::from_raw).collect()
    }

    /// Reads a directory and all its contents. `count` is its entry count from the parent.
    pub fn read_tree(&self, start: u32, count: usize) -> Result<Vec<Node>, CardError> {
        self.read_tree_below(start, count, 0, &mut HashSet::new())
    }

    /// Reads a directory at `depth` (the root is 0). Every directory cluster is visited once: a cluster met a second
    /// time means a loop, and a depth past [`MAX_DIRECTORY_DEPTH`] is refused.
    fn read_tree_below(
        &self,
        start: u32,
        count: usize,
        depth: usize,
        visited: &mut HashSet<u32>,
    ) -> Result<Vec<Node>, CardError> {
        if depth > MAX_DIRECTORY_DEPTH {
            return Err(CardError::Corrupt(format!("directories nested deeper than {MAX_DIRECTORY_DEPTH}")));
        }
        if !visited.insert(start) {
            return Err(CardError::Corrupt(format!("directory loop at cluster {start}")));
        }
        let mut nodes = Vec::new();
        for entry in self.read_dir(start, count)? {
            if entry.is_dot() || !entry.exists() {
                continue;
            }
            if entry.is_dir() {
                let children = self.read_tree_below(entry.cluster(), entry.length() as usize, depth + 1, visited)?;
                nodes.push(Node::dir(entry, children));
            } else {
                let length = entry.length() as usize;
                if length > self.card_bytes() {
                    return Err(CardError::Corrupt(format!("file {} is larger than the card", entry.name())));
                }
                let data = if length == 0 { Vec::new() } else { self.read_chain(entry.cluster(), length)? };
                nodes.push(Node::file(entry, data));
            }
        }
        Ok(nodes)
    }

    /// Reads the root directory and every save and replay on the card.
    pub fn root(&self) -> Result<Vec<Node>, CardError> {
        let dot = self.read_dir(0, 1)?;
        let root_count = dot.first().map(|e| e.length() as usize).unwrap_or(0);
        self.read_tree(0, root_count)
    }

    /// A freshly erased, blank 8 MB card (as on the owner's card: unused areas 0xFF,
    /// superblock at page 0, backup blocks at the card's end).
    pub fn blank() -> Self {
        Builder::new().finish()
    }

    /// Formats a fresh 8 MB card with `items` written into its root directory.
    pub fn build(items: &[Node]) -> Result<Self, CardError> {
        let mut b = Builder::new();
        b.write_root(items)?;
        Ok(b.finish())
    }
}

/// Builds a fresh 8 MB card.
struct Builder {
    card: Card,
    fat: Vec<u32>,
    next_free: u32,
}

impl Builder {
    fn new() -> Self {
        let usable = (ALLOC_END - ALLOC_OFFSET) as usize;
        // Erased pages are 0xFF everywhere, spare bytes included (as on the owner's card).
        let mut card = Card { pages: vec![0xFFu8; IMAGE_WITH_ECC], layout: Layout::EIGHT_MB };
        // Superblock as on the owner's card: unused areas 0xFF, backup blocks at the card's end.
        let mut superblock = vec![0u8; PAGE_DATA_BYTES];
        superblock[..MAGIC.len()].copy_from_slice(MAGIC);
        superblock[0x28..0x2A].copy_from_slice(&512u16.to_le_bytes());
        superblock[0x2A..0x2C].copy_from_slice(&2u16.to_le_bytes());
        superblock[0x2C..0x2E].copy_from_slice(&16u16.to_le_bytes());
        superblock[0x2F] = 0xFF;
        superblock[0x30..0x34].copy_from_slice(&8192u32.to_le_bytes());
        superblock[0x34..0x38].copy_from_slice(&ALLOC_OFFSET.to_le_bytes());
        superblock[0x38..0x3C].copy_from_slice(&ALLOC_END.to_le_bytes());
        superblock[0x3C..0x40].copy_from_slice(&0u32.to_le_bytes()); // root directory at cluster 0
        superblock[0x40..0x44].copy_from_slice(&1023u32.to_le_bytes()); // backup block 1: physical block 1023
        superblock[0x44..0x48].copy_from_slice(&1022u32.to_le_bytes()); // backup block 2: physical block 1022
        for slot in 0..32usize {
            let value = if slot == 0 { IFC_CLUSTER } else { FAT_END };
            superblock[0x50 + slot * 4..0x54 + slot * 4].copy_from_slice(&value.to_le_bytes());
        }
        superblock[0x60..0xE0].fill(0xFF); // bad block list: empty (all 0xFF)
        superblock[0xE0] = 2; // card type: standard PS2
        superblock[0xE1] = 0x2B; // card flags
        superblock[0xE2..PAGE_DATA_BYTES].fill(0xFF);
        card.set_page(0, &superblock);

        // Indirect FAT list: physical cluster 8 lists physical clusters 9..40 (the 32 FAT clusters).
        let mut ifc = vec![0xFFu8; CLUSTER_BYTES];
        for (i, cluster) in (9..9 + FAT_CLUSTERS).enumerate() {
            ifc[i * 4..i * 4 + 4].copy_from_slice(&cluster.to_le_bytes());
        }
        card.set_cluster(IFC_CLUSTER, &ifc);

        let mut fat = vec![FAT_FREE; usable];
        fat[0] = FAT_IN_USE; // the root directory's chain, relinked in finish()
        Builder { card, fat, next_free: 1 }
    }

    fn allocate(&mut self) -> Result<u32, CardError> {
        if self.next_free as usize >= self.fat.len() {
            return Err(CardError::Full);
        }
        let cluster = self.next_free;
        self.next_free += 1;
        self.fat[cluster as usize] = FAT_IN_USE;
        Ok(cluster)
    }

    /// Links a chain of relative clusters into the FAT. The last cluster gets FAT_END.
    fn link_chain(&mut self, clusters: &[u32]) {
        for (i, &cluster) in clusters.iter().enumerate() {
            let next = match clusters.get(i + 1) {
                Some(&c) => FAT_IN_USE | c,
                None => FAT_END,
            };
            self.fat[cluster as usize] = next;
        }
    }

    /// Writes raw bytes across a chain of clusters; unused tail of the last cluster stays 0.
    fn write_chain_bytes(&mut self, clusters: &[u32], data: &[u8]) {
        for (i, &cluster) in clusters.iter().enumerate() {
            let mut block = vec![0u8; CLUSTER_BYTES];
            let start = i * CLUSTER_BYTES;
            if start < data.len() {
                let end = (start + CLUSTER_BYTES).min(data.len());
                block[..end - start].copy_from_slice(&data[start..end]);
            }
            self.card.set_cluster(ALLOC_OFFSET + cluster, &block);
        }
    }

    /// Writes one file: allocates its chain, writes its data and links the FAT.
    fn write_file(&mut self, data: &[u8]) -> Result<(u32, usize), CardError> {
        if data.is_empty() {
            return Ok((FAT_END, 0));
        }
        let needed = data.len().div_ceil(CLUSTER_BYTES);
        let mut clusters = Vec::with_capacity(needed);
        for _ in 0..needed {
            clusters.push(self.allocate()?);
        }
        self.link_chain(&clusters);
        self.write_chain_bytes(&clusters, data);
        Ok((clusters[0], needed))
    }

    /// Writes a directory and every child below it. Returns the directory's first cluster.
    fn write_directory(
        &mut self,
        items: &[Node],
        parent_cluster: u32,
        self_index_in_parent: u32,
    ) -> Result<(u32, usize), CardError> {
        let count = items.len() + 2; // includes "." and ".."
        let needed = (count * DIRENT_BYTES).div_ceil(CLUSTER_BYTES);
        let mut dir_clusters = Vec::with_capacity(needed);
        for _ in 0..needed {
            dir_clusters.push(self.allocate()?);
        }
        self.link_chain(&dir_clusters);

        // "." and ".." entries.
        let mut dot = DirEntry::new(".", MODE_DIR_DEFAULT, [0; 8], [0; 8])?;
        dot.set_length(count as u32);
        dot.set_cluster(0);
        dot.set_dir_entry(self_index_in_parent);

        let mut dotdot = DirEntry::new("..", MODE_DOTDOT, [0; 8], [0; 8])?;
        dotdot.set_cluster(parent_cluster);

        // Write children first so their first-cluster numbers are known.
        let mut child_entries = Vec::with_capacity(items.len());
        for (position, item) in items.iter().enumerate() {
            let mut entry = item.entry.clone();
            if item.is_dir() {
                let (first, _blocks) = self.write_directory(&item.children, dir_clusters[0], position as u32 + 2)?;
                entry.set_cluster(first);
                entry.set_length((item.children.len() + 2) as u32);
            } else {
                let (first, _blocks) = self.write_file(&item.data)?;
                entry.set_cluster(first);
                entry.set_length(item.data.len() as u32);
            }
            child_entries.push(entry);
        }

        // Pack the directory cluster bytes.
        let mut raw = Vec::with_capacity(needed * CLUSTER_BYTES);
        raw.extend_from_slice(dot.raw());
        raw.extend_from_slice(dotdot.raw());
        for entry in &child_entries {
            raw.extend_from_slice(entry.raw());
        }
        self.write_chain_bytes(&dir_clusters, &raw);
        Ok((dir_clusters[0], needed))
    }

    fn write_root(&mut self, items: &[Node]) -> Result<(), CardError> {
        let count = items.len() + 2;
        let needed = (count * DIRENT_BYTES).div_ceil(CLUSTER_BYTES);
        let mut root_clusters = Vec::with_capacity(needed);
        root_clusters.push(0); // root always starts at cluster 0
        for _ in 1..needed {
            root_clusters.push(self.allocate()?);
        }
        self.link_chain(&root_clusters);

        let mut dot = DirEntry::new(".", MODE_DIR_DEFAULT, [0; 8], [0; 8])?;
        dot.set_length(count as u32);
        dot.set_cluster(0);

        let mut dotdot = DirEntry::new("..", MODE_DIR_DEFAULT, [0; 8], [0; 8])?;
        dotdot.set_cluster(0);

        let mut child_entries = Vec::with_capacity(items.len());
        for (position, item) in items.iter().enumerate() {
            let mut entry = item.entry.clone();
            if item.is_dir() {
                let (first, _blocks) = self.write_directory(&item.children, 0, position as u32 + 2)?;
                entry.set_cluster(first);
                entry.set_length((item.children.len() + 2) as u32);
            } else {
                let (first, _blocks) = self.write_file(&item.data)?;
                entry.set_cluster(first);
                entry.set_length(item.data.len() as u32);
            }
            child_entries.push(entry);
        }

        let mut raw = Vec::with_capacity(needed * CLUSTER_BYTES);
        raw.extend_from_slice(dot.raw());
        raw.extend_from_slice(dotdot.raw());
        for entry in &child_entries {
            raw.extend_from_slice(entry.raw());
        }
        self.write_chain_bytes(&root_clusters, &raw);
        Ok(())
    }

    fn finish(mut self) -> Card {
        // Write the FAT table into physical clusters 9..40 (256 entries per cluster).
        for (cluster_index, chunk) in self.fat.chunks(FAT_ENTRIES_PER_CLUSTER as usize).enumerate() {
            let physical = 9 + cluster_index as u32;
            let mut block = vec![0xFFu8; CLUSTER_BYTES];
            for (i, &entry) in chunk.iter().enumerate() {
                block[i * 4..i * 4 + 4].copy_from_slice(&entry.to_le_bytes());
            }
            self.card.set_cluster(physical, &block);
        }
        self.card
    }
}

/// Name of the GT4 game save directory (fixed by the game).
pub const GAME_SAVE_NAME: &str = "BASCUS-97328GAMEDATA";
/// Prefix shared by the GT4 save and replay directory names.
pub const GAME_PREFIX: &str = "BASCUS-97328";

pub fn is_game_save_name(name: &str) -> bool {
    name == GAME_SAVE_NAME
}

/// Replay directories: the prefix, a type letter (R full replay, B best lap) and four digits.
pub fn is_replay_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    let letter = GAME_PREFIX.len();
    name.len() == letter + 5
        && name.starts_with(GAME_PREFIX)
        && matches!(bytes[letter], b'R' | b'B')
        && bytes[letter + 1..].iter().all(u8::is_ascii_digit)
}

impl DirEntry {
    pub fn set_name(&mut self, name: &str) -> Result<(), CardError> {
        validate_name(name)?;
        self.raw[0x40..0x60].fill(0);
        self.raw[0x40..0x40 + name.len()].copy_from_slice(name.as_bytes());
        Ok(())
    }
}

/// True when two lists of items have the same names, modes, lengths, timestamps and file
/// bytes in the same order. Cluster numbers (layout) are ignored.
pub fn same_contents(a: &[Node], b: &[Node]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| {
            x.entry.name() == y.entry.name()
                && x.entry.mode() == y.entry.mode()
                // Lengths are derived: a file's is its data, a folder's is its entry count.
                && x.entry.created() == y.entry.created()
                && x.entry.modified() == y.entry.modified()
                && x.data == y.data
                && same_contents(&x.children, &y.children)
        })
}
