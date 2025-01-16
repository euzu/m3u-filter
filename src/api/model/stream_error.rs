#[derive(Debug)]
pub enum StreamError {
    Reqwest(reqwest::Error),
    // StdIo(std::io::Error),
}

// impl StreamError {
//     pub(crate) fn std_io(msg: String) -> Self {
//       StreamError::StdIo(std::io::Error::new(std::io::ErrorKind::Other, msg))
//     }
// }

impl std::error::Error for StreamError {}

impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamError::Reqwest(e) => write!(f, "Reqwest error: {e}"),
            // StreamError::StdIo(e) => write!(f, "IO error: {e}"),
        }
    }
}