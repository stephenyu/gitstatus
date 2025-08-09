use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use gix::diff::index::{Action, ChangeRef};
use gix::progress;
use gix::status::{self, index_worktree::iter::Summary as IterSummary, UntrackedFiles};
use gix::Repository;
use std::path::Path;

#[derive(Parser)]
#[command(
    name = "gitstatus",
    about = "Get concise git repository status information",
    version
)]
struct Args {
    /// Path to the git repository (defaults to current directory)
    #[arg(short, long, default_value = ".")]
    path: String,

    /// Show detailed output
    #[arg(short, long)]
    verbose: bool,

    /// Control untracked files handling (e.g., -uno == --untracked no)
    /// Values: no | normal | all
    #[arg(short = 'u', long = "untracked", value_enum)]
    untracked: Option<UntrackedArg>,

    /// Show all files including untracked files (overrides default behavior)
    #[arg(long)]
    all: bool,
}

fn main() {
    let args = Args::parse();

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

    let status = GitStatus::from_repository(&repo, args.untracked, args.all)?;
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
        let changes = get_changes_summary(repo, untracked, all)?;

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
    let head = repo.head()?;
    let tracking_branch = head.into_remote(gix::remote::Direction::Fetch).transpose()?;

    match tracking_branch {
        Some(branch) => match branch.name() {
            Some(name) => Ok(name.as_bstr().to_string()),
            None => Err(anyhow::anyhow!("Upstream branch name is not valid UTF-8")),
        },
        None => Err(anyhow::anyhow!(
            "No upstream branch configured for current branch"
        )),
    }
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum UntrackedArg {
    No,
    Normal,
    All,
}

fn get_changes_summary(repo: &Repository, untracked: Option<UntrackedArg>, all: bool) -> Result<ChangesSummary> {
    let mut summary = ChangesSummary::default();

    // 1) Count staged changes: diff HEAD tree vs index
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
