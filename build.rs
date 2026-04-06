fn main() {
    let output_dir = std::env::var("OUT_DIR").unwrap();
    let commit_hash_path = std::path::Path::new(&output_dir).join("commit_hash");

    let commit_hash = format!("{}", get_commit_hash());

    std::fs::write(commit_hash_path, commit_hash).unwrap();
}

fn get_commit_hash() -> String {
    let output = std::process::Command::new("git")
        .arg("log").arg("-1")
        .arg("--pretty=format:%h")
        .arg("--abbrev=10")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    if output.status.success() {
        String::from_utf8_lossy(&output.stdout).to_string()
    } else {
        "unspecified".to_string()
    }
}
