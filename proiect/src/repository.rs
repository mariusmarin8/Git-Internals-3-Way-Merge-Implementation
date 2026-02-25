use crate::objects::{self, TreeEntry};
use anyhow::Result;
use colored::Colorize;
use glob::Pattern;
use similar::{ChangeTag, TextDiff};
use std::collections::HashMap;
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
pub struct Repository {
    pub path: PathBuf,
}

impl Repository {
    pub fn init(root_path: &str) -> Result<Self> {
        let root = Path::new(root_path);
        let git_path = root.join(".mygit");

        if Path::new(".mygit").exists() {
            println!("Repo-ul '.mygit' exista deja!");
            return Ok(Repository { path: git_path });
        }
        fs::create_dir_all(".mygit/objects")?;
        fs::create_dir_all(".mygit/refs/heads")?;

        Ok(Repository { path: git_path })
    }

    pub fn create_blob(&self, file_path: &str) -> Result<String> {
        objects::create_blob(&self.path, file_path)
    }

    pub fn create_tree(&self, entries: Vec<TreeEntry>) -> Result<String> {
        objects::create_tree(&self.path, entries)
    }

    pub fn commit(&self, tree_hash: &str, parents: Vec<String>, msg: &str) -> Result<String> {
        objects::create_commit(&self.path, tree_hash, parents, msg)
    }

    pub fn checkout(&self, commit_hash: &str) -> Result<()> {
        let (obj_type, cont) = objects::read_object(&self.path, commit_hash)?;

        if obj_type != "commit" {
            return Err(anyhow::anyhow!(
                "Hash-ul {} nu este un commit!",
                commit_hash
            ));
        }
        //transform din vec<u8> in string
        let obj_cont = String::from_utf8(cont)?;
        //extrag linia cu tipul, trebuie sa fie tree
        let line = obj_cont
            .lines()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Commitul nu este valid"))?;

        if !line.starts_with("tree ") {
            return Err(anyhow::anyhow!("Commitul nu este valid"));
        }

        let tree_hash = &line[5..];
        let workspace_root = self
            .path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Nu pot determina radacina proiectului"))?;
        self.clean(workspace_root)?;

        self.restore(tree_hash, workspace_root)?;

        Ok(())
    }

    fn restore(&self, tree_hash: &str, current_path: &Path) -> Result<()> {
        let (obj_type, cont) = objects::read_object(&self.path, tree_hash)?;
        if obj_type != "tree" {
            return Err(anyhow::anyhow!("Obiectul {} nu este un tree", tree_hash));
        }

        let mut i = 0;

        while i < cont.len() {
            //extrag formatul

            //mode
            let pos_mode = &cont[i..]
                .iter()
                .position(|&b| b == b' ')
                .ok_or(anyhow::anyhow!("Tree corupt (mod)"))?;

            let final_pos_mode = pos_mode + i;
            let mode = std::str::from_utf8(&cont[i..final_pos_mode])?;

            //name

            let name_pos = &cont[final_pos_mode..]
                .iter()
                .position(|&b| b == 0)
                .ok_or(anyhow::anyhow!("Tree corupt (nume)"))?;
            let final_name_pos = final_pos_mode + name_pos;
            let name = std::str::from_utf8(&cont[final_pos_mode + 1..final_name_pos])?;

            //hash

            let hash_bytes = &cont[final_name_pos + 1..final_name_pos + 21];
            let hash_hex = hex::encode(hash_bytes);

            let new_path = current_path.join(name);

            if mode == "040000" {
                fs::create_dir_all(&new_path)?;
                self.restore(&hash_hex, &new_path)?;
            } else {
                //fisier
                let (_, blob_content) = objects::read_object(&self.path, &hash_hex)?;
                fs::write(&new_path, blob_content)?;
            }

            // Sărim la următoarea intrare
            i = final_name_pos + 1 + 20;
        }
        Ok(())
    }

    fn clean(&self, root: &Path) -> Result<()> {
        let dir = fs::read_dir(root)?;

        let ignore_patterns = self.get_ignored_files()?;

        for entry in dir {
            let ent = entry?;
            let path = ent.path();
            let name = path
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Calea nu are nume"))?
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Numele fișierului nu este UTF-8 valid"))?;

            if name == ".mygit" || name == "target" || name.ends_with(".exe") {
                continue;
            }
            let rel_path = path.strip_prefix(root).unwrap_or(&path).to_string_lossy();
            let rel_path = rel_path.replace('\\', "/");

            if self.is_ignored(&rel_path, &ignore_patterns) {
                continue;
            }

            if path.is_dir() {
                fs::remove_dir_all(path)?;
            } else {
                fs::remove_file(path)?;
            }
        }

        Ok(())
    }

    pub fn get_files(&self, tree_hash: &str) -> Result<HashMap<String, String>> {
        let mut store = HashMap::new();
        self.collect(tree_hash, &mut store, "")?;
        Ok(store)
    }

    fn collect(
        &self,
        tree_hash: &str,
        store: &mut HashMap<String, String>,
        prefix: &str,
    ) -> Result<()> {
        let (obj_type, cont) = crate::objects::read_object(&self.path, tree_hash)?;

        if obj_type != "tree" {
            return Ok(());
        }

        let mut i = 0;
        while i < cont.len() {
            //extrag formatul

            //mode
            let pos_mode = &cont[i..]
                .iter()
                .position(|&b| b == b' ')
                .ok_or(anyhow::anyhow!("Tree corupt (mod)"))?;

            let final_pos_mode = pos_mode + i;
            let mode = std::str::from_utf8(&cont[i..final_pos_mode])?;

            //name

            let name_pos = &cont[final_pos_mode..]
                .iter()
                .position(|&b| b == 0)
                .ok_or(anyhow::anyhow!("Tree corupt (nume)"))?;
            let final_name_pos = final_pos_mode + name_pos;
            let name = std::str::from_utf8(&cont[final_pos_mode + 1..final_name_pos])?;

            //hash

            let hash_bytes = &cont[final_name_pos + 1..final_name_pos + 21];
            let hash_hex = hex::encode(hash_bytes);

            let full_path = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{}/{}", prefix, name)
            };

            if mode == "040000" {
                //director
                self.collect(&hash_hex, store, &full_path)?;
            } else {
                //fisier
                store.insert(full_path, hash_hex);
            }

            i = final_name_pos + 1 + 20;
        }

        Ok(())
    }

    pub fn get_tree_from_commit(&self, hash: &str) -> Result<String> {
        let (_, data) = objects::read_object(&self.path, hash)?;
        let text = String::from_utf8(data)?;
        let tree_line = text
            .lines()
            .next()
            .ok_or(anyhow::anyhow!("Commit invalid"))?;
        Ok(tree_line[5..].to_string())
    }

    pub fn write_conflict_file(
        &self,
        path: &Path,
        head_hash: &str,
        target_hash: &str,
    ) -> Result<()> {
        let (_, content_head) = crate::objects::read_object(&self.path, head_hash)?;
        let (_, content_target) = crate::objects::read_object(&self.path, target_hash)?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = fs::File::create(path)?;
        writeln!(file, "<<<<<<< HEAD")?;
        file.write_all(&content_head)?;
        writeln!(file, "\n=======")?;
        file.write_all(&content_target)?;
        writeln!(file, "\n>>>>>>> MERGE_TARGET")?;

        Ok(())
    }

    pub fn get_current_file(&self) -> Result<HashMap<String, String>> {
        let mut files_hash = HashMap::new();
        let root = self
            .path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Calea nu are director parinte"))?;

        let ignore_patterns = self.get_ignored_files()?;
        let walker = WalkDir::new(root).into_iter().filter_entry(|e| {
            let path = e.path();

            if path.ends_with(".mygit") {
                return false;
            }

            let rel_path = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
            let rel_path = rel_path.replace('\\', "/");

            !self.is_ignored(&rel_path, &ignore_patterns)
        });

        for entry in walker {
            let ent = entry?;
            if ent.file_type().is_file() {
                let path = ent.path();
                let relative_path = path
                    .strip_prefix(root)?
                    .to_string_lossy()
                    .replace('\\', "/");
                let path_str = path.to_str().ok_or(anyhow::anyhow!("Path invalid"))?;
                let hash = self.create_blob(path_str)?;

                files_hash.insert(relative_path, hash);
            }
        }
        Ok(files_hash)
    }

    pub fn status(&self) -> Result<String> {
        let mut output = String::new();
        //extrag fisierele curente
        let current_files = self.get_current_file()?;
        //extrag fisierele de la ultimul commit
        let mut last_commit_files = HashMap::new();

        let refs = crate::refs::Refs::new(self.path.clone());
        if let Ok(Some(head_hash)) = refs.read_head() {
            //exista un hash, adica un commit
            let head_tree = self.get_tree_from_commit(&head_hash)?;
            last_commit_files = self.get_files(&head_tree)?;
        }
        output.push_str("\nSTATUS\n");
        let mut modified = false;
        let mut current_paths: Vec<_> = current_files.keys().collect();
        current_paths.sort();

        for path in current_paths {
            let current_hash = current_files
                .get(path)
                .ok_or_else(|| anyhow::anyhow!("Fisierul {} nu mai este in map", path))?;

            if let Some(head_hash) = last_commit_files.get(path) {
                if current_hash != head_hash {
                    output.push_str(&format!("MODIFICAT: {}\n", path.yellow()));
                    modified = true;
                }
            } else {
                output.push_str(&format!("NOU: {}\n", path.green()));
                modified = true;
            }
        }

        let mut last_commit_paths: Vec<_> = last_commit_files.keys().collect();
        last_commit_paths.sort();

        for path in last_commit_paths {
            if !current_files.contains_key(path) {
                output.push_str(&format!("STERS:  {}\n", path.red()));
                modified = true;
            }
        }

        if !modified {
            Ok("Nu s-a modificat nimic".to_string())
        } else {
            Ok(output)
        }
    }

    pub fn get_diff(&self, hash1: &str, hash2: &str) -> Result<()> {
        let h1 = hash1.chars().take(6).collect::<String>();
        let h2 = hash2.chars().take(6).collect::<String>();
        println!("Diff intre {} si {}", h1, h2);

        //extrag fisierele din ambele commit uri
        let tree1 = self.get_tree_from_commit(hash1)?;
        let files1 = self.get_files(&tree1)?;

        let tree2 = self.get_tree_from_commit(hash2)?;
        let files2 = self.get_files(&tree2)?;

        //extrag toate intrarile unice
        let mut paths: Vec<_> = files1.keys().chain(files2.keys()).collect();
        paths.sort();
        paths.dedup();

        for path in paths {
            let hash1 = files1.get(path);
            let hash2 = files2.get(path);

            match (hash1, hash2) {
                (Some(h1), Some(h2)) => {
                    if h1 != h2 {
                        println!("\n Modificat: {}", path.yellow().bold());
                        let (_, cont1) = crate::objects::read_object(&self.path, h1)?;
                        let (_, cont2) = crate::objects::read_object(&self.path, h2)?;
                        self.print_diff(&cont1, &cont2);
                    }
                }
                (Some(_), None) => {
                    println!("\n Sters: {}", path.red().bold());
                }
                (None, Some(_)) => {
                    println!("\n Nou:   {}", path.green().bold());
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn print_diff(&self, cont1: &[u8], cont2: &[u8]) {
        let text1 = String::from_utf8_lossy(cont1);
        let text2 = String::from_utf8_lossy(cont2);

        let diff = TextDiff::from_lines(&text1, &text2);

        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Delete => print!("{}{}", "- ".red(), change.value().red()),
                ChangeTag::Insert => print!("{}{}", "+ ".green(), change.value().green()),
                ChangeTag::Equal => print!("  {}", change.value().dimmed()),
            };
        }
    }

    fn get_ignored_files(&self) -> Result<Vec<Pattern>> {
        let root = self
            .path
            .parent()
            .ok_or(anyhow::anyhow!("Directorul nu are radacina"))?;
        let ign_path = root.join(".gitignore");
        let mut patterns: Vec<Pattern> = Vec::new();

        //reguli standard
        let basic_files = vec!["*.mygit*", "target", ".git"];

        for file in basic_files {
            if let Ok(p) = Pattern::new(file) {
                patterns.push(p);
            }
            // */file
            if let Ok(p) = Pattern::new(&format!("*/{}", file)) {
                patterns.push(p);
            }

            // file/*
            if let Ok(p) = Pattern::new(&format!("{}/*", file)) {
                patterns.push(p);
            }
        }

        //reguli din fisierul gitignore
        if let Ok(cont) = fs::read_to_string(ign_path) {
            for line in cont.lines() {
                let l = line.trim().trim_start_matches('\u{feff}');

                if l.is_empty() || l.starts_with('#') {
                    continue; //sarim peste liniile goale sau comentarii
                }

                if let Ok(p) = Pattern::new(l) {
                    patterns.push(p);
                }

                //verific si recursiv daca exista
                if let Ok(p) = Pattern::new(&format!("**/{}", l)) {
                    patterns.push(p);
                }
            }
        }
        Ok(patterns)
    }

    fn is_ignored(&self, path: &str, patterns: &[Pattern]) -> bool {
        for pattern in patterns {
            if pattern.matches(path) {
                return true;
            }
        }
        false
    }

    pub fn build_tree(&self, files: &HashMap<String, String>) -> Result<String> {
        let mut entries = Vec::new();

        let mut current_files: Vec<(&str, &str)> = Vec::new();
        let mut subdirs: HashMap<&str, HashMap<String, String>> = HashMap::new();

        for (path, hash) in files {
            if let Some((dir, rest)) = path.split_once('/') {
                subdirs
                    .entry(dir)
                    .or_default()
                    .insert(rest.to_string(), hash.clone());
            } else {
                current_files.push((path, hash));
            }
        }

        for (name, hash) in current_files {
            entries.push(TreeEntry {
                mode: "100644".to_string(),
                name: name.to_string(),
                hash: hex::decode(hash)?,
            });
        }

        for (dir_name, sub_files) in subdirs {
            let tree_hash_hex = self.build_tree(&sub_files)?;

            entries.push(TreeEntry {
                mode: "040000".to_string(),
                name: dir_name.to_string(),
                hash: hex::decode(tree_hash_hex)?,
            });
        }

        self.create_tree(entries)
    }

    pub fn commit_changes(&self, message: &str) -> Result<String> {
        let files_map = self.get_current_file()?;
        let tree_hash = self.build_tree(&files_map)?;

        let mut parents = Vec::new();
        let refs = crate::refs::Refs::new(self.path.clone());

        if let Ok(Some(head)) = refs.read_head() {
            parents.push(head);
        }

        let merge_head_path = self.path.join("MERGE_HEAD");

        if merge_head_path.exists() {
            let merge_hash = std::fs::read_to_string(&merge_head_path)?
                .trim()
                .to_string();
            parents.push(merge_hash);
            println!("Commit de tip Merge detectat.");
        }

        let commit_hash = self.commit(&tree_hash, parents, message)?;

        refs.update_head(&commit_hash)?;

        if merge_head_path.exists() {
            std::fs::remove_file(merge_head_path)?;
        }
        Ok(commit_hash)
    }

    fn get_parents_from_commits(&self, commit_hash: &str) -> Result<Vec<String>> {
        let (obj_type, cont) = objects::read_object(&self.path, commit_hash)?;

        if obj_type != "commit" {
            return Err(anyhow::anyhow!(
                "Obiectul {} nu este un commit",
                commit_hash
            ));
        }

        let str_cont = String::from_utf8(cont)?;
        let mut parents = Vec::new();

        for line in str_cont.lines() {
            if let Some(l) = line.strip_prefix("parent ") {
                parents.push(l.trim().to_string());
            } else if line.is_empty() {
                break;
            }
        }
        Ok(parents)
    }

    pub fn find_common_ancestor(&self, hash1: &str, hash2: &str) -> Result<Option<String>> {
        //stramosii din hash1
        let mut ancestors1 = HashSet::new();
        let mut queue1 = VecDeque::new();
        queue1.push_back(hash1.to_string());

        while let Some(current) = queue1.pop_front() {
            //daca nu se insereaza il avem deja in hashset
            if !ancestors1.insert(current.clone()) {
                continue;
            }

            let parents = self.get_parents_from_commits(&current)?;
            for p in parents {
                queue1.push_back(p);
            }
        }

        //stramosii din hash2

        let mut queue2 = VecDeque::new();
        queue2.push_back(hash2.to_string());
        let mut visited2: HashSet<String> = HashSet::new();

        while let Some(current) = queue2.pop_front() {
            if ancestors1.contains(&current) {
                return Ok(Some(current));
            }

            if !visited2.insert(current.clone()) {
                continue;
            }

            let parents = self.get_parents_from_commits(&current)?;
            for p in parents {
                queue2.push_back(p);
            }
        }

        Ok(None)
    }
}
