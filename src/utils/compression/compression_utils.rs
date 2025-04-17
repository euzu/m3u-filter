use std::io::{Read, Write};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

pub const fn is_gzip(bytes: &[u8]) -> bool {
    // Gzip files start with the bytes 0x1F 0x8B
    bytes.len() >= 2 && bytes[0] == 0x1F && bytes[1] == 0x8B
}

pub const fn is_deflate(bytes: &[u8]) -> bool {
    bytes[0] == 0x78 && (bytes[1] == 0x01 || bytes[1] == 0x9C || bytes[1] == 0xDA)
}

pub fn compress_string(input: &str) -> std::io::Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(input.as_bytes())?;
    encoder.finish()
}

pub fn decompress_string(input: &[u8]) -> std::io::Result<String> {
    let mut decoder = GzDecoder::new(input);
    let mut decompressed = String::new();
    decoder.read_to_string(&mut decompressed)?;
    Ok(decompressed)
}
