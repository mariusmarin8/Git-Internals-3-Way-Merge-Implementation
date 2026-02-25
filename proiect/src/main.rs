mod objects;
mod refs;
mod repository;

use anyhow::Result;
use std::env;
use std::fs;
fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Comenzi: init, commit <mesaj>, branch <nume>, log");
        return Ok(());
    }

    let command = &args[1];
    let current_dir = env::current_dir()?;

    let path_str = current_dir
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Calea curenta are caractere invalide UTF-8"))?;

    let repo = if command == "init" {
        repository::Repository::init(path_str)?
    } else {
        repository::Repository {
            path: current_dir.join(".mygit"),
        }
    };

    let refs = refs::Refs::new(repo.path.clone());

    match command.as_str() {
        "help" => {
            println!("Lista de Comenzi:\n");
            println!("init");
            println!("status");
            println!("commit <mesaj>");
            println!("log");
            println!("branch <nume>");
            println!("checkout <nume|hash>");
            println!("merge <branch>");
            println!("diff <target>");
            println!("diff <sursa> <tinta>");
            println!("help");
            println!();
        }

        "init" => {
            println!("Repo inițializat (.mygit)");
        }

        "branch" => {
            if args.len() < 3 {
                println!("Foloseste: branch <nume>");
                return Ok(());
            }
            let name = &args[2];

            let head = match refs.read_head()? {
                Some(h) => h,
                None => {
                    println!("Nu exista commit-uri inca, nu poate crea branch.");
                    return Ok(());
                }
            };

            refs.create_branch(name, &head)?;
            println!("Branch '{}' creat la {}", name, head);
        }

        "log" => {
            let mut current = refs.read_head()?;

            println!("Istoric commit-uri:\n");

            while let Some(hash) = current {
                let (obj_type, content) = objects::read_object(&repo.path, &hash)?;

                if obj_type != "commit" {
                    println!("HEAD nu indică un commit valid.");
                    break;
                }

                let content = String::from_utf8(content)?;

                println!("commit {}", hash);

                let mut parent = None;
                let mut message = String::new();
                let mut in_header = true;

                for line in content.lines() {
                    if in_header {
                        if let Some(l) = line.strip_prefix("parent ") {
                            parent = Some(l.to_string());
                        } else if line.is_empty() {
                            in_header = false;
                        }
                    } else {
                        message.push_str(line);
                        message.push('\n');
                    }
                }

                println!("    {}\n", message.trim());
                current = parent;
            }
        }

        "commit" => {
            if args.len() < 3 {
                println!("Utilizare: commit <mesaj>");
                return Ok(());
            }
            let message = &args[2];
            match repo.commit_changes(message) {
                Ok(hash) => println!("Commit creat: {}", hash),
                Err(e) => println!("Eroare la commit: {}", e),
            }
        }

        "checkout" => {
            if args.len() < 3 {
                println!("Utilizare: checkout <branch | commit-hash>");
                return Ok(());
            }

            let target = &args[2];
            let branch_path = repo.path.join("refs/heads").join(target);

            let (head_value, commit_hash) = if branch_path.exists() {
                println!("Comut pe branch-ul '{}'...", target);

                (
                    format!("ref: refs/heads/{}", target),
                    fs::read_to_string(&branch_path)?.trim().to_string(),
                )
            } else {
                println!("Comut pe commit-ul '{}' (Detached HEAD)...", target);

                (target.to_string(), target.to_string())
            };

            fs::write(repo.path.join("HEAD"), head_value)?;

            println!("Restaurez fișierele...");
            repo.checkout(&commit_hash)
                .map(|_| println!("Workspace actualizat la {}", commit_hash))
                .unwrap_or_else(|e| {
                    println!("Eroare la checkout: {}", e);
                    println!("Sugestie: Verifică dacă branch-ul sau hash-ul există.");
                });
        }

        "merge" => {
            if args.len() < 3 {
                println!("Utilizare: merge <nume_branch>");
                return Ok(());
            }
            let target_name = &args[2];

            let head_hash = match refs.read_head()? {
                Some(h) => h,
                None => {
                    println!("Trebuie sa ai minim un commit pentru a face merge.");
                    return Ok(());
                }
            };

            let target_path = repo.path.join("refs").join("heads").join(target_name);
            if !target_path.exists() {
                println!("Branch-ul '{}' nu există!", target_name);
                return Ok(());
            }
            let target_hash = std::fs::read_to_string(target_path)?.trim().to_string();

            if head_hash == target_hash {
                println!("Deja este actualizat");
                return Ok(());
            }

            let ancestor = repo.find_common_ancestor(&head_hash, &target_hash)?;
            let base_hash = match ancestor {
                Some(h) => h,
                None => {
                    println!("Eroare: Istoricuri neinrudite.");
                    return Ok(());
                }
            };

            if base_hash == head_hash {
                println!("Merge direct...");
                refs.update_head(&target_hash)?;
                repo.checkout(&target_hash)?;
                println!("Actualizat la {} fara commit.", target_name);
                return Ok(());
            }

            if base_hash == target_hash {
                println!("Branch-ul '{}' este deja actualizat.", target_name);
                return Ok(());
            }

            let display_base = base_hash.get(..7).unwrap_or(&base_hash); //in cazul in care commit ul este prea scurt

            println!("Am aplicat metoda 3-WAY (Base: {}...)", display_base);

            let head_tree = repo.get_tree_from_commit(&head_hash)?;
            let target_tree = repo.get_tree_from_commit(&target_hash)?;
            let base_tree = repo.get_tree_from_commit(&base_hash)?;

            let head_files = repo.get_files(&head_tree)?;
            let target_files = repo.get_files(&target_tree)?;
            let base_files = repo.get_files(&base_tree)?;

            let mut all_paths = std::collections::HashSet::new();
            for p in head_files.keys() {
                all_paths.insert(p);
            }
            for p in target_files.keys() {
                all_paths.insert(p);
            }
            for p in base_files.keys() {
                all_paths.insert(p);
            }

            let root = match repo.path.parent() {
                Some(p) => p,
                None => {
                    println!("Nu pot determina directorul parinte al repo-ului.");
                    return Ok(());
                }
            };
            let mut conflicts = false;

            for path in all_paths {
                let file_path = root.join(path);
                let h_hash = head_files.get(path);
                let t_hash = target_files.get(path);
                let b_hash = base_files.get(path);

                if h_hash == t_hash {
                    continue;
                }

                if h_hash == b_hash {
                    if let Some(t_val) = t_hash {
                        if let Some(parent) = file_path.parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                        let (_, content) = objects::read_object(&repo.path, t_val)?;
                        std::fs::write(&file_path, content)?;
                    } else if file_path.exists() {
                        std::fs::remove_file(&file_path)?;
                    }
                } else if t_hash == b_hash {
                    continue;
                } else {
                    println!("CONFLICT: {}", path);
                    conflicts = true;

                    let h_val = h_hash.unwrap_or(&String::new()).clone();
                    let t_val = t_hash.unwrap_or(&String::new()).clone();

                    if !h_val.is_empty() && !t_val.is_empty() {
                        repo.write_conflict_file(&file_path, &h_val, &t_val)?;
                    } else {
                        println!("Conflict la {}", path);
                    }
                }
            }

            if conflicts {
                println!("Merge oprit din cauza conflictelor. Rezolva manual și da commit.");

                let merge_head_path = repo.path.join("MERGE_HEAD");

                std::fs::write(&merge_head_path, &target_hash)?;

                if !merge_head_path.exists() {
                    println!("Fisierul MERGE_HEAD NU a fost creat!");
                }
            } else {
                let current_files = repo.get_current_file()?;
                let new_root_tree = repo.build_tree(&current_files)?;
                let parents = vec![head_hash, target_hash.clone()];
                let msg = format!("Merge branch '{}'", target_name);

                let merge_commit = repo.commit(&new_root_tree, parents, &msg)?;
                refs.update_head(&merge_commit)?;
                println!("Merge realizat cu succes! Commit: {}", merge_commit);
            }
        }

        "status" => match repo.status() {
            Ok(msg) => println!("{}", msg),
            Err(e) => eprintln!("Eroare la status: {}", e),
        },

        "diff" => {
            if args.len() < 3 {
                println!("Utilizare: \n  diff <target> (HEAD vs Target)\n  diff <source> <target>");
                return Ok(());
            }

            let hash1;
            let hash2;

            if args.len() == 3 {
                //dif target
                let target = &args[2];
                hash1 = match refs.read_head()? {
                    Some(h) => h,
                    None => {
                        println!("Nu exista commit-uri pentru a compara.");
                        return Ok(());
                    }
                };
                let target_path = repo.path.join("refs").join("heads").join(target);

                hash2 = if target_path.exists() {
                    std::fs::read_to_string(target_path)?.trim().to_string()
                } else {
                    //este un hash
                    target.to_string()
                };
            } else {
                //dif source targe
                let src = &args[2];
                let target = &args[3];

                let src_path = repo.path.join("refs").join("heads").join(src);
                hash1 = if src_path.exists() {
                    std::fs::read_to_string(src_path)?.trim().to_string()
                } else {
                    src.to_string()
                };

                let target_path = repo.path.join("refs").join("heads").join(target);
                hash2 = if target_path.exists() {
                    std::fs::read_to_string(target_path)?.trim().to_string()
                } else {
                    target.to_string()
                };
            }
            repo.get_diff(&hash1, &hash2)?;
        }

        _ => {
            println!("Comandă necunoscută: {}", command);
        }
    }

    Ok(())
}
