//! Isolated packaging smoke test. Never opens the real SoundFlow profile or UI.
#![cfg_attr(windows, windows_subsystem = "windows")]
use std::{fs, path::PathBuf};
use velopack::{sources::FileSource, UpdateCheck, UpdateManager, VelopackApp};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    VelopackApp::build().set_auto_apply_on_startup(false).run();
    let args: Vec<String> = std::env::args().collect();
    let value = |key: &str| -> Result<PathBuf, Box<dyn std::error::Error>> {
        args.iter()
            .position(|a| a == key)
            .and_then(|i| args.get(i + 1))
            .map(PathBuf::from)
            .ok_or_else(|| format!("Missing {key}").into())
    };
    let feed = value("--feed")?;
    let result = value("--result")?;
    let manager = UpdateManager::new(FileSource::new(&feed), None, None)?;
    if manager.get_app_id() != "SoundFlowProbe" || !manager.get_is_portable() {
        return Err("Probe must run in its isolated portable fixture".into());
    }
    let version = manager.get_current_version_as_string();
    let locator = velopack::locator::auto_locate_app_manifest(
        velopack::locator::LocationContext::FromCurrentExe,
    )?;
    if version == "1.0.1" {
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(result.with_extension("handoff"))?)?;
        let cached = locator
            .get_packages_dir()
            .join("SoundFlowProbe-1.0.1-full.nupkg");
        let (_, hash) = velopack::misc::calculate_sha1_sha256(&cached)?;
        let index: velopack::VelopackAssetFeed =
            serde_json::from_slice(&fs::read(feed.join("releases.win.json"))?)?;
        let target = index
            .Assets
            .iter()
            .find(|a| a.FileName == "SoundFlowProbe-1.0.1-full.nupkg")
            .ok_or("Missing remote target identity")?;
        if receipt["version"] != "1.0.1"
            || receipt["remoteSHA256"].as_str() != Some(target.SHA256.as_str())
            || receipt["size"].as_u64() != Some(fs::metadata(cached)?.len())
            || receipt["sha256"].as_str() != Some(hash.as_str())
        {
            return Err("Prepared cache receipt did not survive restart".into());
        }
        let bytes = fs::read(
            std::env::current_exe()?
                .parent()
                .unwrap()
                .join("fixture.txt"),
        )?;
        if bytes != b"updated fixture" {
            return Err("Updated files were not applied".into());
        }
        fs::write(
            &result,
            "PASS: 1.0.0 -> 1.0.1; delta, cache receipt, silent apply and restart; no installer",
        )?;
        return Ok(());
    }
    if version != "1.0.0" {
        return Err(format!("Unexpected fixture version {version}").into());
    }
    let UpdateCheck::UpdateAvailable(info) = manager.check_for_updates()? else {
        return Err("New version was not detected".into());
    };
    if info.DeltasToTarget.is_empty() {
        return Err("Expected a delta from the seeded current package".into());
    }
    let base = info.BaseRelease.as_ref().ok_or("Missing delta base")?;
    let feed_index: velopack::VelopackAssetFeed =
        serde_json::from_slice(&fs::read(feed.join("releases.win.json"))?)?;
    let canonical = feed_index
        .Assets
        .iter()
        .find(|a| a.FileName == base.FileName)
        .ok_or("Base not in feed")?;
    let base_path = locator.get_packages_dir().join(&base.FileName);
    let (_, base_hash) = velopack::misc::calculate_sha1_sha256(&base_path)?;
    if !base_hash.eq_ignore_ascii_case(&canonical.SHA256)
        || fs::metadata(base_path)?.len() != canonical.Size
    {
        return Err("Unverified delta base".into());
    }
    manager.download_updates(&info, None)?;
    let cached = locator
        .get_packages_dir()
        .join(&info.TargetFullRelease.FileName);
    let (_, hash) = velopack::misc::calculate_sha1_sha256(&cached)?;
    fs::write(
        result.with_extension("handoff"),
        serde_json::to_vec(
            &serde_json::json!({"version":info.TargetFullRelease.Version,"remoteSHA256":info.TargetFullRelease.SHA256,"sha256":hash,"size":fs::metadata(cached)?.len()}),
        )?,
    )?;
    manager.wait_exit_then_apply_updates(
        &*info,
        true,
        true,
        [
            "--feed".into(),
            feed.into_os_string(),
            "--result".into(),
            result.into_os_string(),
        ],
    )?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        let args: Vec<String> = std::env::args().collect();
        if let Some(path) = args
            .iter()
            .position(|a| a == "--result")
            .and_then(|i| args.get(i + 1))
        {
            let _ = fs::write(path, format!("FAIL: {error}"));
        }
        std::process::exit(1);
    }
}
