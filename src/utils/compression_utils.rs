
pub(crate) const ENCODING_GZIP: &str = "gzip";
pub(crate) const ENCODING_DEFLATE: &str = "deflate";

pub(crate) fn is_gzip(bytes: &[u8]) -> bool {
    // Gzip files start with the bytes 0x1F 0x8B
    bytes.len() >= 2 && bytes[0] == 0x1F && bytes[1] == 0x8B
}

pub(crate) fn is_deflate(bytes: &[u8]) -> bool {
    bytes[0] == 0x78 && (bytes[1] == 0x01 || bytes[1] == 0x9C || bytes[1] == 0xDA)
}