use crate::core::paths::Paths;
use crate::core::types::Activity;
use crate::core::AppError;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};

/// Append one activity row as a JSONL line.
pub fn append_activity(paths: &Paths, activity: &Activity) -> Result<(), AppError> {
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
    Ok(())
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
}
