#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    if let Err(error) = soundflow_lib::run() {
        rfd::MessageDialog::new()
            .set_title("SoundFlow")
            .set_description(error.to_string())
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}
