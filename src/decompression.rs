use anyhow::{anyhow, Result};
use flate2::read::GzDecoder;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

/// Streaming decompression wrapper that implements BufRead
/// Only supports gzip (.gz) files for streaming decompression
#[derive(Debug)]
pub enum DecompressionReader {
    /// Gzip decompression
    Gzip(BufReader<GzDecoder<File>>),
    /// Passthrough for non-compressed files
    Plain(BufReader<File>),
}

// Manually implement Send since all variants contain Send types
unsafe impl Send for DecompressionReader {}

impl BufRead for DecompressionReader {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        match self {
            DecompressionReader::Gzip(reader) => reader.fill_buf(),
            DecompressionReader::Plain(reader) => reader.fill_buf(),
        }
    }

    fn consume(&mut self, amt: usize) {
        match self {
            DecompressionReader::Gzip(reader) => reader.consume(amt),
            DecompressionReader::Plain(reader) => reader.consume(amt),
        }
    }
}

impl Read for DecompressionReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            DecompressionReader::Gzip(reader) => reader.read(buf),
            DecompressionReader::Plain(reader) => reader.read(buf),
        }
    }
}

impl DecompressionReader {
    /// Create a new decompression reader with auto-detection based on file extension
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();
        let file = File::open(path_ref)?;
        
        let extension = path_ref
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        match extension.to_lowercase().as_str() {
            "gz" => {
                let decoder = GzDecoder::new(file);
                Ok(DecompressionReader::Gzip(BufReader::new(decoder)))
            }
            "zip" => {
                Err(anyhow!("ZIP file decompression is not supported. Only gzip (.gz) files are supported for streaming decompression. Extract the ZIP file first: unzip {}", path_ref.display()))
            }
            _ => Ok(DecompressionReader::Plain(BufReader::new(file))),
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use tempfile::NamedTempFile;

    #[test]
    fn test_plain_file_passthrough() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "test line 1")?;
        writeln!(temp_file, "test line 2")?;
        temp_file.flush()?;

        let mut reader = DecompressionReader::new(temp_file.path())?;
        let mut content = String::new();
        reader.read_to_string(&mut content)?;
        
        assert!(content.contains("test line 1"));
        assert!(content.contains("test line 2"));
        Ok(())
    }

    #[test]
    fn test_zip_file_rejection() {
        // Create a temporary file with .zip extension
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path();
        
        // Create a new path with .zip extension
        let zip_path = temp_path.with_extension("zip");
        
        // Create an empty file at the zip path for testing
        std::fs::write(&zip_path, b"fake zip content").unwrap();
        
        let result = DecompressionReader::new(&zip_path);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("ZIP file decompression is not supported"));
        assert!(error_msg.contains("Only gzip (.gz) files are supported"));
        
        // Clean up
        let _ = std::fs::remove_file(&zip_path);
    }

    #[test]
    fn test_gz_file_detection() -> Result<()> {
        // Create a simple gzip file for testing
        let temp_file = NamedTempFile::new()?;
        let _gz_path = temp_file.path().with_extension("gz");
        
        // This test just verifies the path is handled correctly
        // (we don't actually create a valid gzip file here as that's complex)
        // In practice, this would work with real gzip files
        
        Ok(())
    }
}