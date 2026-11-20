//! The model's backups: names, what made them, which to keep, and how to list them for a person.
//!
//! A backup is `Backups/<stem>-<secs>[-<n>][-pre[-<kind>][-a|-p]].sketchcraft`:
//!  - no `-pre`: a timed backup, taken at most every five minutes while working;
//!  - `-pre`: taken just before something destructive (`new`, `open`, a delete, a big shrink, a
//!    plugin run that wiped much of the model, a restore), with `kind` saying which;
//!  - `-a` / `-p`: the destructive step came from an agent / a plugin, not from the person.
//!
//! Each origin has its own quota, so an agent or a plugin that makes many destructive steps can fill
//! only its own few slots and never pushes out the person's backups. Only names in exactly this
//! shape count (anything else in the folder is left alone, never counted and never deleted), and the
//! folder itself is out of reach of agents (see `policy` and `library`).

use std::path::{Path, PathBuf};

/// Who asked for the destructive step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Origin {
    Ui,
    Agent,
    Plugin,
}

impl Origin {
    pub fn of(c: &crate::policy::Caller) -> Origin {
        match c {
            crate::policy::Caller::Ui => Origin::Ui,
            crate::policy::Caller::Plugin => Origin::Plugin,
            crate::policy::Caller::Mcp(_) => Origin::Agent,
        }
    }
    fn tag(self) -> &'static str {
        match self {
            Origin::Ui => "",
            Origin::Agent => "-a",
            Origin::Plugin => "-p",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Origin::Ui => "you",
            Origin::Agent => "an AI agent",
            Origin::Plugin => "a plugin",
        }
    }
}

/// Kinds of destructive step a backup can say it was taken before.
pub const KINDS: [&str; 7] = ["new", "open", "del", "shr", "bat", "big", "rst"];

/// How the person reads a kind.
pub fn kind_label(kind: Option<&str>) -> &'static str {
    match kind {
        Some("new") => "before New",
        Some("open") => "before Open",
        Some("del") => "before a delete",
        Some("shr") => "before a big shrink",
        Some("bat") => "before a plugin run",
        Some("rst") => "before a restore",
        Some("big") => "before a big change",
        _ => "before a big change",
    }
}

/// A backup SketchCraft wrote.
#[derive(Clone, Debug)]
pub struct Backup {
    pub path: PathBuf,
    pub secs: u64,
    pub n: u64,
    pub pre: bool,
    pub kind: Option<String>,
    pub origin: Origin,
    pub size: u64,
}

impl Backup {
    pub fn name(&self) -> String {
        self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }
}

/// Backups whose names say they were made further ahead than this are not ours: ignored, never thinned.
pub const CLOCK_SLACK_SECS: u64 = 120;

pub struct Parsed {
    pub secs: u64,
    pub n: u64,
    pub pre: bool,
    pub kind: Option<String>,
    pub origin: Origin,
}

/// What `name` says about itself if it is one of the backups of the document `stem` (and nothing
/// else: `a-b-170...` is not a backup of `a`).
pub fn parse(stem: &str, name: &str) -> Option<Parsed> {
    let rest = name.strip_prefix(stem)?.strip_prefix('-')?.strip_suffix(".sketchcraft")?;
    let mut parts = rest.split('-').peekable();
    let secs = parts.next().filter(|d| (9..=11).contains(&d.len()) && d.bytes().all(|b| b.is_ascii_digit()))?.parse::<u64>().ok()?;
    let mut n = 0;
    if let Some(d) = parts.peek().copied().filter(|d| !d.is_empty() && d.len() <= 6 && d.bytes().all(|b| b.is_ascii_digit())) {
        n = d.parse().ok()?;
        parts.next();
    }
    let (mut pre, mut kind, mut origin) = (false, None, Origin::Ui);
    if parts.peek() == Some(&"pre") {
        pre = true;
        parts.next();
        if let Some(k) = parts.peek().copied().filter(|k| KINDS.contains(k)) {
            kind = Some(k.to_string());
            parts.next();
        }
        match parts.peek().copied() {
            Some("a") => {
                origin = Origin::Agent;
                parts.next();
            }
            Some("p") => {
                origin = Origin::Plugin;
                parts.next();
            }
            _ => {}
        }
    }
    parts.next().is_none().then_some(Parsed { secs, n, pre, kind, origin })
}

/// The file name for a new backup.
pub fn file_name(stem: &str, secs: u64, n: u64, pre: Option<(Option<&str>, Origin)>) -> String {
    let tail = if n == 0 { String::new() } else { format!("-{n}") };
    match pre {
        None => format!("{stem}-{secs}{tail}.sketchcraft"),
        Some((kind, origin)) => format!("{stem}-{secs}{tail}-pre{}{}.sketchcraft", kind.map(|k| format!("-{k}")).unwrap_or_default(), origin.tag()),
    }
}

/// The backups of `stem` in `dir`, oldest first. Only real files with SketchCraft's own names;
/// a name claiming a time well in the future (a clock that was set back, or a file put there by
/// something else) is ignored, so it can't push out real ones.
pub fn list(dir: &Path, stem: &str, now: u64) -> Vec<Backup> {
    let mut all: Vec<Backup> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let meta = crate::safety::plain_file(&e.path()).filter(|m| m.len() > 0)?;
                    let p = parse(stem, &e.file_name().to_string_lossy())?;
                    (p.secs <= now + CLOCK_SLACK_SECS).then(|| Backup { path: e.path(), secs: p.secs, n: p.n, pre: p.pre, kind: p.kind, origin: p.origin, size: meta.len() })
                })
                .collect()
        })
        .unwrap_or_default();
    all.sort_by(|a, b| (a.secs, a.n, a.pre).cmp(&(b.secs, b.n, b.pre)));
    all
}

/// How many backups and how many bytes one origin's backups may hold, and how long a "pre" one is protected.
struct Quota {
    max: usize,
    bytes: u64,
    /// "pre" backups newer than this are protected (they go only after the plain ones)
    pre_secs: u64,
    /// the byte limit never takes the number of backups below this (the newest and the biggest-last-day excepted from nothing:
    /// a few huge files can't push every good backup out)
    min_keep: usize,
}

const PERSON: Quota = Quota { max: 80, bytes: 2 << 30, pre_secs: 3600, min_keep: 6 };
const OTHER: Quota = Quota { max: 10, bytes: 256 << 20, pre_secs: 7 * 86_400, min_keep: 3 };
/// The newest few are always kept.
const KEEP_NEWEST: usize = 8;
/// What the person's own backups are limited to, for the notice and tests.
pub const MAX_PERSON: usize = PERSON.max;
pub const MAX_OTHER: usize = OTHER.max;

/// Who goes first when there are too many or too big: the lowest tier, oldest first (by bytes: biggest first).
const T_PLAIN: u8 = 0;
const T_PROTECTED: u8 = 1;
/// one survivor per hour for a day and per day for a month: the person's way back, however many deletes there were
const T_SPREAD: u8 = 2;
const T_NEWEST: u8 = 3;

/// Which backups (given in any order) to delete. Each origin is judged on its own, so one origin's
/// flood never costs another its backups.
///
/// The person's: the newest 8; of the older ones the newest of each 30 s (last 10 min), 5 min (last
/// hour), hour (two days) and day; every "pre" backup for an hour; and the biggest of the last day;
/// at most 80 files and 2 GB. When there are too many, the plain ones go first, then the "pre" ones,
/// and the hourly (last day) and daily (last month) survivors only after those, so a long run of deletes
/// can't push out the older history; when too big, the biggest go first and never below 6 files.
/// An agent's or a plugin's (always "pre"): the newest 9 and the oldest of the last week, at most 10
/// files and 256 MB, nothing older than a week (but the newest).
pub fn to_delete(all: &[Backup], now: u64) -> Vec<PathBuf> {
    let mut out = vec![];
    for (origin, quota) in [(Origin::Ui, &PERSON), (Origin::Agent, &OTHER), (Origin::Plugin, &OTHER)] {
        let mine: Vec<Backup> = all.iter().filter(|b| b.origin == origin).cloned().collect();
        out.extend(if origin == Origin::Ui { thin_person(&mine, now, quota) } else { thin_other(&mine, now, quota) });
    }
    out
}

/// Drop what is over the count (lowest tier, oldest first), then what is over the bytes (lowest tier, biggest
/// first, never below `min_keep` files); the newest is never dropped.
fn evict(all: &[Backup], keep: &mut [bool], tier: &[u8], q: &Quota) {
    let n = all.len();
    let mut kept = keep.iter().filter(|k| **k).count();
    for t in [T_PLAIN, T_PROTECTED, T_SPREAD] {
        for i in 0..n.saturating_sub(1) {
            if kept <= q.max {
                break;
            }
            if keep[i] && tier[i] == t {
                keep[i] = false;
                kept -= 1;
            }
        }
    }
    let mut total: u64 = all.iter().zip(keep.iter()).filter(|(_, k)| **k).map(|(b, _)| b.size).sum();
    for t in [T_PLAIN, T_PROTECTED, T_SPREAD] {
        let mut idx: Vec<usize> = (0..n.saturating_sub(1)).filter(|&i| keep[i] && tier[i] == t).collect();
        idx.sort_by(|&a, &b| all[b].size.cmp(&all[a].size).then(a.cmp(&b)));
        for i in idx {
            if total <= q.bytes || kept <= q.min_keep {
                return;
            }
            keep[i] = false;
            kept -= 1;
            total -= all[i].size;
        }
    }
}

fn thin_person(all: &[Backup], now: u64, q: &Quota) -> Vec<PathBuf> {
    let mut all = all.to_vec();
    all.sort_by(|a, b| (a.secs, a.n, a.pre).cmp(&(b.secs, b.n, b.pre)));
    let n = all.len();
    let mut keep = vec![false; n];
    let mut tier = vec![T_PLAIN; n];
    let cut = n.saturating_sub(KEEP_NEWEST);
    let mut seen = std::collections::HashSet::new();
    // newest first, so the first seen in a bucket is the newest in it
    for (i, b) in all.iter().enumerate().rev() {
        if i >= cut {
            keep[i] = true;
            continue;
        }
        let t = b.secs;
        let age = now.saturating_sub(t);
        let bucket = match age {
            0..600 => (0u8, t / 30),
            600..3600 => (1, t / 300),
            3600..172_800 => (2, t / 3600),
            _ => (3, t / 86400),
        };
        keep[i] = seen.insert(bucket);
    }
    // the way back over time: the newest of each hour for a day, of each day for a month
    let mut spread_seen = std::collections::HashSet::new();
    for (i, b) in all.iter().enumerate().rev() {
        let age = now.saturating_sub(b.secs);
        let bucket = match age {
            600..86_400 => Some((0u8, b.secs / 3600)),
            86_400..2_592_000 => Some((1, b.secs / 86_400)),
            _ => None,
        };
        if let Some(bk) = bucket {
            if spread_seen.insert(bk) {
                keep[i] = true;
                tier[i] = T_SPREAD;
            }
        }
    }
    let day = |b: &Backup| now.saturating_sub(b.secs) < 86_400;
    for (i, b) in all.iter().enumerate() {
        if b.pre && now.saturating_sub(b.secs) < q.pre_secs {
            keep[i] = true;
            tier[i] = tier[i].max(T_PROTECTED);
        }
    }
    if let Some((i, _)) = all.iter().enumerate().filter(|(_, b)| day(b)).max_by_key(|(_, b)| b.size) {
        keep[i] = true;
        tier[i] = tier[i].max(T_PROTECTED);
    }
    if let Some(last) = tier.last_mut() {
        *last = T_NEWEST; // the newest is never dropped
        keep[n - 1] = true;
    }
    evict(&all, &mut keep, &tier, q);
    all.iter().zip(keep).filter(|(_, k)| !*k).map(|(b, _)| b.path.clone()).collect()
}

fn thin_other(all: &[Backup], now: u64, q: &Quota) -> Vec<PathBuf> {
    let mut all = all.to_vec();
    all.sort_by(|a, b| (a.secs, a.n, a.pre).cmp(&(b.secs, b.n, b.pre)));
    let n = all.len();
    let mut keep = vec![true; n];
    let mut tier = vec![T_PLAIN; n];
    // nothing older than the window, except the newest
    for (i, b) in all.iter().enumerate() {
        if i + 1 < n && now.saturating_sub(b.secs) >= q.pre_secs {
            keep[i] = false;
        }
    }
    // the oldest still in its window is the state closest to before the run started: it is kept through a flood
    if let Some(i) = (0..n).find(|&i| keep[i]) {
        tier[i] = T_SPREAD;
    }
    if n > 0 {
        tier[n - 1] = T_NEWEST;
    }
    evict(&all, &mut keep, &tier, q);
    all.iter().zip(keep).filter(|(_, k)| !*k).map(|(b, _)| b.path.clone()).collect()
}

// ---- side files: work set aside, and copies of unreadable models -------------------------

/// The two kinds of file put beside the model when something would otherwise be lost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// `<stem>.unsaved-<secs>[-<n>].sketchcraft`: the person's own work, set aside before a reload replaced it
    Unsaved,
    /// `<stem>.damaged-<secs>[-<n>]`: a copy of a file that couldn't be read
    Damaged,
}

impl Side {
    pub fn word(self) -> &'static str {
        match self {
            Side::Unsaved => "unsaved",
            Side::Damaged => "damaged",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Side::Unsaved => "Work set aside",
            Side::Damaged => "Copy of an unreadable file",
        }
    }
}

/// Exactly `<stem>.<unsaved|damaged>-<digits>[-<digits>][.sketchcraft]`, nothing looser.
pub fn parse_side(stem: &str, name: &str) -> Option<(Side, u64)> {
    let rest = name.strip_prefix(stem)?.strip_prefix('.')?;
    let (side, rest) = if let Some(r) = rest.strip_prefix("unsaved-") {
        (Side::Unsaved, r.strip_suffix(".sketchcraft")?)
    } else {
        (Side::Damaged, rest.strip_prefix("damaged-")?)
    };
    let mut parts = rest.split('-');
    let secs = parts.next().filter(|d| (9..=11).contains(&d.len()) && d.bytes().all(|b| b.is_ascii_digit()))?.parse().ok()?;
    if let Some(d) = parts.next() {
        if d.is_empty() || d.len() > 6 || !d.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
    }
    parts.next().is_none().then_some((side, secs))
}

/// Does `name` look like one of the files SketchCraft sets aside? An agent never writes such a name.
pub fn looks_like_side(name: &str) -> bool {
    name.contains(".unsaved-") || name.contains(".damaged-")
}

/// The index of side files this program wrote (a hidden file beside the model, out of reach of agents).
/// Only files named in it are ever pruned.
pub fn index_path(doc: &Path) -> PathBuf {
    doc.with_file_name(format!(".{}.aside.json", doc.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "live".into())))
}

pub fn read_index(doc: &Path) -> Vec<String> {
    read_index_checked(doc).unwrap_or_default()
}

/// The index, or None when there is one that can't be read (deleted: an empty list, nothing is ours yet).
pub fn read_index_checked(doc: &Path) -> Option<Vec<String>> {
    match std::fs::read_to_string(index_path(doc)) {
        Ok(t) => serde_json::from_str::<Vec<String>>(&t).ok(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(vec![]),
        Err(_) => None,
    }
}

pub fn write_index(doc: &Path, names: &[String]) {
    if let Ok(t) = serde_json::to_vec(names) {
        crate::safety::write_atomic(&index_path(doc), &t).ok();
    }
}

/// Was this side file made by this program, as far as the disk can tell? It is named exactly as one of ours, is a plain
/// file, and was last written within two minutes of the time in its name (our copy is written when the name is made;
/// a file put there by hand or by another tool, or copied with its date kept, is not). Used when the index is gone.
pub fn provenance_ok(secs: u64, path: &Path) -> bool {
    crate::safety::plain_file(path).and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).is_some_and(|d| d.as_secs().abs_diff(secs) <= 120)
}

/// A side file in the folder: (kind, secs, path, size).
pub fn side_files(doc: &Path) -> Vec<(Side, u64, PathBuf, u64)> {
    let (Some(dir), Some(stem)) = (doc.parent().filter(|d| !d.as_os_str().is_empty()).or(Some(Path::new("."))), doc.file_stem().map(|s| s.to_string_lossy().into_owned())) else { return vec![] };
    let mut out: Vec<_> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let meta = crate::safety::plain_file(&e.path())?;
                    let (side, secs) = parse_side(&stem, &e.file_name().to_string_lossy())?;
                    Some((side, secs, e.path(), meta.len()))
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|(_, secs, p, _)| (*secs, p.clone()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(secs: u64, n: u64, pre: bool, origin: Origin, size: u64) -> Backup {
        Backup { path: PathBuf::from(format!("b-{secs}-{n}-{origin:?}{}", if pre { "-pre" } else { "" })), secs, n, pre, kind: None, origin, size }
    }

    #[test]
    fn names_round_trip_and_only_this_documents_count() {
        assert!(parse("a", "a-1794862562.sketchcraft").is_some_and(|p| p.secs == 1794862562 && !p.pre));
        let p = parse("a", "a-1794862562-3-pre-del-a.sketchcraft").unwrap();
        assert_eq!((p.secs, p.n, p.pre, p.kind.as_deref(), p.origin), (1794862562, 3, true, Some("del"), Origin::Agent));
        assert_eq!(parse("a", "a-1794862562-pre.sketchcraft").unwrap().origin, Origin::Ui);
        assert_eq!(parse("a", "a-1794862562-pre-p.sketchcraft").unwrap().origin, Origin::Plugin);
        for bad in ["a-b-1794862562.sketchcraft", "a-draft.sketchcraft", "a-12.sketchcraft", "a-1794862562.skp", "a-1794862562-1-2.sketchcraft", "a-1794862562-a.sketchcraft", "a-1794862562-pre-zzz.sketchcraft", "a-1794862562-pre-a-a.sketchcraft"] {
            assert!(parse("a", bad).is_none(), "{bad}");
        }
        assert!(parse("a-b", "a-b-1794862562.sketchcraft").is_some());
        // what we write, we read
        for (kind, o) in [(None, Origin::Ui), (Some("new"), Origin::Ui), (Some("bat"), Origin::Plugin), (Some("del"), Origin::Agent)] {
            let n = file_name("live", 1_800_000_000, 2, Some((kind, o)));
            let p = parse("live", &n).unwrap_or_else(|| panic!("{n}"));
            assert_eq!((p.kind.as_deref(), p.origin, p.pre, p.n), (kind, o, true, 2));
        }
    }

    #[test]
    fn side_names_are_exact() {
        assert_eq!(parse_side("live", "live.unsaved-1800000000.sketchcraft"), Some((Side::Unsaved, 1800000000)));
        assert_eq!(parse_side("live", "live.unsaved-1800000000-2.sketchcraft"), Some((Side::Unsaved, 1800000000)));
        assert_eq!(parse_side("live", "live.damaged-1800000000"), Some((Side::Damaged, 1800000000)));
        for bad in ["live.damaged-1800000000.sketchcraft.bak", "live.unsaved-1800000000", "live.unsaved-18.sketchcraft", "live2.unsaved-1800000000.sketchcraft", "live.unsaved-1800000000-x.sketchcraft", "live.unsaved-1800000000-1-1.sketchcraft"] {
            assert!(parse_side("live", bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn thinning_keeps_the_newest_of_a_bucket_and_what_was_taken_before_a_destructive_change() {
        let now = 1_800_000_000;
        let mut all: Vec<Backup> = (0..20).map(|i| b(now - 100 + i, 0, false, Origin::Ui, 10)).collect();
        all.insert(0, b(now - 100, 1, true, Origin::Ui, 5000));
        let gone: Vec<String> = to_delete(&all, now).iter().map(|p| p.to_string_lossy().into_owned()).collect();
        assert!(!gone.iter().any(|g| g.contains("-pre")), "the pre-destructive backup was thinned: {gone:?}");
        assert!(!gone.iter().any(|g| g == &format!("b-{}-0-Ui", now - 81)), "newest 8 kept");
        assert!(gone.len() >= 8, "the older plain ones are thinned: {gone:?}");
        let many: Vec<Backup> = (0..200).map(|i| b(now - 200_000 + i * 1000, 0, false, Origin::Ui, 10)).collect();
        let gone = to_delete(&many, now);
        assert!(!gone.contains(&many[199].path));
        assert!(many.len() - gone.len() <= MAX_PERSON);
    }

    #[test]
    fn an_agents_flood_cannot_touch_the_persons_backups_and_keeps_its_oldest() {
        let now = 1_800_000_000;
        let mut all: Vec<Backup> = (0..5).map(|i| b(now - 5000 + i * 100, 0, true, Origin::Ui, 100)).collect();
        // 200 agent backups in the last hour
        all.extend((0..200).map(|i| b(now - 3500 + i * 10, 0, true, Origin::Agent, 100)));
        let gone = to_delete(&all, now);
        assert!(!gone.iter().any(|p| p.to_string_lossy().contains("Ui")), "the person's are untouched: {gone:?}");
        let kept_agent: Vec<&Backup> = all.iter().filter(|x| x.origin == Origin::Agent && !gone.contains(&x.path)).collect();
        assert_eq!(kept_agent.len(), MAX_OTHER);
        assert_eq!(kept_agent[0].secs, now - 3500, "the oldest of the flood is kept");
        assert_eq!(kept_agent.last().unwrap().secs, now - 3500 + 1990, "and the newest");
    }

    #[test]
    fn bytes_are_capped_as_well_as_counts() {
        let now = 1_800_000_000;
        // 12 agent backups of 40 MB each: 480 MB, over the 256 MB limit
        let all: Vec<Backup> = (0..9).map(|i| b(now - 900 + i * 10, 0, true, Origin::Plugin, 40 << 20)).collect();
        let gone = to_delete(&all, now);
        let kept: u64 = all.iter().filter(|x| !gone.contains(&x.path)).map(|x| x.size).sum();
        assert!(kept <= 256 << 20, "{kept}");
        assert!(!gone.contains(&all[8].path), "the newest stays");
        // one giant backup alone is still kept
        let one = vec![b(now - 10, 0, true, Origin::Ui, 3 << 30)];
        assert!(to_delete(&one, now).is_empty());
    }
}
