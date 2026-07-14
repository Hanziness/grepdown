# Testing Rust CLI Crates with Filesystem Access

## 1. Temporary Directory Management

### The assert-rs Ecosystem (Recommended)

The Rust CLI Working Group built the `assert-rs` family specifically for CLI testing. These crates compose together.

| Crate | Purpose | Version |
|-------|---------|---------|
| [`assert_cmd`](https://docs.rs/assert_cmd) | Run your binary and assert on stdout/stderr/exit code | 2.2 |
| [`assert_fs`](https://docs.rs/assert_fs) | Filesystem fixtures + assertions (superset of tempfile for tests) | 1.1 |
| [`predicates`](https://docs.rs/predicates) | Composable boolean assertions for strings, paths, files | 3.1 |

**Why `assert_fs` over bare `tempfile`:** `assert_fs::TempDir` is a drop-in for `tempfile::TempDir` but adds:
- `child("foo.txt")` — typed file proxy with `.write_str()`, `.touch()`, `.assert()`
- `TempDirAsTestDir` trait — copy fixture dirs into temp dirs
- Auto-cleanup on drop (same as tempfile)
- Built-in assertions: `temp.child("output.txt").assert(predicate::str::contains("expected"))`

```rust
// ❌ manual approach
let dir = tempfile::tempdir()?;
fs::write(dir.path().join("input.md"), "# Title\ncontent")?;
let output = Command::new("../target/debug/grepdown")
    .arg("index")
    .current_dir(dir.path())
    .output()?;
assert!(output.status.success());

// ✅ assert-rs way
use assert_cmd::cargo::cargo_bin_cmd;
use assert_fs::prelude::*;
use predicates::prelude::*;

let temp = assert_fs::TempDir::new()?;
temp.child("input.md").write_str("# Title\ncontent")?;

let mut cmd = cargo_bin_cmd!("grepdown");
cmd.arg("index").current_dir(temp.path());
cmd.assert().success();

temp.close()?; // explicit cleanup with error checking
```

### `cli_test_dir` (ripgrep/xsv pattern)

[`cli_test_dir`](https://docs.rs/cli_test_dir) packages the `WorkDir` pattern that BurntSushi used in ripgrep and xsv. It creates isolated test dirs with automatic naming, has convenience methods for creating files, and auto-cleans up.

```rust
use cli_test_dir::*;

#[test]
fn test_index() {
    let testdir = TestDir::new("grepdown", "index");
    testdir.create_file("doc.md", "# Hello\nworld");
    
    testdir.cmd()
        .arg("index")
        .expect_success();
    
    testdir.expect_path("md.db");
}
```

**Trade-off:** `cli_test_dir` is opinionated and less actively maintained. The `assert-rs` ecosystem is the de facto standard now — it's what the Rust CLI Book recommends.

### `tempfile` (bare-bones alternative)

If you want minimal dependencies, `tempfile::tempdir()` + `std::process::Command` + manual assertions works fine. This is what bat's integration tests use alongside `assert_cmd`.

```rust
let tmp_dir = tempfile::tempdir()?;
fs::write(tmp_dir.path().join("doc.md"), "# Title\ncontent")?;

bat()
    .arg("--paging=never")
    .arg(tmp_dir.path().join("doc.md"))
    .assert()
    .success()
    .stdout(predicate::str::contains("Title"));
```

### Recommendation for grepdown

Use **`assert_cmd` + `assert_fs` + `predicates`**. This is the standard trio from the Rust CLI Book, used by cargo itself, and gives you the richest assertion API.

```toml
[dev-dependencies]
assert_cmd = "2.2"
assert_fs = "1.1"
predicates = "3.1"
```

---

## 2. Integration Testing Structure

### Convention

Cargo finds integration tests in `tests/` at the crate root. Each `.rs` file becomes its own binary (compiled separately). This means:

- `tests/cli.rs` → one test binary
- `tests/fixtures.rs` → another test binary
- Files in `tests/common/mod.rs` → shared helpers (not a binary, modules only)

### Recommended layout for grepdown

```
crates/cli/
├── Cargo.toml
├── src/
│   ├── main.rs
│   └── cmd/
│       ├── init.rs
│       ├── lint.rs
│       ├── search.rs
│       └── mod.rs
└── tests/
    ├── common/
    │   └── mod.rs          # shared test helpers
    ├── init.rs             # init command tests
    ├── index.rs            # index command tests  
    ├── search.rs           # search command tests
    ├── lint.rs             # lint command tests
    ├── approve_edits.rs    # approve-edits tests
    └── e2e.rs              # full workflow tests
```

**Why split?** Each `tests/*.rs` is a separate crate. Splitting gives:
- Parallel test compilation
- Clean separation of concerns
- Shorter iteration when working on one command

**Caveat:** Each file compiles independently, so `common/mod.rs` won't cause dead-code warnings. fd handles this with `#![allow(dead_code)]` in shared modules.

### Shared test helpers (`tests/common/mod.rs`)

```rust
use assert_fs::TempDir;
use std::path::Path;

/// Create a temp dir with a basic grepdown project structure
pub fn setup_project() -> TempDir {
    let temp = TempDir::new().unwrap();
    temp.child("doc1.md")
        .write_str("# Document One\n\nContent of doc one.\n");
    temp.child("doc2.md")
        .write_str("# Document Two\n\nContent of doc two.\n");
    temp.child("subdir")
        .create_dir_all()
        .unwrap();
    temp.child("subdir/doc3.md")
        .write_str("# Nested Doc\n\nRefer to [[Document One]].\n");
    temp
}

/// Read a fixture file from tests/fixtures/
pub fn fixture(path: &str) -> String {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    std::fs::read_to_string(base.join(path)).unwrap()
}
```

---

## 3. Test Fixtures

### Approach A: Inline string constants (ripgrep style)

ripgrep defines test content as Rust string constants directly in test files:

```rust
const SHERLOCK: &str = "\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck.\n";

#[test]
fn test_search() {
    let temp = assert_fs::TempDir::new().unwrap();
    temp.child("sherlock.txt").write_str(SHERLOCK).unwrap();
    // ... assert
}
```

**Pros:** Self-contained, no hidden state.  
**Cons:** Gets unwieldy for large fixtures.

### Approach B: `tests/fixtures/` directory (fd/bat style)

Place `.md` files, sample projects, or expected output in `tests/fixtures/`:

```
tests/
├── fixtures/
│   ├── simple-project/
│   │   ├── doc1.md
│   │   ├── doc2.md
│   │   └── expected-output.json
│   ├── broken-links/
│   │   ├── doc1.md
│   │   └── doc2.md
│   └── sample-query-results.txt
```

```rust
#[test]
fn test_lint_broken_links() {
    let temp = assert_fs::TempDir::new().unwrap();
    temp.copy_from("tests/fixtures/broken-links/", &["*.md"]).unwrap();
    
    let mut cmd = assert_cmd::cargo_bin_cmd!("grepdown");
    cmd.arg("lint").current_dir(temp.path());
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("broken link"));
}
```

### Approach C: `assert_fs::fixture` (programmatic)

`assert_fs` itself has a fixture API that mirrors the directory tree in code:

```rust
use assert_fs::fixture::{FileWriteStr, ChildPath};

let temp = assert_fs::TempDir::new().unwrap();
temp.child("doc1.md").write_str("# Doc 1\nContent\n").unwrap();
temp.child("sub/doc2.md").write_str("# Doc 2\nSee [[Doc 1]]\n").unwrap();
```

### Recommendation for grepdown

**Use approach B (fixtures directory) for stable test data and approach C (programmatic) for variation.** Fixture files are easier to review and edit. Use programmatic creation when you need slight variations of the same base data.

---

## 4. End-to-End Testing

### Testing the full workflow (init → index → lint → approve-edits)

```rust
use assert_cmd::cargo::cargo_bin_cmd;
use assert_fs::prelude::*;
use predicates::prelude::*;

#[test]
fn full_workflow() {
    let temp = assert_fs::TempDir::new().unwrap();
    
    // Create initial documents
    temp.child("doc1.md").write_str("# Doc One\n\nContent of doc one.\n").unwrap();
    temp.child("doc2.md").write_str("# Doc Two\n\nSee [[Doc One]] for details.\n").unwrap();

    let run = |args: &[&str]| {
        let mut cmd = cargo_bin_cmd!("grepdown");
        cmd.args(args).current_dir(temp.path());
        cmd
    };

    // Step 1: Init
    run(&["init"]).assert().success();
    // Verify md.db was created
    temp.child("md.db").assert(predicate::path::exists());

    // Step 2: Index
    run(&["index"]).assert().success();

    // Step 3: Search
    run(&["search", "Doc One"])
        .assert()
        .success()
        .stdout(predicate::str::contains("doc1.md"))
        .stdout(predicate::str::contains("doc2.md")); // backlink

    // Step 4: Lint
    run(&["lint"]).assert().success();
}

#[test]
fn workflow_with_stale_references() {
    let temp = assert_fs::TempDir::new().unwrap();
    
    // Setup: doc1 references doc2, but doc2 doesn't exist
    temp.child("doc1.md").write_str("# Doc One\n\nSee [[Missing Doc]].\n").unwrap();

    let run = |args: &[&str]| {
        let mut cmd = cargo_bin_cmd!("grepdown");
        cmd.args(args).current_dir(temp.path());
        cmd
    };

    run(&["init"]).assert().success();
    run(&["index"]).assert().success();
    
    // Lint should catch the broken reference
    run(&["lint"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Missing Doc"));
    
    // Approve the stale reference
    run(&["approve-edits", "--all"])
        .assert()
        .success();
    
    // Lint should pass now
    run(&["lint"]).assert().success();
}
```

### Pattern: extracting the `run` helper

For tests with many command invocations, extracting a helper avoids repetition:

```rust
fn grepdown(temp: &assert_fs::TempDir) -> assert_cmd::Command {
    let mut cmd = cargo_bin_cmd!("grepdown");
    cmd.current_dir(temp.path());
    cmd
}

// Usage:
grepdown(&temp).arg("init").assert().success();
grepdown(&temp).arg("index").assert().success();
grepdown(&temp).args(["search", "query"]).assert().success();
```

---

## 5. Snapshot Testing

### Option A: [insta](https://docs.rs/insta) (by Armin Ronacher / mitsuhiko)

Best for testing structured data or CLI output you want to review visually.

```rust
use insta::assert_snapshot;

#[test]
fn test_search_output() {
    let temp = setup_project();
    // Run init + index
    grepdown(&temp).arg("init").assert().success();
    grepdown(&temp).arg("index").assert().success();
    
    let output = grepdown(&temp)
        .args(["search", "doc"])
        .output()
        .unwrap();
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    // First run: creates snapshots/search_output.snap
    // Subsequent runs: compares against snapshot
    insta::assert_snapshot!(stdout);
}
```

**Workflow:**
1. Write test, run `cargo test` → creates `.snap.new` files
2. Run `cargo insta review` → interactive review/accept/reject
3. Accepted snapshots stored as `.snap` files alongside tests

**Configuration** (in `Cargo.toml` or `.cargo/config.toml`):
```toml
[dev-dependencies]
insta = { version = "1", features = ["yaml"] }
```

**With glob for batch testing:**
```rust
#[test]
fn test_all_fixtures() {
    insta::glob!("tests/fixtures/*.md", |path| {
        let content = std::fs::read_to_string(path).unwrap();
        // Transform and snapshot
        insta::assert_snapshot!(content);
    });
}
```

### Option B: [trycmd](https://docs.rs/trycmd) (by epage / clap maintainer)

Snapshot testing specifically for CLIs. Test cases are `.trycmd` or `.md` files that embed commands and expected output.

```
tests/cmd/
├── init.trycmd
├── search.trycmd
└── lint.trycmd
```

**Example `tests/cmd/search.trycmd`:**
```console
$ grepdown search "Doc One"
doc1.md
  # Doc One
  Content of doc one.

doc2.md  
  # Doc Two
  See [[Doc One]] for details.
```

**Test runner:**
```rust
#[test]
fn cli_tests() {
    trycmd::TestCases::new()
        .default_bin_path(trycmd::cargo_bin!("grepdown"))
        .case("tests/cmd/*.trycmd");
}
```

**Workflow:**
1. `TRYCMD=dump cargo test` → generates `dump/` with `.stdout`/`.stderr` files
2. Copy desired outputs into `tests/cmd/` as `.trycmd` files
3. `TRYCMD=overwrite cargo test` → update existing snapshots

**trycmd advantages:**
- Test cases are human-readable prose files
- Can embed in README.md (literate testing)
- Automatic binary path resolution via `cargo_bin!`
- Supports exit code assertions (`? <status>`)
- Multiple commands in one file share the same temp dir

**trycmd disadvantages:**
- Less flexible than programmatic tests
- Harder to do complex setup/teardown
- Limited assertion capabilities compared to predicates

### Option C: Manual snapshot (bat's approach)

bat does manual snapshot testing — compare output to files in `tests/snapshots/`:

```rust
let actual = String::from_utf8_lossy(&output.stdout);
let expected = std::fs::read_to_string("tests/snapshots/search-output.txt").unwrap();
assert_eq!(expected, actual);
```

**Pros:** Zero dependencies.  
**Cons:** Manual maintenance; update means editing text files.

### Recommendation

- **insta** for snapshot testing complex data structures or variable-length output
- **trycmd** for testing CLI output where you want human-readable, reviewable test cases
- **Manual comparison** if you want zero extra dependencies

For grepdown, **trycmd** is a good fit for the happy-path tests (test cases double as documentation). Use **assert_cmd + predicates** for edge cases and error paths.

---

## 6. Real-World Reference Table

| Tool | Test approach | Temp dirs | Assertions | Fixtures |
|------|--------------|-----------|------------|----------|
| **ripgrep** | Custom `Dir` + `TestCommand` struct, `rgtest!` macro | `tempdir()` with global counter | `eqnice!()` macro (manual comparison) | Inline string constants |
| **bat** | `assert_cmd` + `tempfile` | `tempdir()` wrapped in helpers | `predicates` for most, manual `.snap` files for snapshots | Directory `tests/snapshots/` |
| **fd** | Shell script → migrating to Rust tests | `mktemp -d` in shell, `tempfile` in Rust | diff-based | Inline in test script |
| **cargo** | `assert_cmd` + custom test support | `TempDir` from `cargo-test-support` | `predicates` | `tests/testsuite/` fixtures |

---

## 7. Immediate Recommendations for grepdown

### Dependencies to add

```toml
[dev-dependencies]
assert_cmd = "2.2"
assert_fs = "1.1"
predicates = "3.1"
```

### Files to create

```
crates/cli/tests/
├── common/
│   └── mod.rs          # setup_project(), fixture(), run helper
├── init.rs             # init creates md.db
├── index.rs            # indexing documents
├── search.rs           # search output verification
├── lint.rs             # lint catches broken links
├── approve_edits.rs    # approving stale references
├── e2e.rs              # full workflow: init→index→lint→approve
└── fixtures/           # (optional) test data files
    └── sample-project/
        ├── doc1.md
        └── doc2.md
```

### Quick-start test template (`tests/common/mod.rs`)

```rust
use assert_cmd::cargo::cargo_bin_cmd;
use assert_fs::TempDir;

pub fn setup_basic_project() -> TempDir {
    let temp = TempDir::new().unwrap();
    temp.child("doc1.md").write_str("# Alpha\nContent A\n").unwrap();
    temp.child("doc2.md").write_str("# Beta\nSee [[Alpha]]\n").unwrap();
    temp
}

pub fn grepdown(temp: &TempDir) -> assert_cmd::Command {
    let mut cmd = cargo_bin_cmd!("grepdown");
    cmd.current_dir(temp.path());
    cmd
}
```

### Example test (`tests/search.rs`)

```rust
mod common;

#[test]
fn search_finds_document_by_title() {
    let temp = common::setup_basic_project();
    common::grepdown(&temp).arg("init").assert().success();
    common::grepdown(&temp).arg("index").assert().success();

    common::grepdown(&temp)
        .args(["search", "Alpha"])
        .assert()
        .success()
        .stdout(predicates::str::contains("doc1.md"))
        .stdout(predicates::str::contains("Alpha"));
}
```

### Key gotchas

1. **`cargo_bin_cmd!` is macro, `cargo_bin` is fn.** The macro version (`cargo_bin_cmd!`) returns a pre-configured `Command`. The fn version returns a `Result<Command>`. Use the macro — it panics at compile time if the binary doesn't exist.

2. **`current_dir` vs `arg` path:** Set `current_dir(temp.path())` so the binary runs with the temp dir as CWD (like real usage). Pass explicit file paths via `.arg()`.

3. **TempDir cleanup on Windows:** Files must not be held open when the temp dir drops. Use `.close()?` for explicit error-checked cleanup if your test spawns child processes.

4. **Parallel test safety:** Each test creates its own `TempDir`. Tests run in parallel by default. No shared state issues.

5. **Debugging failures:** `TempDir` has `into_persistent()` and `keep()` methods to prevent cleanup when debugging. Or set the env var `assert_fs::fixture::TempDir::keep_on_failure()`.
