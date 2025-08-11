use anyhow::{Context, Result};
use std::env;
use gix::diff::index::{Action, ChangeRef};
use gix::progress;
use gix::status::{self, index_worktree::iter::Summary as IterSummary, UntrackedFiles};
use gix::Repository;
use std::fs;
use std::path::{Path, PathBuf};

struct Args {
    path: String,
    verbose: bool,
    untracked: Option<UntrackedArg>,
    all: bool,
    no_staged: bool,
    direct_upstream: bool,
}

fn parse_args() -> Args {
    let mut path = ".".to_string();
    let mut verbose = false;
    let mut untracked: Option<UntrackedArg> = None;
    let mut all = false;
    let mut no_staged = false;
    let mut direct_upstream = false;

    let mut args = env::args().skip(1).peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "-p" | "--path" => {
                if let Some(val) = args.next() {
                    path = val;
                } else {
                    eprintln!("--path requires a value");
                    std::process::exit(2);
                }
            }
            "-v" | "--verbose" => {
                verbose = true;
            }
            "-u" | "--untracked" => {
                let val = args.next().unwrap_or_else(|| {
                    eprintln!("--untracked requires one of: no|normal|all");
                    std::process::exit(2);
                });
                untracked = match val.as_str() {
                    "no" => Some(UntrackedArg::No),
                    "normal" => Some(UntrackedArg::Normal),
                    "all" => Some(UntrackedArg::All),
                    _ => {
                        eprintln!("Invalid value for --untracked: {}", val);
                        std::process::exit(2);
                    }
                };
            }
            "--all" => {
                all = true;
            }
            "-S" | "--no-staged" => { no_staged = true; }
            "-U" | "--direct-upstream" => { direct_upstream = true; }
            "-h" | "--help" => {
                print_usage_and_exit(0);
            }
            _ if arg.starts_with('-') => {
                eprintln!("Unknown option: {}", arg);
                print_usage_and_exit(2);
            }
            other => {
                // Positional path
                path = other.to_string();
            }
        }
    }

    Args { path, verbose, untracked, all, no_staged, direct_upstream }
}

fn print_usage_and_exit(code: i32) -> ! {
    eprintln!(
        "gitstatus [--path <dir>] [--verbose] [-u no|normal|all] [--all] [--no-staged|-S] [--direct-upstream|-U] [--version]"
    );
    std::process::exit(code);
}

fn main() {
    let args = parse_args();

    match run(&args) {
        Ok(output) => println!("{}", output),
        Err(e) => {
            if args.verbose {
                eprintln!("Error: {:?}", e);
            }
            std::process::exit(1);
        }
    }
}

fn run(args: &Args) -> Result<String> {
    let repo = discover_repository(&args.path).context("Failed to find git repository")?;

    // Resolve branch/upstream either via direct reads or via gix
    let (current_branch, upstream_branch) = if args.direct_upstream {
        let (_worktree_root, git_dir) = find_repo_roots(Path::new(&args.path))?;
        let current_branch = read_current_branch_fast(&git_dir)?;
        let upstream_branch = read_upstream_branch_fast(&git_dir, &current_branch).ok();
        (current_branch, upstream_branch)
    } else {
        let current_branch = get_current_branch_name(&repo)?;
        let upstream_branch = get_upstream_branch_name(&repo).ok();
        (current_branch, upstream_branch)
    };

    let changes = get_changes_summary(&repo, args.untracked, args.all, !args.no_staged)?;
    let status = GitStatus { current_branch, upstream_branch, changes };
    Ok(status.format())
}

fn discover_repository(path: &str) -> Result<Repository> {
    gix::discover(Path::new(path)).context("Not a git repository or unable to access")
}

#[derive(Debug)]
struct GitStatus {
    current_branch: String,
    upstream_branch: Option<String>,
    changes: ChangesSummary,
}

#[derive(Debug, Default)]
struct ChangesSummary {
    staged: usize,
    modified: usize,
    deleted: usize,
    renamed: usize,
    untracked: usize,
}

impl GitStatus {
    fn from_repository(repo: &Repository, untracked: Option<UntrackedArg>, all: bool) -> Result<Self> {
        let current_branch = get_current_branch_name(repo)?;
        let upstream_branch = get_upstream_branch_name(repo).ok();
        let changes = get_changes_summary(repo, untracked, all, true)?;

        Ok(GitStatus {
            current_branch,
            upstream_branch,
            changes,
        })
    }

    fn format(&self) -> String {
        let mut components = Vec::new();

        // Add current branch
        components.push(self.current_branch.clone());

        // Add upstream branch if it exists and is different
        if let Some(ref upstream) = self.upstream_branch {
            if upstream != &self.current_branch {
                components.push(upstream.clone());
            }
        }

        // Add changes summary
        components.push(self.changes.format());

        components.join(" ")
    }
}

impl ChangesSummary {
    fn is_clean(&self) -> bool {
        self.staged == 0
            && self.modified == 0
            && self.deleted == 0
            && self.renamed == 0
            && self.untracked == 0
    }

    fn format(&self) -> String {
        if self.is_clean() {
            return "✓".to_string();
        }

        let mut parts = Vec::new();

        if self.staged > 0 {
            parts.push(format!("^{}", self.staged));
        }
        if self.renamed > 0 {
            parts.push(format!("~{}", self.renamed));
        }
        if self.modified > 0 {
            parts.push(format!("~{}", self.modified));
        }
        if self.deleted > 0 {
            parts.push(format!("-{}", self.deleted));
        }
        if self.untracked > 0 {
            parts.push(format!("+{}", self.untracked));
        }

        parts.join("")
    }
}

fn get_current_branch_name(repo: &Repository) -> Result<String> {
    match repo.head() {
        Ok(head_ref) => {
            match head_ref.referent_name() {
                Some(refname) => {
                    // Extract branch name from refs/heads/branch_name
                    let branch_name = refname.shorten();
                    Ok(branch_name.to_string())
                }
                None => {
                    // Detached HEAD state
                    Ok("HEAD".to_string())
                }
            }
        }
        Err(_) => {
            // Repository has no HEAD (empty repository)
            Ok("(no branch)".to_string())
        }
    }
}

fn get_upstream_branch_name(repo: &Repository) -> Result<String> {
    // Resolve current HEAD to obtain the local branch reference name
    let head = repo.head()?;
    let local_ref_name = match head.referent_name() {
        Some(name) => name,
        None => {
            // Detached or unborn HEAD: no upstream
            return Err(anyhow::anyhow!(
                "No upstream branch configured for current branch"
            ));
        }
    };

    // Determine the configured remote name for fetch operations, e.g., "origin"
    let remote_name = match repo.branch_remote_name(local_ref_name.shorten(), gix::remote::Direction::Fetch) {
        Some(name) => name.as_bstr().to_string(),
        None => {
            return Err(anyhow::anyhow!(
                "No upstream branch configured for current branch"
            ));
        }
    };

    // Determine the upstream branch ref name on the remote, e.g., "refs/heads/main"
    let upstream_ref = match repo.branch_remote_ref_name(local_ref_name, gix::remote::Direction::Fetch) {
        Some(Ok(name)) => name,
        Some(Err(_)) | None => {
            return Err(anyhow::anyhow!(
                "No upstream branch configured for current branch"
            ));
        }
    };

    // Shorten to branch name like "main" from "refs/heads/main"
    let short_upstream = upstream_ref.shorten().to_string();
    let branch_only = short_upstream.strip_prefix("heads/").unwrap_or(&short_upstream);

    Ok(format!("{}/{}", remote_name, branch_only))
}

#[derive(Copy, Clone, Debug)]
enum UntrackedArg {
    No,
    Normal,
    All,
}

fn get_changes_summary(repo: &Repository, untracked: Option<UntrackedArg>, all: bool, include_staged: bool) -> Result<ChangesSummary> {
    let mut summary = ChangesSummary::default();

    // 1) Optional: Count staged changes (HEAD tree vs index)
    if include_staged {
        if let Ok(head_tree_id) = repo.head_tree_id() {
            let worktree_index = repo.index_or_empty()?;
            let _ = repo.tree_index_status::<anyhow::Error>(
                &head_tree_id,
                &worktree_index,
                None,
                status::tree_index::TrackRenames::Disabled,
                |change, _tree_idx, _work_idx| {
                    match change {
                        ChangeRef::Addition { .. } => summary.staged += 1,
                        ChangeRef::Modification { .. } => summary.staged += 1,
                        ChangeRef::Deletion { .. } => summary.staged += 1,
                        ChangeRef::Rewrite { .. } => summary.staged += 1,
                    }
                    Ok(Action::Continue)
                },
            );
        }
    }

    // 2) Count working tree changes: diff index vs worktree (tracked files only by default)
    let include_untracked_mode = match untracked {
        Some(UntrackedArg::No) => UntrackedFiles::None,
        Some(UntrackedArg::Normal) => UntrackedFiles::Collapsed,
        Some(UntrackedArg::All) => UntrackedFiles::Files,
        None => if all { UntrackedFiles::Files } else { UntrackedFiles::None },
    };

    let mut platform = repo
        .status(progress::Discard)?
        .index_worktree_rewrites(None)
        .index_worktree_submodules(None);

    platform = match include_untracked_mode {
        UntrackedFiles::None => platform.index_worktree_options_mut(|opts| {
            // Disable dirwalk entirely for maximum speed.
            opts.dirwalk_options = None;
        }),
        other => platform.untracked_files(other),
    };

    let mut iter = platform.into_index_worktree_iter(Vec::new())?;
    while let Some(item_res) = iter.next() {
        let Ok(item) = item_res else { break };
        match item.summary() {
            Some(IterSummary::Added) => summary.untracked += 1,
            Some(IterSummary::Removed) => summary.deleted += 1,
            Some(IterSummary::Modified) | Some(IterSummary::TypeChange) | Some(IterSummary::Conflict) => {
                summary.modified += 1
            }
            Some(IterSummary::Renamed) => summary.renamed += 1,
            Some(IterSummary::Copied) => {
                // Treat copies similar to renames for summary purposes
                summary.renamed += 1
            }
            Some(IterSummary::IntentToAdd) => summary.staged += 1,
            None => {}
        }
    }

    Ok(summary)
}

// Ultra-fast path helpers: read branch and upstream directly from .git without loading repo config in gix

fn find_repo_roots(start: &Path) -> Result<(PathBuf, PathBuf)> {
    let mut cur = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    if cur.is_file() {
        cur = cur.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
    }
    loop {
        let dot_git = cur.join(".git");
        if dot_git.is_dir() {
            return Ok((cur.clone(), dot_git));
        }
        if dot_git.is_file() {
            // read gitdir: path
            let content = fs::read_to_string(&dot_git).context("Failed to read .git file")?;
            let prefix = "gitdir:";
            for line in content.lines() {
                if let Some(rest) = line.strip_prefix(prefix) {
                    let gitdir = rest.trim();
                    let gitdir_path = if Path::new(gitdir).is_absolute() {
                        PathBuf::from(gitdir)
                    } else {
                        cur.join(gitdir)
                    };
                    return Ok((cur.clone(), gitdir_path));
                }
            }
            anyhow::bail!("Invalid .git file: missing gitdir");
        }
        if !cur.pop() {
            anyhow::bail!("Not a git repository");
        }
    }
}

fn read_current_branch_fast(git_dir: &Path) -> Result<String> {
    let head_path = git_dir.join("HEAD");
    let head = fs::read_to_string(&head_path).with_context(|| format!("Failed to read {}", head_path.display()))?;
    if let Some(rest) = head.trim().strip_prefix("ref: ") {
        // ref: refs/heads/branch
        let branch = rest.rsplit('/').next().unwrap_or(rest).to_string();
        Ok(branch)
    } else {
        Ok("HEAD".to_string())
    }
}

fn read_upstream_branch_fast(git_dir: &Path, current_branch: &str) -> Result<String> {
    if current_branch == "HEAD" || current_branch == "(no branch)" {
        anyhow::bail!("No upstream for detached or unborn HEAD");
    }
    let config_path = git_dir.join("config");
    let config = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;

    let mut in_section = false;
    let mut remote: Option<String> = None;
    let mut merge: Option<String> = None;
    let section_header = format!("[branch \"{}\"]", current_branch);
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_section = line == section_header;
            continue;
        }
        if !in_section || line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(val) = line.strip_prefix("remote =") {
            remote = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("merge =") {
            merge = Some(val.trim().to_string());
        }
    }

    let remote = remote.ok_or_else(|| anyhow::anyhow!("No upstream branch configured"))?;
    let merge = merge.ok_or_else(|| anyhow::anyhow!("No upstream branch configured"))?;
    let short = merge.strip_prefix("refs/heads/").unwrap_or(&merge);
    Ok(format!("{}/{}", remote, short))
}

// Server functionality removed for simplicity.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_changes_summary_clean() {
        let summary = ChangesSummary::default();
        assert!(summary.is_clean());
        assert_eq!(summary.format(), "✓");
    }

    #[test]
    fn test_changes_summary_with_changes() {
        let summary = ChangesSummary {
            staged: 0,
            modified: 1,
            deleted: 3,
            renamed: 0,
            untracked: 0,
        };
        assert!(!summary.is_clean());
        assert_eq!(summary.format(), "~1-3");
    }

    #[test]
    fn test_changes_summary_all_types() {
        let summary = ChangesSummary {
            staged: 1,
            modified: 2,
            deleted: 3,
            renamed: 4,
            untracked: 5,
        };
        assert_eq!(summary.format(), "^1~4~2-3+5");
    }

    #[test]
    fn test_git_status_format_no_upstream() {
        let status = GitStatus {
            current_branch: "main".to_string(),
            upstream_branch: None,
            changes: ChangesSummary::default(),
        };
        assert_eq!(status.format(), "main ✓");
    }

    #[test]
    fn test_git_status_format_with_upstream() {
        let status = GitStatus {
            current_branch: "main".to_string(),
            upstream_branch: Some("origin/main".to_string()),
            changes: ChangesSummary::default(),
        };
        assert_eq!(status.format(), "main origin/main ✓");
    }

    #[test]
    fn test_git_status_format_same_upstream() {
        let status = GitStatus {
            current_branch: "main".to_string(),
            upstream_branch: Some("main".to_string()),
            changes: ChangesSummary::default(),
        };
        // Should not duplicate branch name when upstream is the same
        assert_eq!(status.format(), "main ✓");
    }

    #[test]
    fn test_git_status_format_with_changes() {
        let status = GitStatus {
            current_branch: "feature".to_string(),
            upstream_branch: Some("origin/feature".to_string()),
            changes: ChangesSummary {
                staged: 0,
                modified: 2,
                deleted: 0,
                renamed: 0,
                untracked: 0,
            },
        };
        assert_eq!(status.format(), "feature origin/feature ~2");
    }

    #[test]
    fn test_changes_summary_one_modified_one_deleted() {
        let summary = ChangesSummary {
            staged: 0,
            modified: 1,
            deleted: 1,
            renamed: 0,
            untracked: 0,
        };
        assert_eq!(summary.format(), "~1-1");
    }
}
