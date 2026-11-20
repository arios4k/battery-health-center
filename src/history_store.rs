//! Coordinated, validated local-history backup and replacement. No root settings are included.
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};

pub const FILES: [&str; 3] = ["history.csv", "app-history.csv", "health.csv"];
const MAX_BYTES: usize = 64 * 1024 * 1024;
const HEADER: &str = "BHC-HISTORY-BACKUP 1\n";
pub type Backup = BTreeMap<String, String>;

fn number(s: &str, min: f64, max: f64) -> bool {
    s.parse::<f64>()
        .is_ok_and(|n| n.is_finite() && n >= min && n <= max)
}

pub fn validate(name: &str, text: &str) -> Result<(), String> {
    if text.len() > MAX_BYTES || text.contains('\0') {
        return Err("History is too large or contains invalid text.".into());
    }
    for (index, line) in text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        let f: Vec<_> = if matches!(name, "app-history.csv" | "health.csv") {
            line.splitn(5, ',').collect()
        } else {
            line.split(',').collect()
        };
        let timestamp = f.first().is_some_and(|t| {
            t.parse::<i64>()
                .is_ok_and(|t| t >= 0 && t <= crate::history::now_unix() + 86_400)
        });
        let valid = timestamp
            && match name {
                "history.csv" => {
                    (5..=7).contains(&f.len())
                        && number(f[1], 0.0, 100.0)
                        && matches!(f[2], "c" | "d" | "n" | "f" | "u")
                        && (f[3].is_empty() || number(f[3], 0.0, 100_000.0))
                        && (f[4].is_empty() || number(f[4], 0.0, 100.0))
                        && f.get(5)
                            .is_none_or(|v| v.is_empty() || u32::from_str_radix(v, 16).is_ok())
                        && f.get(6)
                            .is_none_or(|v| v.is_empty() || v.parse::<i64>().is_ok())
                }
                "app-history.csv" => {
                    f.len() == 5
                        && !f[1].is_empty()
                        && f[1].len() <= 512
                        && f[4].len() <= 1024
                        && number(f[2], 0.0, 1e12)
                        && number(f[3], 0.0, 1e12)
                }
                "health.csv" => {
                    matches!(f.len(), 3 | 5)
                        && number(f[1], 0.0, 100.0)
                        && (f[2].is_empty() || f[2].parse::<u32>().is_ok())
                        && (f.len() == 3
                            || ((f[3] == "legacy"
                                || (f[3].starts_with("battery-")
                                    && f[3].len() == 24
                                    && f[3][8..].bytes().all(|b| b.is_ascii_hexdigit())))
                                && f[4].len() <= 1024))
                }
                _ => false,
            };
        if !valid {
            return Err(format!("Invalid record in {name}, line {}.", index + 1));
        }
    }
    Ok(())
}

pub fn encode(data: &Backup) -> String {
    let mut text = HEADER.to_string();
    for name in FILES {
        let content = data.get(name).map(String::as_str).unwrap_or("");
        text.push_str(&format!("{name} {}\n", content.len()));
        text.push_str(content);
        text.push('\n');
    }
    text
}

pub fn decode(text: &str) -> Result<Backup, String> {
    if text.len() > MAX_BYTES {
        return Err("The backup exceeds the 64 MiB limit.".into());
    }
    let mut rest = text
        .strip_prefix(HEADER)
        .ok_or("This is not a supported Battery Health Center history backup.")?;
    let mut data = Backup::new();
    for expected in FILES {
        let (header, body) = rest.split_once('\n').ok_or("Incomplete backup header.")?;
        let (name, size) = header.split_once(' ').ok_or("Invalid backup header.")?;
        if name != expected {
            return Err("Unexpected or duplicate file in backup.".into());
        }
        let size = size
            .parse::<usize>()
            .ok()
            .filter(|n| *n <= MAX_BYTES)
            .ok_or("Invalid backup size.")?;
        let content = body.get(..size).ok_or("Incomplete backup data.")?;
        rest = body
            .get(size..)
            .and_then(|s| s.strip_prefix('\n'))
            .ok_or("Invalid backup boundary.")?;
        validate(name, content)?;
        data.insert(name.to_string(), content.to_string());
    }
    if !rest.is_empty() {
        return Err("Unexpected content after the backup.".into());
    }
    Ok(data)
}

pub fn read_backup(path: &Path) -> Result<Backup, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > MAX_BYTES as u64 {
        return Err("The backup exceeds the 64 MiB limit.".into());
    }
    let mut text = String::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_BYTES as u64 + 1)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    decode(&text)
}

pub struct HistoryStore {
    pub dir: PathBuf,
    _lock: File,
}
impl HistoryStore {
    pub fn open() -> Result<Self, String> {
        Self::at(crate::history::data_dir())
    }
    fn at(dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join("history-operation.lock"))
            .map_err(|e| e.to_string())?;
        file.try_lock()
            .map_err(|_| "History is busy in another copy. Try again in a moment.".to_string())?;
        Ok(Self { dir, _lock: file })
    }
    pub fn revision(&self) -> String {
        fs::read_to_string(self.dir.join("history-generation")).unwrap_or_default()
    }
    pub fn heartbeat(&self, now: i64) -> Result<(), String> {
        crate::storage::replace(
            &self.dir.join("history-writer-protocol"),
            &format!("1 {} {now}", std::process::id()),
        )
    }
    pub fn compatible_writer(&self, now: i64) -> bool {
        let text = fs::read_to_string(self.dir.join("history-writer-protocol")).unwrap_or_default();
        let f: Vec<_> = text.split_whitespace().collect();
        f.len() == 3
            && f[0] == "1"
            && f[1]
                .parse::<u32>()
                .is_ok_and(|pid| PathBuf::from(format!("/proc/{pid}")).exists())
            && f[2]
                .parse::<i64>()
                .is_ok_and(|ts| (0..=15).contains(&(now - ts)))
    }
    pub fn touch(&self) -> Result<(), String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        crate::storage::replace(&self.dir.join("history-generation"), &stamp.to_string())
    }
    pub fn snapshot(&self) -> Result<Backup, String> {
        let mut result = Backup::new();
        let mut size = 0;
        for name in FILES {
            if fs::metadata(self.dir.join(name)).is_ok_and(|m| m.len() > MAX_BYTES as u64) {
                return Err("History exceeds the 64 MiB backup limit.".into());
            }
            let text = match fs::read_to_string(self.dir.join(name)) {
                Ok(text) => text,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(e) => return Err(e.to_string()),
            };
            size += text.len();
            if size > MAX_BYTES {
                return Err("History exceeds the 64 MiB backup limit.".into());
            }
            validate(name, &text)?;
            result.insert(name.into(), text);
        }
        Ok(result)
    }
    pub fn replace(&self, data: &Backup) -> Result<(), String> {
        self.replace_with(data, crate::storage::replace)
    }
    fn replace_with(
        &self,
        data: &Backup,
        mut write: impl FnMut(&Path, &str) -> Result<(), String>,
    ) -> Result<(), String> {
        if data.len() != FILES.len() || FILES.iter().any(|f| !data.contains_key(*f)) {
            return Err("The backup is incomplete.".into());
        }
        for (name, text) in data {
            validate(name, text)?;
        }
        let before = self.snapshot()?;
        let result = (|| {
            for name in FILES {
                write(&self.dir.join(name), &data[name])?;
            }
            self.touch()
        })();
        if let Err(error) = result {
            let mut rollback_failed = false;
            for name in FILES {
                rollback_failed |= write(&self.dir.join(name), &before[name]).is_err();
            }
            if rollback_failed {
                let recovery = self.dir.join("history-recovery.bhc");
                crate::storage::replace(&recovery, &encode(&before))?;
                return Err(format!(
                    "{error}. Recovery data was saved to {}.",
                    recovery.display()
                ));
            }
            return Err(error);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn example() -> Backup {
        [
            ("history.csv".into(), "100,50,d,10,90,,\n".into()),
            (
                "app-history.csv".into(),
                "0,app:foo,10,1,名字, app\n".into(),
            ),
            ("health.csv".into(), "100,90,2\n".into()),
        ]
        .into()
    }
    #[test]
    fn unicode_backup_roundtrip_and_invalid_input() {
        let data = example();
        assert_eq!(decode(&encode(&data)).unwrap(), data);
        assert!(decode(&(encode(&data) + "extra")).is_err());
        assert!(decode(&encode(&data).replace("health.csv", "../../etc/passwd")).is_err());
        assert!(decode(&encode(&data).replace("100,90,2", "100,NaN,2")).is_err());
        assert!(decode("BHC-HISTORY-BACKUP 2\n").is_err());
    }
    #[test]
    fn a_mid_restore_failure_rolls_back_all_datasets() {
        let dir = std::env::temp_dir().join(format!("bhc-restore-rollback-{}", std::process::id()));
        let store = HistoryStore::at(dir.clone()).unwrap();
        let original = example();
        store.replace(&original).unwrap();
        let revision = store.revision();
        let mut restored = original.clone();
        restored.insert("history.csv".into(), String::new());
        let mut failed = false;
        let result = store.replace_with(&restored, |path, text| {
            if path.file_name().unwrap() == "app-history.csv" && !failed {
                failed = true;
                return Err("injected disk error".into());
            }
            crate::storage::replace(path, text)
        });
        assert!(result.is_err());
        assert_eq!(store.snapshot().unwrap(), original);
        assert_eq!(store.revision(), revision);
        drop(store);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn writer_lock_and_replacement_are_coordinated() {
        let dir = std::env::temp_dir().join(format!("bhc-store-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let store = HistoryStore::at(dir.clone()).unwrap();
        assert!(HistoryStore::at(dir.clone()).is_err());
        store.replace(&example()).unwrap();
        assert_eq!(store.snapshot().unwrap(), example());
        assert!(!store.revision().is_empty());
        let before = store.snapshot().unwrap();
        let mut invalid = example();
        invalid.insert("health.csv".into(), "oops".into());
        assert!(store.replace(&invalid).is_err());
        assert_eq!(store.snapshot().unwrap(), before);
        drop(store);
        fs::remove_dir_all(dir).unwrap();
    }
}
