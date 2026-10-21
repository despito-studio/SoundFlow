//! Іконка застосунку на робочому столі.
use std::{path::PathBuf, process::Command};

fn shortcut_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(profile) = std::env::var("USERPROFILE") {
        paths.push(PathBuf::from(&profile).join("Desktop\\SoundFlow.lnk"));
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        let programs = PathBuf::from(appdata).join("Microsoft\\Windows\\Start Menu\\Programs");
        paths.push(programs.join("SoundFlow.lnk"));
        paths.push(programs.join("SoundFlow").join("SoundFlow.lnk"));
    }
    paths.into_iter().filter(|path| path.is_file()).collect()
}

fn icon_path(name: &str, app_dir: Option<&std::path::Path>) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    if name == "default" {
        return Ok(exe.display().to_string());
    }
    let directory = app_dir
        .map(std::path::Path::to_path_buf)
        .or_else(|| exe.parent().map(std::path::Path::to_path_buf))
        .ok_or("Не вдалося визначити каталог застосунку")?;
    let candidate = directory.join("resources").join(format!("icon-{name}.ico"));
    if candidate.is_file() {
        Ok(candidate.display().to_string())
    } else {
        Err("Ця іконка відсутня в установці".into())
    }
}

/// Переписує ярлики SoundFlow так, щоб вони показували вибрану іконку.
pub fn set_desktop_icon(name: &str) -> Result<usize, String> {
    set_icon_from(name, None)
}

fn set_icon_from(name: &str, app_dir: Option<&std::path::Path>) -> Result<usize, String> {
    let icon = icon_path(name, app_dir)?;
    let location = format!("{icon},0");
    let shortcuts = shortcut_paths();
    if shortcuts.is_empty() {
        return Err("Ярлик SoundFlow не знайдено".into());
    }
    let mut updated = 0;
    for path in shortcuts {
        let script = format!(
            "$shell = New-Object -ComObject WScript.Shell; $link = $shell.CreateShortcut('{}'); $link.IconLocation = '{}'; $link.Save()",
            path.display(),
            location
        );
        let mut command = Command::new("powershell");
        command.args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &script,
        ]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        if command.status().map(|status| status.success()).unwrap_or(false) {
            updated += 1;
        }
    }
    if updated == 0 {
        return Err("Не вдалося оновити ярлик".into());
    }
    Ok(updated)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    #[ignore = "changes the real desktop shortcut"]
    fn switches_the_desktop_icon() {
        let project = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let updated = set_icon_from("rose", Some(project)).expect("update shortcut");
        assert!(updated >= 1);
        let updated = set_icon_from("default", Some(project)).expect("restore shortcut");
        assert!(updated >= 1);
    }
}
