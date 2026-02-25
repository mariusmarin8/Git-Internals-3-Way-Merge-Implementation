use anyhow::Result;
use std::fs;
use std::path::PathBuf;

pub struct Refs {
    git_path: PathBuf,
}

impl Refs {
    pub fn new(git_path: PathBuf) -> Self {
        Refs { git_path }
    }

    pub fn read_head(&self) -> Result<Option<String>> {
        let head_path = self.git_path.join("HEAD");

        // repo gol
        if !head_path.exists() {
            return Ok(None);
        }

        let head_content_string = fs::read_to_string(&head_path)?;
        let head_content = head_content_string.trim();

        if let Some(l) = head_content.strip_prefix("ref: ") {
            let ref_path = self.git_path.join(l);
            if ref_path.exists() {
                let hash = fs::read_to_string(ref_path)?;
                Ok(Some(hash.trim().to_string()))
            } else {
                // branch fara commit
                Ok(None)
            }
        } else {
            // detached HEAD
            Ok(Some(head_content.to_string()))
        }
    }

    pub fn update_head(&self, new_commit_hash: &str) -> Result<()> {
        let head_path = self.git_path.join("HEAD");

        if !head_path.exists() {
            fs::write(&head_path, "ref: refs/heads/main")?;
        }

        let head_content_string = fs::read_to_string(&head_path)?;
        let head_content = head_content_string.trim();

        if let Some(l) = head_content.strip_prefix("ref: ") {
            let ref_path = self.git_path.join(l);

            let parent_dir = ref_path.parent().ok_or_else(|| {
                anyhow::anyhow!(
                    "Nu pot determina directorul părinte pentru referința: {:?}",
                    ref_path
                )
            })?;

            fs::create_dir_all(parent_dir)?;
            fs::write(ref_path, new_commit_hash)?;
        } else {
            fs::write(head_path, new_commit_hash)?;
        }
        Ok(())
    }

    pub fn create_branch(&self, branch_name: &str, commit_hash: &str) -> Result<()> {
        let branch_path = self.git_path.join("refs").join("heads").join(branch_name);
        if branch_path.exists() {
            return Err(anyhow::anyhow!("Branch-ul '{}' există deja!", branch_name));
        }
        if let Some(parent) = branch_path.parent() {
            fs::create_dir_all(parent)?;
        } else {
            return Err(anyhow::anyhow!("Nu pot crea directorul pentru branch."));
        }
        fs::write(branch_path, commit_hash)?;
        Ok(())
    }
}
