use std::fs::File;
use std::io::{self, BufReader, ErrorKind, Read};
use std::path::{PathBuf};

pub struct MultiFileReader {
    files: Vec<File>,
    current_reader: Option<BufReader<File>>,
}

impl MultiFileReader {
    pub fn new(paths: &Vec<PathBuf>) -> io::Result<Self> {
        let mut files = Vec::new();
        for path in paths {
            match File::open(path) {
                Ok(file) => { files.push(file); }
                Err(err) => {
                    return Err(io::Error::new(ErrorKind::NotFound,format!("Could not find file {} {}", path.to_str().unwrap_or("?"), err)));
                }
            }
        }
        files.reverse();
        Ok(Self {
            files,
            current_reader: None,
        })
    }
}

impl Read for MultiFileReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.current_reader.is_none() {
                if self.files.is_empty() {
                    return Ok(0);
                }
                self.current_reader = Some(BufReader::new(self.files.pop().unwrap()));
            }
            let reader = self.current_reader.as_mut().unwrap();
            match reader.read(buf) {
                Ok(0) => {
                    // The current reader is exhausted, move to the next one
                    self.current_reader = None;
                }
                Ok(n) => return Ok(n),
                Err(e) => return Err(e),
            }
        }
    }
}