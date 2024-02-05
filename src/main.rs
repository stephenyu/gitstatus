use git2::{BranchType, Error, Repository, StatusOptions, StatusShow};
use std::process;

fn main() {
    let repo = match Repository::open(".") {
        Ok(repo) => repo,
        Err(_) => process::exit(0), // Exit if the repository cannot be opened
    };

    let mut components = Vec::new();

    // Get current branch name and add it to components if present
    if let Ok(branch_name) = get_current_branch_name(&repo) {
        components.push(branch_name);
    }

    // Get upstream branch name and add it to components if present
    if let Ok(upstream_name) = get_upstream_branch_name(&repo) {
        components.push(upstream_name);
    }

    // Get git status message and add it to components if present
    let status_message = get_git_status(&repo);
    if !status_message.is_empty() {
        components.push(status_message);
    }

    // Join all components with a space, ensuring no extra spaces if a section is empty
    let output = components.join(" ");
    println!("{}", output);
}

fn get_current_branch_name(repo: &Repository) -> Result<String, Error> {
    let head = repo.head()?;
    if head.is_branch() {
        let shorthand = head.shorthand().unwrap_or("unknown branch");
        Ok(shorthand.to_string())
    } else {
        Ok("HEAD is detached".to_string())
    }
}

fn get_upstream_branch_name(repo: &Repository) -> Result<String, Error> {
    let head = repo.head()?;
    let branch = repo.find_branch(head.shorthand().unwrap_or_default(), BranchType::Local)?;
    let upstream = branch.upstream()?;
    let upstream_name = upstream.name()?.unwrap_or_default();
    Ok(upstream_name.to_string())
}

fn get_git_status(repo: &Repository) -> String {
    let mut opts = StatusOptions::new();
    opts.include_untracked(false) // Equivalent to -uno flag
        .show(StatusShow::IndexAndWorkdir);

    let statuses = match repo.statuses(Some(&mut opts)) {
        Ok(statuses) => statuses,
        Err(_) => return "Failed to get status".to_string(),
    };

    construct_git_status(&statuses)
}

fn construct_git_status(statuses: &git2::Statuses) -> String {
    let mut updated = 0;
    let mut deleted = 0;
    let mut untracked = 0;

    for entry in statuses.iter() {
        match entry.status() {
            s if s.contains(git2::Status::WT_MODIFIED) => updated += 1,
            s if s.contains(git2::Status::WT_DELETED) => deleted += 1,
            s if s.contains(git2::Status::WT_NEW) => untracked += 1,
            _ => {}
        }
    }

    let mut message = String::new();
    if updated > 0 {
        message += &format!("+{}", updated);
    }
    if deleted > 0 {
        message += &format!("x{}", deleted);
    }
    if untracked > 0 {
        message += &format!("n{}", untracked);
    }
    if message.is_empty() {
        message = "✓".to_string();
    }

    message
}

