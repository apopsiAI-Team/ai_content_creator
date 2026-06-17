use vergen::EmitBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    EmitBuilder::builder()
        .build_date()
        .build_timestamp()
        .git_branch()
        .git_commit_date()
        .git_commit_timestamp()
        .git_sha(false) // full SHA, not short
        .git_dirty(false) // don't include untracked files
        .emit()?;

    Ok(())
}
