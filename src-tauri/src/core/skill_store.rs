//! Grok CLI skills manager.
//!
//! Skills live under `~/.grok/skills/<name>/SKILL.md` (user scope). Grok also
//! discovers `~/.claude/skills` and project-local `.grok/skills`; this module
//! manages the user scope and can import from CC Switch / Claude directories.

use crate::core::paths::{atomic_write, Paths};
use crate::core::AppError;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Source location of a discovered skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkillScope {
    /// `~/.grok/skills` — managed by Grok Switch (editable).
    Grok,
    /// `~/.claude/skills` — Claude Code compat (importable).
    Claude,
    /// `~/.cc-switch/skills` — CC Switch library (importable).
    CcSwitch,
}

/// One skill package discovered on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInfo {
    /// Directory / skill name (slug).
    pub name: String,
    /// Frontmatter description (may be empty if unreadable).
    pub description: String,
    /// Absolute path to the skill directory.
    pub path: String,
    /// Absolute path to SKILL.md.
    pub skill_md_path: String,
    pub scope: SkillScope,
    /// True when the skill dir is a symlink/junction (don't delete target lightly).
    pub is_symlink: bool,
    /// Resolved target if symlink.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_target: Option<String>,
    /// Whether SKILL.md exists and was parseable enough to list.
    pub has_skill_md: bool,
    /// True when this skill lives under `~/.grok/skills` and is not a symlink
    /// — safe for in-place edit / delete via Grok Switch.
    pub editable: bool,
}

/// Full skill payload for the editor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillDetail {
    pub info: SkillInfo,
    /// Raw SKILL.md contents.
    pub content: String,
}

/// Draft used to create or overwrite a skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillDraft {
    pub name: String,
    pub description: String,
    /// Full markdown body (may include frontmatter; we normalize on write).
    pub content: String,
}

/// Validate skill directory name: lowercase alnum + hyphen, 2–64 chars.
pub fn validate_skill_name(name: &str) -> Result<String, AppError> {
    let n = name.trim();
    if n.len() < 2 || n.len() > 64 {
        return Err(AppError::Invalid(
            "skill name must be 2–64 characters".into(),
        ));
    }
    let bytes = n.as_bytes();
    if !bytes[0].is_ascii_alphanumeric() || !bytes[n.len() - 1].is_ascii_alphanumeric() {
        return Err(AppError::Invalid(
            "skill name must start and end with a letter or digit".into(),
        ));
    }
    if !n
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(AppError::Invalid(
            "skill name may only contain a-z, 0-9, and hyphens".into(),
        ));
    }
    if n.contains("--") {
        return Err(AppError::Invalid(
            "skill name must not contain consecutive hyphens".into(),
        ));
    }
    Ok(n.to_string())
}

/// List skills from Grok + Claude + CC Switch scopes (dedup by name, Grok wins).
pub fn list_skills(paths: &Paths) -> Result<Vec<SkillInfo>, AppError> {
    let mut out: Vec<SkillInfo> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Priority: grok > claude > cc-switch (for display of same name).
    for (dir, scope) in [
        (&paths.grok_skills_dir, SkillScope::Grok),
        (&paths.claude_skills_dir, SkillScope::Claude),
        (&paths.ccswitch_skills_dir, SkillScope::CcSwitch),
    ] {
        for info in scan_skills_dir(dir, scope)? {
            if seen.insert(info.name.clone()) {
                out.push(info);
            }
        }
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Read one skill by name. Prefers Grok scope, then Claude, then CC Switch.
pub fn get_skill(paths: &Paths, name: &str) -> Result<SkillDetail, AppError> {
    let name = validate_skill_name(name)?;
    let candidates = [
        (paths.skill_dir(&name), SkillScope::Grok),
        (
            paths.claude_skills_dir.join(&name),
            SkillScope::Claude,
        ),
        (
            paths.ccswitch_skills_dir.join(&name),
            SkillScope::CcSwitch,
        ),
    ];
    for (dir, scope) in candidates {
        if dir.exists() {
            let info = skill_info_from_dir(&dir, scope)?;
            let content = if info.has_skill_md {
                fs::read_to_string(&info.skill_md_path)?
            } else {
                String::new()
            };
            return Ok(SkillDetail { info, content });
        }
    }
    Err(AppError::NotFound(format!("skill not found: {name}")))
}

/// Create or overwrite a skill under `~/.grok/skills/<name>/`.
///
/// Refuses to overwrite a symlink target in place (would edit foreign tree);
/// instead errors so the UI can ask the user to detach first.
pub fn upsert_skill(paths: &Paths, draft: &SkillDraft) -> Result<SkillDetail, AppError> {
    let _guard = crate::core::lock_store();
    let name = validate_skill_name(&draft.name)?;
    let dir = paths.skill_dir(&name);

    if dir.exists() {
        let meta = fs::symlink_metadata(&dir)?;
        if meta.file_type().is_symlink() {
            return Err(AppError::Invalid(format!(
                "skill '{name}' is a symlink/junction — remove the link first or edit the source"
            )));
        }
    } else {
        fs::create_dir_all(&dir)?;
    }

    let body = normalize_skill_md(&name, &draft.description, &draft.content);
    atomic_write(&paths.skill_md(&name), body.as_bytes())?;

    // Re-read for response.
    drop(_guard);
    get_skill(paths, &name)
}

/// Delete a Grok-scoped skill. Symlinks are unlinked (not recursive into target).
/// Real directories are backed up under `~/.grok-switch/skill-backups/` first.
pub fn delete_skill(paths: &Paths, name: &str) -> Result<bool, AppError> {
    let _guard = crate::core::lock_store();
    let name = validate_skill_name(name)?;
    let dir = paths.skill_dir(&name);
    if !dir.exists() {
        return Ok(false);
    }

    let meta = fs::symlink_metadata(&dir)?;
    if meta.file_type().is_symlink() {
        fs::remove_file(&dir).or_else(|_| fs::remove_dir(&dir))?;
        return Ok(true);
    }

    // Backup then remove.
    paths.ensure_app_dirs()?;
    let stamp = Local::now().format("%Y%m%d_%H%M%S");
    let backup = paths
        .skill_backups_dir
        .join(format!("{stamp}_{name}"));
    copy_dir_recursive(&dir, &backup)?;
    fs::remove_dir_all(&dir)?;
    Ok(true)
}

/// Import selected skills from CC Switch (or Claude) into `~/.grok/skills`.
///
/// `names` empty = import all from CC Switch that are not already present as
/// non-symlink Grok skills. Existing Grok skills are skipped (no overwrite).
pub fn import_skills(
    paths: &Paths,
    names: &[String],
    source: SkillScope,
) -> Result<Vec<SkillInfo>, AppError> {
    let _guard = crate::core::lock_store();
    let src_root = match source {
        SkillScope::CcSwitch => &paths.ccswitch_skills_dir,
        SkillScope::Claude => &paths.claude_skills_dir,
        SkillScope::Grok => {
            return Err(AppError::Invalid(
                "cannot import from grok scope into itself".into(),
            ));
        }
    };
    if !src_root.is_dir() {
        return Err(AppError::NotFound(format!(
            "source skills dir not found: {}",
            src_root.display()
        )));
    }

    let wanted: Option<std::collections::HashSet<String>> = if names.is_empty() {
        None
    } else {
        let mut set = std::collections::HashSet::new();
        for n in names {
            set.insert(validate_skill_name(n)?);
        }
        Some(set)
    };

    let mut imported = Vec::new();
    for entry in fs::read_dir(src_root)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        if !ft.is_dir() && !ft.is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if validate_skill_name(&name).is_err() {
            continue;
        }
        if let Some(ref w) = wanted {
            if !w.contains(&name) {
                continue;
            }
        }
        let dest = paths.skill_dir(&name);
        if dest.exists() {
            // Skip existing (do not clobber user edits / symlinks).
            continue;
        }
        let src = entry.path();
        // Only import packages that have SKILL.md.
        if !src.join("SKILL.md").is_file() {
            continue;
        }
        fs::create_dir_all(&paths.grok_skills_dir)?;
        copy_dir_recursive(&src, &dest)?;
        if let Ok(info) = skill_info_from_dir(&dest, SkillScope::Grok) {
            imported.push(info);
        }
    }
    imported.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(imported)
}

// ─── internals ───────────────────────────────────────────────────────────────

fn scan_skills_dir(dir: &Path, scope: SkillScope) -> Result<Vec<SkillInfo>, AppError> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        if !ft.is_dir() && !ft.is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if validate_skill_name(&name).is_err() {
            continue;
        }
        match skill_info_from_dir(&entry.path(), scope) {
            Ok(info) => out.push(info),
            Err(_) => continue,
        }
    }
    Ok(out)
}

fn skill_info_from_dir(dir: &Path, scope: SkillScope) -> Result<SkillInfo, AppError> {
    let name = dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let meta = fs::symlink_metadata(dir)?;
    let is_symlink = meta.file_type().is_symlink();
    let link_target = if is_symlink {
        fs::read_link(dir)
            .ok()
            .map(|p| p.to_string_lossy().into_owned())
    } else {
        None
    };
    let skill_md = dir.join("SKILL.md");
    let has_skill_md = skill_md.is_file();
    let description = if has_skill_md {
        parse_description(&fs::read_to_string(&skill_md).unwrap_or_default())
    } else {
        String::new()
    };
    let editable = scope == SkillScope::Grok && !is_symlink;
    Ok(SkillInfo {
        name,
        description,
        path: dir.to_string_lossy().into_owned(),
        skill_md_path: skill_md.to_string_lossy().into_owned(),
        scope,
        is_symlink,
        link_target,
        has_skill_md,
        editable,
    })
}

/// Extract `description` from YAML frontmatter (simple line parser, no full YAML).
fn parse_description(content: &str) -> String {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return String::new();
    }
    let rest = &trimmed[3..];
    let end = match rest.find("\n---") {
        Some(i) => i,
        None => return String::new(),
    };
    let fm = &rest[..end];
    // Support single-line and folded `description: >` blocks (first paragraph).
    let mut lines = fm.lines().peekable();
    while let Some(line) = lines.next() {
        let line = line.trim_end();
        if let Some(val) = line.strip_prefix("description:") {
            let val = val.trim();
            if val.is_empty() || val == ">" || val == "|" {
                // Multi-line: collect indented lines.
                let mut parts = Vec::new();
                while let Some(next) = lines.peek() {
                    if next.starts_with(' ') || next.starts_with('\t') {
                        parts.push(next.trim().to_string());
                        lines.next();
                    } else if next.trim().is_empty() {
                        lines.next();
                        break;
                    } else {
                        break;
                    }
                }
                return parts.join(" ").trim().to_string();
            }
            // Strip surrounding quotes if present.
            let v = val.trim_matches('"').trim_matches('\'').trim();
            return v.to_string();
        }
    }
    String::new()
}

/// Ensure SKILL.md has correct frontmatter; preserve body.
fn normalize_skill_md(name: &str, description: &str, content: &str) -> String {
    let desc = description.trim();
    let body = strip_frontmatter(content).trim().to_string();
    let desc_line = if desc.contains('\n') || desc.len() > 80 {
        // Use folded block for long descriptions.
        let folded = desc
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n  ");
        format!("description: >\n  {folded}")
    } else {
        format!("description: {desc}")
    };
    format!("---\nname: {name}\n{desc_line}\n---\n\n{body}\n")
}

fn strip_frontmatter(content: &str) -> String {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return content.to_string();
    }
    let rest = &trimmed[3..];
    if let Some(idx) = rest.find("\n---") {
        let after = &rest[idx + 4..];
        return after.trim_start_matches('\r').trim_start_matches('\n').to_string();
    }
    content.to_string()
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), AppError> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let to = dest.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &to)?;
        } else if ty.is_symlink() {
            // Copy the link as a real file/dir by following it once.
            let target = fs::read_link(entry.path())?;
            let resolved = if target.is_absolute() {
                target
            } else {
                entry.path().parent().unwrap_or(src).join(target)
            };
            if resolved.is_dir() {
                copy_dir_recursive(&resolved, &to)?;
            } else if resolved.is_file() {
                fs::copy(&resolved, &to)?;
            }
        } else {
            fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_root(dir.path());
        paths.ensure_app_dirs().unwrap();
        fs::create_dir_all(&paths.grok_skills_dir).unwrap();
        (dir, paths)
    }

    fn write_skill(dir: &Path, name: &str, desc: &str, body: &str) {
        let sdir = dir.join(name);
        fs::create_dir_all(&sdir).unwrap();
        fs::write(
            sdir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {desc}\n---\n\n{body}\n"),
        )
        .unwrap();
    }

    #[test]
    fn validates_skill_names() {
        assert!(validate_skill_name("ab").is_ok());
        assert!(validate_skill_name("my-skill-1").is_ok());
        assert!(validate_skill_name("a").is_err());
        assert!(validate_skill_name("-ab").is_err());
        assert!(validate_skill_name("ab-").is_err());
        assert!(validate_skill_name("AB").is_err());
        assert!(validate_skill_name("a b").is_err());
        assert!(validate_skill_name("a--b").is_err());
        assert!(validate_skill_name("../x").is_err());
    }

    #[test]
    fn list_and_get_roundtrip() {
        let (_tmp, paths) = setup();
        write_skill(&paths.grok_skills_dir, "demo-skill", "Does demo things", "# Demo");

        let list = list_skills(&paths).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "demo-skill");
        assert!(list[0].description.contains("demo"));
        assert!(list[0].editable);
        assert!(!list[0].is_symlink);

        let detail = get_skill(&paths, "demo-skill").unwrap();
        assert!(detail.content.contains("# Demo"));
        assert_eq!(detail.info.scope, SkillScope::Grok);
    }

    #[test]
    fn upsert_creates_and_updates() {
        let (_tmp, paths) = setup();
        let draft = SkillDraft {
            name: "new-one".into(),
            description: "A brand new skill".into(),
            content: "# Hello\n\nDo the thing.".into(),
        };
        let d = upsert_skill(&paths, &draft).unwrap();
        assert_eq!(d.info.name, "new-one");
        assert!(paths.skill_md("new-one").is_file());
        assert!(d.content.contains("name: new-one"));
        assert!(d.content.contains("A brand new skill"));
        assert!(d.content.contains("Do the thing"));

        let draft2 = SkillDraft {
            name: "new-one".into(),
            description: "Updated desc".into(),
            content: d.content,
        };
        let d2 = upsert_skill(&paths, &draft2).unwrap();
        assert!(d2.content.contains("Updated desc"));
    }

    #[test]
    fn delete_backs_up_and_removes() {
        let (_tmp, paths) = setup();
        write_skill(&paths.grok_skills_dir, "todelete", "x", "body");
        assert!(delete_skill(&paths, "todelete").unwrap());
        assert!(!paths.skill_dir("todelete").exists());
        // Backup exists
        let backups: Vec<_> = fs::read_dir(&paths.skill_backups_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(backups.len(), 1);
        assert!(backups[0].file_name().to_string_lossy().contains("todelete"));
        assert!(!delete_skill(&paths, "todelete").unwrap());
    }

    #[test]
    fn import_from_ccswitch_skips_existing() {
        let (_tmp, paths) = setup();
        fs::create_dir_all(&paths.ccswitch_skills_dir).unwrap();
        write_skill(&paths.ccswitch_skills_dir, "from-cc", "imported", "body");
        write_skill(&paths.grok_skills_dir, "already", "keep", "old");
        write_skill(&paths.ccswitch_skills_dir, "already", "should-skip", "new");

        let imported = import_skills(&paths, &[], SkillScope::CcSwitch).unwrap();
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].name, "from-cc");
        assert!(paths.skill_dir("from-cc").is_dir());

        // Existing unchanged
        let existing = get_skill(&paths, "already").unwrap();
        assert!(existing.content.contains("old") || existing.content.contains("keep"));
    }

    #[test]
    fn parse_description_variants() {
        assert_eq!(
            parse_description("---\nname: x\ndescription: hello world\n---\n\nbody"),
            "hello world"
        );
        assert_eq!(
            parse_description("---\nname: x\ndescription: >\n  line one\n  line two\n---\n"),
            "line one line two"
        );
        assert_eq!(parse_description("no frontmatter"), "");
    }

    #[test]
    fn path_traversal_rejected_by_name() {
        assert!(validate_skill_name("..").is_err());
        assert!(validate_skill_name("../etc").is_err());
        assert!(validate_skill_name("a/b").is_err());
        assert!(validate_skill_name("a\\b").is_err());
    }
}
