use capybara::emulator::my_lab_sd::Card;
use std::fs;

#[test]
fn maps_host_file_as_a_fat32_sector_chain() {
    let dir = std::env::temp_dir().join(format!("capybara-sd-test-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let payload: Vec<_> = (0..20_000).map(|i| (i % 239) as u8).collect();
    fs::write(dir.join("mltupd.bin"), &payload).unwrap();
    let mut card = Card::open(dir.clone()).unwrap();
    fs::remove_dir_all(dir).unwrap();

    let mut b = [0; 512];
    let u16_at = |b: &[u8], p: usize| u16::from_le_bytes(b[p..p + 2].try_into().unwrap());
    let u32_at = |b: &[u8], p: usize| u32::from_le_bytes(b[p..p + 4].try_into().unwrap());
    assert!(card.read(0, &mut b));
    let partition = u32_at(&b, 454);
    assert_eq!(u16_at(&b, 510), 0xaa55);
    assert!(card.read(partition, &mut b));
    assert_eq!(&b[82..90], b"FAT32   ");
    let fat_start = partition + u16_at(&b, 14) as u32;
    let data_start = fat_start + u32_at(&b, 36) * b[16] as u32;
    let cluster_sectors = b[13] as u32;
    let root = data_start + (u32_at(&b, 44) - 2) * cluster_sectors;
    assert!(card.read(root, &mut b));
    assert_eq!(&b[..11], b"MLTUPD  BIN");
    assert_eq!(u32_at(&b, 28) as usize, payload.len());
    let file_cluster = u16_at(&b, 26) as u32;
    let mut readback = vec![0; payload.len().div_ceil(512) * 512];
    assert!(card.read(
        data_start + (file_cluster - 2) * cluster_sectors,
        &mut readback
    ));
    assert_eq!(&readback[..payload.len()], payload);
    assert!(!card.read(card.sector_count(), &mut b));
}
