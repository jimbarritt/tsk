use std::path::Path;
use std::process::Output;

const PUBLIC: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExampleExampleExampleExampleExampleExample";

struct Setup {
    dir: tempfile::TempDir,
}

impl Setup {
    fn new(config: &str, ssh_add: &str) -> Setup {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join("config").join("tsk");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("config.toml"), config).unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        write_script(&bin.join("ssh-add"), ssh_add);
        std::fs::create_dir_all(dir.path().join("work")).unwrap();
        Setup { dir }
    }

    fn seen(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("seen"))
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    fn tsk(&self, args: &[&str]) -> Output {
        let path = format!(
            "{}:{}",
            self.dir.path().join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        crate::common::cmd(env!("CARGO_BIN_EXE_tsk"))
            .args(args)
            .current_dir(self.dir.path().join("work"))
            .env("HOME", self.dir.path())
            .env("XDG_CONFIG_HOME", self.dir.path().join("config"))
            .env("XDG_STATE_HOME", self.dir.path().join("state"))
            .env("TSK_HOME", self.dir.path().join(".tsk"))
            .env("TSK_NO_UPDATE_CHECK", "1")
            .env("TSK_AUTO_UPDATE", "never")
            .env("GIT_CEILING_DIRECTORIES", self.dir.path())
            .env("SSH_AUTH_SOCK", "/shell/agent.sock")
            .env("PATH", path)
            .env_remove("TSK_ATTRIBUTION_SIGNING_KEY")
            .env_remove("TSK_ATTRIBUTION_IDENTITY")
            .output()
            .unwrap()
    }
}

fn write_script(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, format!("#!/bin/sh\n{}", body)).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn config(socket: Option<&str>) -> String {
    let mut text = format!(
        "[attribution]\nidentity = \"robot\"\nemail = \"r@example.test\"\nsigning_key = \"agent:{}\"\n",
        PUBLIC
    );
    if let Some(socket) = socket {
        text.push_str(&format!("ssh_auth_sock = \"{}\"\n", socket));
    }
    text
}

fn ssh_add_listing(setup_dir: &str, key_line: &str, code: i32) -> String {
    format!(
        "echo \"$SSH_AUTH_SOCK\" > '{}/seen'\necho '{}'\nexit {}\n",
        setup_dir, key_line, code
    )
}

fn text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn signing_check_asks_the_configured_socket_and_exits_0_when_the_key_is_listed() {
    let setup = Setup::new(&config(Some("/configured/agent.sock")), "");
    let dir_path = setup.dir.path().display().to_string();
    write_script(
        &setup.dir.path().join("bin").join("ssh-add"),
        &ssh_add_listing(&dir_path, &format!("{} robot", PUBLIC), 0),
    );
    let output = setup.tsk(&["signing", "check"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert!(text(&output).contains("available"), "{}", text(&output));
    assert_eq!(setup.seen(), "/configured/agent.sock");
}

#[test]
fn signing_check_exits_1_and_says_to_unlock_1password_when_the_agent_holds_no_keys() {
    let setup = Setup::new(&config(Some("/configured/agent.sock")), "");
    let dir_path = setup.dir.path().display().to_string();
    write_script(
        &setup.dir.path().join("bin").join("ssh-add"),
        &ssh_add_listing(&dir_path, "", 1),
    );
    let output = setup.tsk(&["signing", "check"]);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output));
    assert!(
        text(&output).contains("unlock 1Password"),
        "{}",
        text(&output)
    );
    assert_eq!(setup.seen(), "/configured/agent.sock");
}

#[test]
fn signing_check_exits_1_when_the_agent_lists_other_keys_only() {
    let setup = Setup::new(&config(Some("/configured/agent.sock")), "");
    let dir_path = setup.dir.path().display().to_string();
    write_script(
        &setup.dir.path().join("bin").join("ssh-add"),
        &ssh_add_listing(
            &dir_path,
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOtherOtherOther other",
            0,
        ),
    );
    let output = setup.tsk(&["signing", "check"]);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output));
    assert!(
        text(&output).contains("not the robot signing key"),
        "{}",
        text(&output)
    );
}

#[test]
fn signing_check_exits_1_when_no_agent_answers() {
    let setup = Setup::new(&config(Some("/configured/agent.sock")), "exit 2\n");
    let output = setup.tsk(&["signing", "check"]);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output));
    assert!(
        text(&output).contains("could not reach"),
        "{}",
        text(&output)
    );
}

#[test]
fn signing_check_uses_the_process_socket_when_none_is_configured() {
    let setup = Setup::new(&config(None), "");
    let dir_path = setup.dir.path().display().to_string();
    write_script(
        &setup.dir.path().join("bin").join("ssh-add"),
        &ssh_add_listing(&dir_path, &format!("{} robot", PUBLIC), 0),
    );
    let output = setup.tsk(&["signing", "check"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert_eq!(setup.seen(), "/shell/agent.sock");
}

#[test]
fn signing_check_exits_1_when_no_signing_key_is_configured() {
    let setup = Setup::new("[attribution]\nidentity = \"robot\"\n", "exit 0\n");
    let output = setup.tsk(&["signing", "check"]);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output));
    assert!(
        text(&output).contains("signing_key is not set"),
        "{}",
        text(&output)
    );
}

#[test]
fn the_ssh_auth_sock_setting_is_written_to_the_user_config_and_printed_with_its_source() {
    let setup = Setup::new("", "exit 0\n");
    let output = setup.tsk(&[
        "config",
        "attribution",
        "--user",
        "--ssh-auth-sock",
        "/written/agent.sock",
    ]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let out = text(&output);
    assert!(
        out.contains("attribution ssh auth sock: /written/agent.sock"),
        "{}",
        out
    );
    assert!(out.contains("source: user config"), "{}", out);
    let file = std::fs::read_to_string(setup.dir.path().join("config/tsk/config.toml")).unwrap();
    assert!(
        file.contains("ssh_auth_sock = \"/written/agent.sock\""),
        "{}",
        file
    );
}

#[cfg(target_os = "macos")]
#[test]
fn the_ssh_auth_sock_defaults_to_the_one_password_socket_when_that_file_exists() {
    let setup = Setup::new("", "exit 0\n");
    let none = setup.tsk(&["config", "attribution", "--user"]);
    assert!(
        text(&none).contains("attribution ssh auth sock: not set"),
        "{}",
        text(&none)
    );
    let socket = setup
        .dir
        .path()
        .join("Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock");
    std::fs::create_dir_all(socket.parent().unwrap()).unwrap();
    std::fs::write(&socket, "").unwrap();
    let output = setup.tsk(&["config", "attribution", "--user"]);
    let out = text(&output);
    assert!(
        out.contains(&format!("attribution ssh auth sock: {}", socket.display())),
        "{}",
        out
    );
    assert!(out.contains("source: default"), "{}", out);
}
