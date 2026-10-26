// Defenses against damaged and hostile card images: deep trees, loops in chains and directory
// references, huge lengths, unformatted images and cards of sizes other than 8 MB.

use gt4saves::card::{same_contents, CardError, Node};
use gt4saves::{Card, Library};
use std::fs;
use std::path::PathBuf;

mod common;
use common::*;

fn scratch(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn names(nodes: &[Node]) -> Vec<String> {
    nodes.iter().map(Node::name).collect()
}

fn expect_corrupt<T: std::fmt::Debug>(result: Result<T, CardError>) {
    match result {
        Err(CardError::Corrupt(_)) => {}
        other => panic!("expected a corrupt-card error, got {other:?}"),
    }
}

#[test]
fn a_card_with_saves_of_several_games_lists_each_one() {
    let items = vec![
        game_save(1),
        replay(1),
        dir("SLUS-20000GAMEDATA", vec![file("icon.sys", pattern(964, 9))]),
        dir("SCUS-97436GAMEDATA", vec![file("icon.sys", pattern(964, 8))]),
    ];
    let image = synth(8192, 41, 8135, &items);
    let card = Card::from_image(&image).unwrap();
    assert_eq!(
        names(&card.root().unwrap()),
        ["BASCUS-97328GAMEDATA", "BASCUS-97328R0001", "SLUS-20000GAMEDATA", "SCUS-97436GAMEDATA"]
    );
    // Import keeps the Gran Turismo 4 items and names the other games' saves as skipped.
    let library = Library::open(&scratch("several-games")).unwrap();
    let report = library.import_card_image(&image).unwrap();
    assert_eq!(report.added, vec!["Save 01".to_string(), "BASCUS-97328R0001".to_string()]);
    assert_eq!(report.skipped.len(), 2, "{:?}", report.skipped);
}

#[test]
fn a_directory_that_spans_many_clusters_reads_every_entry() {
    let files: Vec<Node> = (0..60).map(|i| file(&format!("file{i:02}"), pattern(i * 37 % 3000, i as u8))).collect();
    let items = vec![dir("BASCUS-97328GAMEDATA", files.clone())];
    let image = synth(8192, 41, 8135, &items);
    let card = Card::from_image(&image).unwrap();
    let root = card.root().unwrap();
    assert!(card.chain(root[0].entry.cluster()).unwrap().len() > 2, "the directory spans several clusters");
    assert_eq!(root[0].children.len(), 60);
    for (got, want) in root[0].children.iter().zip(&files) {
        assert_eq!(got.name(), want.name());
        assert!(got.data == want.data, "data of {}", want.name());
    }
}

#[test]
fn sixteen_and_sixty_four_megabyte_cards_read_back_in_every_form() {
    let items = vec![game_save(3), replay(2)];
    // (clusters, alloc_offset, alloc_end): the allocation area ends 57 clusters before the card's end, as on 8 MB.
    for (clusters, alloc_offset, alloc_end) in [(16384u32, 80u32, 16327u32), (65536, 300, 65479)] {
        let image = synth(clusters, alloc_offset, alloc_end, &items);
        let card = Card::from_image(&image).unwrap_or_else(|e| panic!("{clusters} clusters: {e}"));
        assert_eq!(card.layout().clusters, clusters);
        assert!(same_contents(&card.root().unwrap(), &items), "{clusters} clusters, data-only form");

        let with_spare = card.to_image();
        assert_eq!(with_spare.len(), clusters as usize * 2 * 528);
        let from_spare = Card::from_image(&with_spare).unwrap();
        assert_eq!(from_spare.layout().clusters, clusters);
        assert!(same_contents(&from_spare.root().unwrap(), &items), "{clusters} clusters, with-spare form");
        assert!(from_spare.ecc_ok());

        let raw = from_spare.to_image_without_ecc();
        assert_eq!(raw, image);
    }
}

#[test]
fn a_superblock_with_another_alloc_offset_is_followed() {
    let items = vec![game_save(4), replay(3)];
    let image = synth(8192, 60, 8135, &items);
    let card = Card::from_image(&image).unwrap();
    assert_eq!(card.layout().alloc_offset, 60);
    assert!(same_contents(&card.root().unwrap(), &items));
}

#[test]
fn a_directory_that_points_back_to_an_ancestor_is_corrupt_not_a_crash() {
    let items = vec![dir("BASCUS-97328GAMEDATA", vec![file("icon.sys", pattern(964, 1))])];
    let base = synth(8192, 41, 8135, &items);
    let mut image = base.clone();
    // The game directory is root entry 2 (after "." and ".."): its cluster becomes the root, its count 4.
    let at = root_byte(&base, 2 * 512);
    put_u32(&mut image, at + 0x10, 0);
    put_u32(&mut image, at + 4, 4);
    expect_corrupt(Card::from_image(&image).unwrap().root());
}

#[test]
fn directories_nested_past_the_depth_limit_are_refused() {
    fn nest(levels: usize) -> Vec<Node> {
        let mut node = file("leaf", pattern(10, 1));
        for level in 0..levels {
            node = dir(&format!("D{level}"), vec![node]);
        }
        vec![node]
    }
    // Eight levels of directories are read; a ninth is refused.
    let eight = Card::from_image(&synth(8192, 41, 8135, &nest(8))).unwrap();
    assert!(eight.root().is_ok());
    let nine = Card::from_image(&synth(8192, 41, 8135, &nest(9))).unwrap();
    expect_corrupt(nine.root());
}

#[test]
fn a_huge_or_too_long_length_is_refused_before_anything_is_allocated() {
    let items = vec![file("F", pattern(10, 1))]; // root entry 2
    let base = synth(8192, 41, 8135, &items);
    // A file length of 4 GB.
    let mut image = base.clone();
    put_u32(&mut image, root_byte(&base, 2 * 512) + 4, u32::MAX);
    expect_corrupt(Card::from_image(&image).unwrap().root());
    // A file length longer than its one-cluster chain.
    let mut image = base.clone();
    put_u32(&mut image, root_byte(&base, 2 * 512) + 4, 5000);
    expect_corrupt(Card::from_image(&image).unwrap().root());
    // A directory entry count claiming more entries than fit the card.
    let mut image = base.clone();
    put_u32(&mut image, root_byte(&base, 2 * 512) + 4, 100_000);
    expect_corrupt(Card::from_image(&image).unwrap().root());
}

#[test]
fn a_fat_chain_that_loops_is_corrupt() {
    let base = synth(8192, 41, 8135, &[file("F", pattern(3000, 2))]);
    let card = Card::from_image(&base).unwrap();
    let root = card.root().unwrap();
    let first = root[0].entry.cluster();
    let chain = card.chain(first).unwrap();
    assert_eq!(chain.len(), 3);
    let mut image = base.clone();
    // The last cluster of the chain now points back to the first.
    put_u32(&mut image, fat_entry_byte(chain[2]), 0x8000_0000 | chain[0]);
    expect_corrupt(Card::from_image(&image).unwrap().root());
}

#[test]
fn an_unformatted_card_is_not_a_card() {
    for length in [8_650_752usize, 8_388_608] {
        match Card::from_image(&vec![0xFFu8; length]) {
            Err(CardError::NotACard) => {}
            other => panic!("{length} bytes of 0xFF: {other:?}"),
        }
    }
}

#[test]
fn wrong_sizes_and_damaged_superblocks_get_clear_errors() {
    assert_eq!(Card::from_image(&vec![0u8; 1000]).unwrap_err(), CardError::BadSize(1000));

    let four_mb = synth(4096, 41, 4039, &[]);
    assert_eq!(four_mb.len(), 4_194_304);
    assert!(matches!(Card::from_image(&four_mb), Err(CardError::Unsupported(_))));

    let base = synth(8192, 41, 8135, &[game_save(5)]);
    let mut odd = base.clone();
    odd.push(0);
    assert!(matches!(Card::from_image(&odd), Err(CardError::Unsupported(_))));

    let mut mismatch = base.clone();
    put_u32(&mut mismatch, 0x30, 16384); // superblock claims 16 MB on an 8 MB image
    expect_corrupt(Card::from_image(&mismatch));

    let mut bad_indirect = base.clone();
    put_u32(&mut bad_indirect, 0x50, 9000); // indirect list cluster past the card
    expect_corrupt(Card::from_image(&bad_indirect));

    let mut bad_fat = base;
    put_u32(&mut bad_fat, 8 * 1024, 9000); // FAT cluster 0 past the card
    expect_corrupt(Card::from_image(&bad_fat));
}

#[test]
fn chain_entry_at_high_cluster_7937_is_accepted_and_read() {
    // Regression test for Issue #86:
    // PCSX2 memory cards with saves allocated at high clusters (such as cluster 7937)
    // failed with "chain entry 7937 outside the allocation area" when usable clusters
    // were computed as alloc_end - alloc_offset (e.g. 7978 - 41 = 7937).
    let items = vec![file("HIGH_CLUSTER_FILE", pattern(500, 42))];
    let base = synth(8192, 41, 7978, &items);
    let mut image = base.clone();

    let at = root_byte(&base, 2 * 512);
    let orig_cluster = u32::from_le_bytes(image[at + 0x10..at + 0x14].try_into().unwrap());

    // Copy cluster data from orig_cluster to physical cluster 41 + 7937 = 7978.
    let orig_physical = (41 + orig_cluster as usize) * CLUSTER;
    let new_physical = (41 + 7937usize) * CLUSTER;
    let chunk = image[orig_physical..orig_physical + CLUSTER].to_vec();
    image[new_physical..new_physical + CLUSTER].copy_from_slice(&chunk);

    // Update root entry 2 cluster to 7937.
    put_u32(&mut image, at + 0x10, 7937);

    // Update FAT entry for 7937 to END.
    put_u32(&mut image, fat_entry_byte(7937), END);
    // Free the old cluster in FAT.
    put_u32(&mut image, fat_entry_byte(orig_cluster), FREE);

    // Verify the card opens and reads the file at cluster 7937 cleanly.
    let card = Card::from_image(&image).expect("card with cluster 7937 opens");
    let root = card.root().expect("root directory reads without error");
    assert_eq!(root.len(), 1);
    assert_eq!(root[0].name(), "HIGH_CLUSTER_FILE");
    assert_eq!(root[0].data, pattern(500, 42));

    // A cluster actually outside the card's physical flash capacity (>= 8151) is still refused.
    let mut out_of_bounds = image;
    put_u32(&mut out_of_bounds, at + 0x10, 8151);
    expect_corrupt(Card::from_image(&out_of_bounds).unwrap().root());
}
