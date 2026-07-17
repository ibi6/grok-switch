use crate::core::paths::{atomic_write, Paths};
use crate::core::types::Activity;
use crate::core::AppError;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};

/// Rotate the log once it grows past this size, keeping the most recent entries.
const ROTATE_THRESHOLD_BYTES: u64 = 512 * 1024;
const ROTATE_KEEP_LINES: usize = 1_000;

/// Append one activity row as a JSONL line, rotating the file when it grows too
/// large so the log cannot grow without bound (and every read re-parse it).
pub fn append_activity(paths: &Paths, activity: &Activity) -> Result<(), AppError> {
    let _guard = crate::core::lock_store();
    paths.ensure_app_dirs()?;
    if let Some(parent) = paths.activity_jsonl.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.activity_jsonl)?;
    let line = serde_json::to_string(activity)?;
    writeln!(file, "{line}")?;
    drop(file);

    // Cheap size check first; only pay the full rewrite when we cross the limit.
    if let Ok(meta) = fs::metadata(&paths.activity_jsonl) {
        if meta.len() > ROTATE_THRESHOLD_BYTES {
            rotate_activity(paths, ROTATE_KEEP_LINES)?;
        }
    }
    Ok(())
}

/// Rewrite the log keeping only the last `keep` non-empty lines.
fn rotate_activity(paths: &Paths, keep: usize) -> Result<(), AppError> {
    let file = fs::File::open(&paths.activity_jsonl)?;
    let lines: Vec<String> = BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter(|l| !l.trim().is_empty())
        .collect();
    if lines.len() <= keep {
        return Ok(());
    }
    let start = lines.len() - keep;
    let mut kept = lines[start..].join("\n");
    kept.push('\n');
    atomic_write(&paths.activity_jsonl, kept)
}

/// Return the last `limit` activity entries (newest first).
pub fn list_activity(paths: &Paths, limit: usize) -> Result<Vec<Activity>, AppError> {
    if !paths.activity_jsonl.exists() {
        return Ok(Vec::new());
    }
    let file = fs::File::open(&paths.activity_jsonl)?;
    let reader = BufReader::new(file);
    let mut items = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<Activity>(trimmed) {
            Ok(a) => items.push(a),
            Err(_) => continue, // skip corrupt lines
        }
    }
    if limit == 0 {
        return Ok(Vec::new());
    }
    let start = items.len().saturating_sub(limit);
    let mut tail: Vec<Activity> = items.into_iter().skip(start).collect();
    tail.reverse();
    Ok(tail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::ActivityType;

    fn act(ts: i64, msg: &str) -> Activity {
        Activity {
            ts,
            activity_type: ActivityType::SwitchProvider,
            message: msg.into(),
            meta: None,
        }
    }

    #[test]
    fn activity_append_and_list_last_n() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_root(dir.path());
        paths.ensure_app_dirs().unwrap();

        assert!(list_activity(&paths, 10).unwrap().is_empty());

        for i in 1..=5 {
            append_activity(&paths, &act(i, &format!("msg-{i}"))).unwrap();
        }

        let last3 = list_activity(&paths, 3).unwrap();
        assert_eq!(last3.len(), 3);
        assert_eq!(last3[0].message, "msg-5");
        assert_eq!(last3[1].message, "msg-4");
        assert_eq!(last3[2].message, "msg-3");

        let all = list_activity(&paths, 100).unwrap();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0].ts, 5);
        assert_eq!(all[4].ts, 1);
    }

    #[test]
    fn rotate_keeps_last_n_lines() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_root(dir.path());
        paths.ensure_app_dirs().unwrap();

        // 2000 raw JSONL lines, oldest first.
        let mut content = String::new();
        for i in 0..2000 {
            content.push_str(&format!("{{\"ts\":{i},\"type\":\"health\",\"message\":\"m{i}\"}}\n"));
        }
        fs::write(&paths.activity_jsonl, content).unwrap();

        rotate_activity(&paths, 10).unwrap();

        let remaining = fs::read_to_string(&paths.activity_jsonl).unwrap();
        let lines: Vec<&str> = remaining.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(lines.len(), 10);
        assert!(lines[0].contains("\"ts\":1990"));
        assert!(lines[9].contains("\"ts\":1999"));
    }
}
