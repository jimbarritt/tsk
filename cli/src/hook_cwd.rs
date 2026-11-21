use std::path::Path;

use crate::thread::session_start::{HookFormat, HookInput};

pub fn enter(format: HookFormat, bytes: &[u8]) -> bool {
    if format != HookFormat::Copilot {
        return false;
    }
    let Some(cwd) = HookInput::parse(bytes).cwd else {
        return false;
    };
    enter_dir(Path::new(&cwd))
}

fn enter_dir(dir: &Path) -> bool {
    dir.is_absolute() && dir.is_dir() && std::env::set_current_dir(dir).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    static CWD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn input(cwd: &Path) -> Vec<u8> {
        serde_json::json!({"sessionId": "s1", "cwd": cwd})
            .to_string()
            .into_bytes()
    }

    #[test]
    fn copilot_input_changes_the_directory() {
        let _guard = CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let before = std::env::current_dir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let expected = target.path().canonicalize().unwrap();
        assert!(enter(HookFormat::Copilot, &input(&expected)));
        assert_eq!(
            std::env::current_dir().unwrap().canonicalize().unwrap(),
            expected
        );
        std::env::set_current_dir(before).unwrap();
    }

    #[test]
    fn claude_input_keeps_the_directory() {
        let _guard = CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let before = std::env::current_dir().unwrap();
        let target = tempfile::tempdir().unwrap();
        assert!(!enter(HookFormat::Claude, &input(target.path())));
        assert_eq!(std::env::current_dir().unwrap(), before);
    }

    #[test]
    fn a_missing_or_unusable_cwd_keeps_the_directory() {
        let _guard = CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let before = std::env::current_dir().unwrap();
        let gone = tempfile::tempdir().unwrap().path().join("gone");
        assert!(!enter(HookFormat::Copilot, &input(&gone)));
        assert!(!enter(HookFormat::Copilot, b"{\"sessionId\":\"s1\"}"));
        assert!(!enter(HookFormat::Copilot, b"{\"cwd\":\"relative/dir\"}"));
        assert!(!enter(HookFormat::Copilot, b"not json"));
        assert_eq!(std::env::current_dir().unwrap(), before);
    }
}
