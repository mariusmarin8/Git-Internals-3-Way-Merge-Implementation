use anyhow::Result;
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use sha1::{Digest, Sha1};
use std::fs;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::time::SystemTime;
pub struct TreeEntry {
    pub mode: String,
    pub name: String,
    pub hash: Vec<u8>,
}

impl TreeEntry {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(self.mode.as_bytes());
        bytes.push(b' ');
        bytes.extend_from_slice(self.name.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&self.hash);
        bytes
    }
}

fn save_object(repo_path: &Path, content: &[u8]) -> anyhow::Result<String> {
    let mut hasher = Sha1::new();
    hasher.update(content);
    let result = hasher.finalize();
    let hash = hex::encode(result);

    let folder_name = &hash[0..2];
    let file_name = &hash[2..];

    let objects_dir = repo_path.join("objects").join(folder_name);
    fs::create_dir_all(&objects_dir)?;

    let full_path = objects_dir.join(file_name);

    if !Path::new(&full_path).exists() {
        let file = fs::File::create(&full_path)?;
        let mut encoder = ZlibEncoder::new(file, Compression::default());
        encoder.write_all(content)?;
        encoder.finish()?;
    }

    Ok(hash)
}

pub fn create_blob(repo_path: &Path, file_path: &str) -> anyhow::Result<String> {
    let content = fs::read(file_path)?;
    let header = format!("blob {}\0", content.len());
    let mut store: Vec<u8> = Vec::new();
    store.extend_from_slice(header.as_bytes());
    store.extend_from_slice(&content);
    save_object(repo_path, &store)
}

pub fn create_tree(repo_path: &Path, mut entries: Vec<TreeEntry>) -> Result<String> {
    // Sortare obligatorie pentru Git
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    let mut content = Vec::new();
    for entry in entries {
        content.extend(entry.to_bytes());
    }

    let header = format!("tree {}\0", content.len());

    let mut store = Vec::new();
    store.extend_from_slice(header.as_bytes());
    store.extend_from_slice(&content);

    save_object(repo_path, &store)
}

pub fn create_commit(
    repo_path: &Path,
    tree_hash: &str,
    parents: Vec<String>,
    message: &str,
) -> anyhow::Result<String> {
    let mut body = String::new();
    body.push_str(&format!("tree {}\n", tree_hash));
    //primul commit nu are parinte
    for parent in parents {
        body.push_str(&format!("parent {}\n", parent));
    }

    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs();

    let author_line = format!("author Eu <eu@test.com> {} +0000\n", timestamp);
    let committer_line = format!("committer Eu <eu@test.com> {} +0000\n", timestamp);

    body.push_str(&author_line);
    body.push_str(&committer_line);

    body.push('\n');
    body.push_str(message);

    let header = format!("commit {}\0", body.len());
    let mut store = Vec::new();
    store.extend_from_slice(header.as_bytes());
    store.extend_from_slice(body.as_bytes());

    save_object(repo_path, &store)
}

pub fn read_object(repo_path: &Path, hash: &str) -> anyhow::Result<(String, Vec<u8>)> {
    if hash.len() < 2 {
        return Err(anyhow::anyhow!("Hash invalid: '{}' este prea scurt.", hash));
    }
    let folder = &hash[0..2];
    let file = &hash[2..];
    let obj_path = repo_path.join("objects").join(folder).join(file);
    if !obj_path.exists() {
        return Err(anyhow::anyhow!("Obiectul {} nu există.", hash));
    }
    let f = fs::File::open(obj_path)?;
    let mut decoder = ZlibDecoder::new(f);
    let mut buffer = Vec::new();
    decoder.read_to_end(&mut buffer)?;

    let null_idx = buffer
        .iter()
        .position(|&x| x == 0)
        .ok_or(anyhow::anyhow!("Obiect corupt: lipsește header-ul null"))?;

    let header = String::from_utf8(buffer[0..null_idx].to_vec())?;
    let content = buffer[null_idx + 1..].to_vec();

    let type_str = header
        .split_whitespace()
        .next()
        .ok_or(anyhow::anyhow!("Header invalid"))?
        .to_string();

    Ok((type_str, content))
}
