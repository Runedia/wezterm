#![windows_subsystem = "windows"] // 콘솔 창을 띄우지 않는다(GUI subsystem)
//! VS Code 외부 터미널 등에서 호출되는 콘솔 없는 wezterm 런처.
//!
//! 호출자(VS Code)는 이 실행 파일을 활성 폴더의 작업 디렉토리에서 spawn 한다.
//! 그 작업 디렉토리를 `--cwd` 로 명시 주입해 `wezterm-gui` 를 새 창으로 띄운다.
//! wezterm 은 cwd 미지정 시 부모 cwd 가 아니라 `%USERPROFILE%` 로 fallback 하므로
//! (pty/src/cmdbuilder.rs 의 `current_directory`), `--cwd` 를 명시해야만 한다.

use std::env;
use std::path::PathBuf;
use std::process::Command;

/// 호출할 `wezterm-gui.exe` 경로를 결정한다.
/// 설치본·빌드본 모두 이 런처와 같은 디렉토리에 위치하므로 그것을 우선 사용하고,
/// 찾지 못하면 PATH 에 위임한다.
fn wezterm_gui_path() -> PathBuf {
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("wezterm-gui.exe");
            if candidate.exists() {
                return candidate;
            }
        }
    }
    PathBuf::from("wezterm-gui.exe")
}

fn main() {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // shell 미경유, .spawn() 으로 분리 실행(종료를 기다리지 않음).
    // 핸들을 drop 하고 즉시 반환하면 wezterm 은 독립 프로세스로 살아남는다.
    let _ = Command::new(wezterm_gui_path())
        .arg("start")
        .arg("--cwd")
        .arg(&cwd)
        .spawn();
}
