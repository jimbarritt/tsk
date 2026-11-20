use std::path::Path;
use std::process::Output;

fn tsk(config: &Path, args: &[&str]) -> Output {
    crate::common::cmd(env!("CARGO_BIN_EXE_tsk"))
        .env("XDG_CONFIG_HOME", config)
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn config_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tsk-features-it-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn features_print_none_when_the_config_has_none() {
    let dir = config_dir("none");
    let output = tsk(&dir, &["config", "features"]);
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("features enabled: none"), "{}", text);
    assert!(
        text.contains("known features: software-english"),
        "{}",
        text
    );
}

#[test]
fn features_write_the_list_and_print_it_back() {
    let dir = config_dir("write");
    let file = dir.join("tsk").join("config.toml");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "# kept\n[ledger]\nserver = \"off\"\n").unwrap();
    let written = tsk(&dir, &["config", "features", "software-english"]);
    assert!(written.status.success());
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("# kept"), "{}", text);
    assert!(text.contains("[features]"), "{}", text);
    assert!(
        text.contains("enabled = [\"software-english\"]"),
        "{}",
        text
    );
    assert!(
        stdout(&tsk(&dir, &["config", "features"])).contains("features enabled: software-english")
    );
    let cleared = tsk(&dir, &["config", "features", "--none"]);
    assert!(cleared.status.success());
    assert!(stdout(&tsk(&dir, &["config", "features"])).contains("features enabled: none"));
}

#[test]
fn an_unknown_feature_is_refused_and_writes_nothing() {
    let dir = config_dir("unknown");
    let output = tsk(&dir, &["config", "features", "nope"]);
    assert!(!output.status.success());
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(err.contains("unknown feature nope"), "{}", err);
    assert!(!dir.join("tsk").join("config.toml").exists());
}
