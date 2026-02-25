# Git-Internals-3-Way-Merge-Implementation
Local version control application written in Rust. Implements Git internals including secure commits, diffs, .gitignore logic, and branch management.
## Architecture and Internals

Unlike simple file-backup scripts, this project replicates the core data structures used by Git:

* **Content-Addressable Storage:** Every file, directory state, and commit is hashed using SHA-1. The resulting hash determines how and where the object is stored inside the `.mygit/objects` directory.
* **Object Types:**
    * `blob`: Represents file content.
    * `tree`: Represents a directory structure, containing references (hashes) to blobs and other trees.
    * `commit`: Represents a snapshot of the working directory, containing a reference to the root tree, author metadata, and references to parent commits.
* **Storage Compression:** To optimize disk usage, all objects are compressed using Zlib (`flate2` crate) before being written to disk.
* **Reference Management:** Branches are simply text files pointing to a specific commit hash. The system safely handles the `HEAD` reference, including detached HEAD states.

## Key Features

* **3-Way Merge Algorithm:** Instead of a simple fast-forward merge, the application calculates the Lowest Common Ancestor (LCA) between two branches using Breadth-First Search (BFS). It performs a 3-way merge, detects file conflicts, and automatically injects standard conflict markers (`<<<<<<< HEAD`, `=======`, `>>>>>>>`) for manual resolution.
* **Status and Diffing:** Evaluates the working tree against the current commit. It tracks modified, new, and deleted files. The `diff` command displays line-by-line changes using colored terminal output (powered by the `similar` and `colored` crates).
* **Ignore Logic:** Implements `.gitignore` functionality. It parses ignore rules and uses pattern matching (`glob`) alongside recursive directory traversal (`walkdir`) to exclude specific files or build folders.
* **Branching:** Full support for creating, switching, and merging multiple branches.

## Command Line Interface

The application supports the following CLI commands:

* `init` - Initializes an empty repository and creates the `.mygit` hidden directory structure.
* `status` - Displays the current state of the working directory (new, modified, deleted files).
* `commit <message>` - Snapshots the tracked files and creates a new commit object.
* `log` - Traverses the commit history backwards from HEAD.
* `branch <name>` - Creates a new branch pointing to the current commit.
* `checkout <branch_name | commit_hash>` - Updates files in the working tree to match the specified branch or commit.
* `merge <branch_name>` - Joins the specified branch history into the current branch.
* `diff <target>` or `diff <source> <target>` - Shows line-by-line file differences.

## Build and Run

### Prerequisites
* Rust toolchain (Edition 2024)
* Cargo package manager

### Instructions

1.  Clone the repository:
    ```bash
    git clone [YOUR_GITHUB_REPO_LINK]
    cd [YOUR_REPO_NAME]
    ```

2.  Build the executable in release mode:
    ```bash
    cargo build --release
    ```

3.  Run the application using Cargo:
    ```bash
    cargo run -- init
    cargo run -- commit "Initial commit"
    cargo run -- status
    ```
    *Alternatively, you can run the compiled binary directly from `./target/release/proiect`.*
