//! Іконка застосунку на робочому столі.
use std::{path::PathBuf, process::Command};

fn run_powershell(script: &str) -> Result<std::process::Output, String> {
    let mut encoded = Vec::with_capacity(script.len() * 2);
    for unit in script.encode_utf16() {
        encoded.extend_from_slice(&unit.to_le_bytes());
    }
    let payload = base64(&encoded);
    let mut command = Command::new("powershell");
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-WindowStyle",
        "Hidden",
        "-EncodedCommand",
        &payload,
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command.output().map_err(|error| error.to_string())
}

fn ps_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn current_icon(path: &std::path::Path) -> Option<String> {
    let script = format!(
        "$shell = New-Object -ComObject WScript.Shell; $shell.CreateShortcut({}).IconLocation",
        ps_string(&path.to_string_lossy())
    );
    let output = run_powershell(&script).ok()?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn base64(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let first = chunk[0] as u32;
        let second = *chunk.get(1).unwrap_or(&0) as u32;
        let third = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (first << 16) | (second << 8) | third;
        output.push(ALPHABET[(triple >> 18) as usize & 63] as char);
        output.push(ALPHABET[(triple >> 12) as usize & 63] as char);
        output.push(if chunk.len() > 1 {
            ALPHABET[(triple >> 6) as usize & 63] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            ALPHABET[triple as usize & 63] as char
        } else {
            '='
        });
    }
    output
}

fn shortcut_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(desktop) = dirs::desktop_dir() {
        paths.push(desktop.join("SoundFlow.lnk"));
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        let programs = PathBuf::from(appdata).join("Microsoft\\Windows\\Start Menu\\Programs");
        paths.push(programs.join("SoundFlow.lnk"));
        paths.push(programs.join("SoundFlow").join("SoundFlow.lnk"));
    }
    paths.into_iter().filter(|path| path.is_file()).collect()
}

fn icon_path(name: &str, app_dir: Option<&std::path::Path>) -> Result<String, String> {
    if !["default", "rose", "violet", "ocean"].contains(&name) {
        return Err("Невідома іконка".into());
    }
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
        return Ok(0);
    }
    let mut updated = 0;
    let mut last_error = String::new();
    for path in shortcuts {
        let script = format!(
            "$shell = New-Object -ComObject WScript.Shell; $link = $shell.CreateShortcut({}); $link.IconLocation = {}; $link.Save()",
            ps_string(&path.to_string_lossy()),
            ps_string(&location)
        );
        match run_powershell(&script) {
            Ok(_) => {
                // PowerShell інколи завершується з ненульовим кодом, хоча ярлик
                // уже переписано, тому перевіряємо результат читанням.
                if current_icon(&path).as_deref() == Some(location.as_str()) {
                    updated += 1;
                } else {
                    last_error = "ярлик не змінився".into();
                }
            }
            Err(error) => last_error = error,
        }
    }
    if updated == 0 {
        return Err(format!("Не вдалося оновити ярлик: {last_error}"));
    }
    Ok(updated)
}
