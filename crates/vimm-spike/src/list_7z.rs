use std::fs::File;
use std::path::PathBuf;

fn main() {
    let path = std::env::args().nth(1).expect("usage: list-7z <path>");
    let file = File::open(&path).expect("open file");
    let out = PathBuf::from("/tmp/7z_list_dummy");
    let mut entries = Vec::new();
    sevenz_rust2::decompress_with_extract_fn(
        file,
        &out,
        |entry: &sevenz_rust2::SevenZArchiveEntry,
         _reader: &mut dyn std::io::Read,
         _dest: &std::path::PathBuf| {
            entries.push((entry.name.clone(), entry.size, entry.is_directory));
            Ok(false)
        },
    )
    .expect("decompress");
    println!("Entries ({} total):", entries.len());
    for (name, size, is_dir) in &entries {
        println!("  {}  {} bytes  (dir={})", name, size, is_dir);
    }
}
