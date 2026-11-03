//! Fallible local writes with the destination included in user-facing errors.
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

pub fn append(path: &Path, text: &str) -> Result<(), String> {
    let result = (|| -> io::Result<()> {
        if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
        OpenOptions::new().create(true).append(true).open(path)?.write_all(text.as_bytes())
    })();
    result.map_err(|e| format!("{}: {e}", path.display()))
}

pub fn replace(path: &Path, text: &str) -> Result<(), String> {
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let result = (|| -> io::Result<()> {
        if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
        fs::write(&temporary, text)?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() { let _ = fs::remove_file(&temporary); }
    result.map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_write_keeps_existing_file_and_reports_destination() {
        let folder = std::env::temp_dir().join(format!("bhc-storage-{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        let blocker = folder.join("file");
        fs::write(&blocker, "original").unwrap();
        let invalid = blocker.join("settings.ini");
        assert!(replace(&invalid, "new").unwrap_err().contains("settings.ini"));
        assert!(append(&invalid, "new").is_err());
        assert_eq!(fs::read_to_string(&blocker).unwrap(), "original");
        replace(&blocker, "replacement").unwrap();
        assert_eq!(fs::read_to_string(&blocker).unwrap(), "replacement");
        fs::remove_dir_all(folder).unwrap();
    }
}
