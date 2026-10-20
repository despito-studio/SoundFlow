//! Explicit, isolated native verification; never reads the user's library.
use crate::{application::Application, AppWindow, TrackRow};
use rodio::Source;
use serde_json::{json, Value};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    cell::RefCell,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};

pub fn run_from_args() -> Result<bool, Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if let Some(index) = args.iter().position(|arg| arg == "--decode-audio") {
        let input = args.get(index + 1).ok_or("Expected an audio file")?;
        let result = decode_file(Path::new(input));
        let output = args
            .iter()
            .position(|arg| arg == "--report")
            .and_then(|index| args.get(index + 1));
        let report = match result {
            Ok(value) => value,
            Err(error) => json!({"success":false,"error":error}),
        };
        if let Some(output) = output {
            fs::write(output, serde_json::to_vec_pretty(&report)?)?;
        } else {
            println!("{report}");
        }
        return Ok(true);
    }
    let Some(index) = args.iter().position(|arg| arg == "--verify-native") else {
        return Ok(false);
    };
    let output = PathBuf::from(
        args.get(index + 1)
            .ok_or("Expected a verification report path")?,
    );
    let resources = args
        .iter()
        .position(|arg| arg == "--resources")
        .and_then(|index| args.get(index + 1))
        .map(PathBuf::from)
        .unwrap_or(
            std::env::current_exe()?
                .parent()
                .ok_or("Missing executable directory")?
                .into(),
        );
    let snapshots = args
        .iter()
        .position(|arg| arg == "--snapshot-dir")
        .and_then(|index| args.get(index + 1))
        .map(PathBuf::from);
    let report = match verify(resources, snapshots) {
        Ok(value) => value,
        Err(error) => json!({"success":false,"error":error.to_string()}),
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, serde_json::to_vec_pretty(&report)?)?;
    Ok(true)
}

fn decode_file(path: &Path) -> Result<Value, String> {
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut decoder = rodio::Decoder::try_from(file).map_err(|error| error.to_string())?;
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let duration = decoder.total_duration().map(|value| value.as_secs_f64());
    // Decode the entire file to catch truncated streams and unsupported codecs.
    let mut samples = 0u64;
    let mut finite = true;
    for sample in &mut decoder {
        samples += 1;
        finite &= sample.is_finite();
    }
    if samples == 0 || !finite {
        return Err("Audio did not decode into finite samples".into());
    }
    Ok(
        json!({"success":true,"channels":channels,"sample_rate":sample_rate,"duration":duration,"decoded_samples":samples}),
    )
}

fn fixture_wav(path: &Path) -> Result<(), std::io::Error> {
    let sample_rate = 16_000u32;
    let samples = sample_rate * 3;
    let data_size = samples * 2;
    let mut data = Vec::with_capacity(data_size as usize + 44);
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&(data_size + 36).to_le_bytes());
    data.extend_from_slice(b"WAVEfmt ");
    data.extend_from_slice(&16u32.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&sample_rate.to_le_bytes());
    data.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    data.extend_from_slice(&2u16.to_le_bytes());
    data.extend_from_slice(&16u16.to_le_bytes());
    data.extend_from_slice(b"data");
    data.extend_from_slice(&data_size.to_le_bytes());
    for _ in 0..samples {
        data.extend_from_slice(&0i16.to_le_bytes());
    }
    fs::write(path, data)
}

fn verify(
    resources: PathBuf,
    snapshots: Option<PathBuf>,
) -> Result<Value, Box<dyn std::error::Error>> {
    let sandbox = tempfile::tempdir()?;
    let music = sandbox.path().join("music");
    fs::create_dir(&music)?;
    let wav = music.join("SoundFlow - Native audio.wav");
    fixture_wav(&wav)?;
    let decoded = decode_file(&wav)?;
    let app = Application::new(sandbox.path().join("data"), resources.clone(), true)?;
    let provider = app.desktop.call("provider_ping", &[])?;
    if provider["runtime"] != "node" {
        return Err("The native package did not start its JavaScript worker".into());
    }
    {
        let mut core = app.desktop.core.lock().map_err(|_| "Core lock failed")?;
        if core.scan(&music).is_ok() {
            return Err("An unapproved local folder was accepted".into());
        }
        core.approve_root(&music)?;
    }
    let scan = app.desktop.call("scan_local_music", &[json!(music)])?;
    let track = {
        let core = app.desktop.core.lock().map_err(|_| "Core lock failed")?;
        core.library.snapshot()["playlists"]
            .as_object()
            .and_then(|playlists| {
                playlists.values().find_map(|p| {
                    p["tracks"]
                        .as_array()
                        .and_then(|tracks| tracks.first().cloned())
                })
            })
            .ok_or("Local scan did not create an audio track")?
    };
    app.desktop.call("toggle_like", &[track.clone()])?;
    let playlist = app
        .desktop
        .call("create_playlist", &[json!("Native verification")])?;
    app.desktop
        .call("add_to_playlist", &[playlist["id"].clone(), track.clone()])?;
    let library = app.desktop.call("get_library", &[])?;
    if library["liked_tracks"].as_array().map_or(0, Vec::len) != 1 {
        return Err("Native library write failed".into());
    }
    let gate = app
        .desktop
        .call("get_listening_stats", &[json!("30d"), json!("all")])?;
    if gate["locked"] != true {
        return Err("Unverified Plus entitlement unlocked statistics".into());
    }
    let ui = AppWindow::new()?;
    let binding = app.bind(&ui);
    let screenshot_result = Rc::new(RefCell::new(Vec::<Value>::new()));
    let capture_error = Rc::new(RefCell::new(None::<String>));
    let playback_result = Rc::new(RefCell::new(None::<Value>));
    if let Some(folder) = &snapshots {
        fs::create_dir_all(folder)?;
    }
    ui.window().set_size(slint::LogicalSize::new(1366.0, 715.0));
    ui.show()?;
    let stages = Rc::new(RefCell::new(0usize));
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    let result = screenshot_result.clone();
    let error = capture_error.clone();
    let playback_capture = playback_result.clone();
    let resources_for_ui = resources.clone();
    let snapshots_for_ui = snapshots.clone();
    let audio_for_ui = app.audio.clone();
    let fixture_id = track["id"]
        .as_str()
        .ok_or("Missing native fixture ID")?
        .to_string();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(500), move || {
        let Some(ui) = weak.upgrade() else { let _ = slint::quit_event_loop(); return; };
        let mut stage = stages.borrow_mut();
        if *stage > 0 {
            let names = ["home-1366x715", "home-1024x640", "home-840x560", "home-1600x900", "home-dark", "flow", "search", "dialog"];
            if let Some(folder) = &snapshots_for_ui {
                match ui.window().take_snapshot() {
                    Ok(buffer) => {
                        let path = folder.join(format!("{}.png", names[*stage - 1]));
                        if let Err(problem) = image::save_buffer(&path, buffer.as_bytes(), buffer.width(), buffer.height(), image::ColorType::Rgba8) {
                            *error.borrow_mut() = Some(problem.to_string());
                        } else { result.borrow_mut().push(json!({"file":path,"width":buffer.width(),"height":buffer.height()})); }
                    }
                    Err(problem) => *error.borrow_mut() = Some(problem.to_string()),
                }
            }
        }
        match *stage {
            0 => {
                ui.set_view("home".into()); ui.window().set_size(slint::LogicalSize::new(1366.0, 715.0));
                audio_for_ui.set_volume(0.0);
                ui.invoke_play_track(fixture_id.clone().into());
            }
            1 => ui.window().set_size(slint::LogicalSize::new(1024.0, 640.0)),
            2 => { audio_for_ui.pause(); ui.window().set_size(slint::LogicalSize::new(840.0, 560.0)); }
            3 => ui.window().set_size(slint::LogicalSize::new(1600.0, 900.0)),
            4 => {
                // Record the real fixture before the later UI-only preview rows.
                let loaded = ui.get_current_track().id.as_str() == fixture_id
                    && (ui.get_duration() - 3.0).abs() < 0.1;
                let paused = !ui.get_playing();
                *playback_capture.borrow_mut() = Some(json!({"loaded":loaded,"duration":ui.get_duration(),"paused":paused,"muted_fixture":true}));
                if !loaded || !paused {
                    *error.borrow_mut() = Some("The native bound audio player did not load and pause the fixture".into());
                }
                audio_for_ui.shutdown();
                ui.window().set_size(slint::LogicalSize::new(1100.0, 720.0)); ui.set_dark(true);
            }
            5 => {
                ui.set_dark(false); ui.set_view("flow".into());
                let artwork = slint::Image::load_from_path(&resources_for_ui.join("resources/covers/7.jpg")).unwrap_or_default();
                let row = TrackRow { id:"verification".into(),title:"Show Me How".into(),artist:"Men I Trust".into(),source:"soundcloud".into(),duration:"3:35".into(),liked:false,offline:false,artwork };
                ui.set_flow_track(row.clone()); ui.set_flow_count(12); ui.set_current_track(row); ui.set_duration(215.0); ui.set_position(42.0);
            }
            6 => {
                ui.set_view("search".into()); ui.set_search_query("SoundFlow".into());
                let row = TrackRow { id:"local-verification".into(),title:"Native audio".into(),artist:"SoundFlow".into(),source:"local".into(),duration:"0:03".into(),liked:true,offline:false,artwork:slint::Image::default() };
                ui.set_tracks(ModelRc::from(Rc::new(VecModel::from(vec![row]))));
            }
            7 => { ui.set_modal_kind("create-playlist".into()); ui.set_modal_title("Новий плейлист".into()); ui.set_modal_open(true); }
            _ => { let _ = slint::quit_event_loop(); }
        }
        *stage += 1;
    });
    slint::run_event_loop()?;
    let native_playback = playback_result
        .borrow()
        .clone()
        .ok_or("Missing native playback verification")?;
    timer.stop();
    drop(binding);
    app.shutdown();
    if let Some(error) = capture_error.borrow().as_ref() {
        return Err(error.clone().into());
    }
    let captured = screenshot_result.borrow().clone();
    Ok(
        json!({"success":true,"runtime":"native-slint","provider":provider,"local_scan":scan,"library_crud":true,"plus_locked":true,"decoder":decoded,"native_playback":native_playback,"snapshots":captured}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_decoder_consumes_complete_wav_without_audio_hardware() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("native.wav");
        fixture_wav(&path).unwrap();
        let report = decode_file(&path).unwrap();
        assert_eq!(report["decoded_samples"], 48_000);
        assert_eq!(report["channels"], 1);
        assert_eq!(report["sample_rate"], 16_000);
    }
}
