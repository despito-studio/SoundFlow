//! Velopack owns package selection, deltas and replacement. Our transport adds
//! cancellation while the existing UI owns consent, progress and graceful exit.
use reqwest::blocking::Client;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::Sender,
        Arc, Mutex,
    },
    time::Duration,
};
use velopack::{
    sources::UpdateSource, Error, UpdateCheck, UpdateInfo, UpdateManager, VelopackAsset,
    VelopackAssetFeed,
};

const FEED: &str = "https://despito-studio.github.io/SoundFlow/updates/win-x64/";
const MAX_PACKAGE: u64 = 512 * 1024 * 1024;

/// Called by the Windows host BEFORE COM, Flutter, single-instance or audio startup.
#[no_mangle]
pub extern "C" fn sf_velopack_startup() {
    velopack::VelopackApp::build()
        .set_auto_apply_on_startup(false)
        .run();
}

fn cancelled(flag: &AtomicBool) -> Result<(), Error> {
    if flag.load(Ordering::Acquire) {
        Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "Скасовано").into())
    } else {
        Ok(())
    }
}
fn stable(text: &str) -> Option<semver::Version> {
    semver::Version::parse(text)
        .ok()
        .filter(|v| v.pre.is_empty() && v.build.is_empty())
}
fn validate_asset(asset: &VelopackAsset) -> Result<(), Error> {
    let version =
        stable(&asset.Version).ok_or_else(|| Error::InvalidPackage("Недійсна версія".into()))?;
    let suffix = if asset.Type == "Full" {
        "full"
    } else if asset.Type == "Delta" {
        "delta"
    } else {
        return Err(Error::InvalidPackage("Невідомий тип пакета".into()));
    };
    if asset.PackageId != "SoundFlow"
        || asset.FileName != format!("SoundFlow-{version}-{suffix}.nupkg")
        || !(1..=MAX_PACKAGE).contains(&asset.Size)
        || asset.SHA256.len() != 64
        || !asset.SHA256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(Error::InvalidPackage(
            "Недійсні дані пакета SoundFlow".into(),
        ));
    }
    Ok(())
}
fn parse_feed(bytes: &[u8]) -> Result<VelopackAssetFeed, Error> {
    let feed: VelopackAssetFeed = serde_json::from_slice(bytes)?;
    for asset in &feed.Assets {
        validate_asset(asset)?;
    }
    Ok(feed)
}
#[derive(Clone)]
struct PagesSource {
    cancel: Arc<AtomicBool>,
    assets: Arc<Mutex<Vec<VelopackAsset>>>,
}
impl PagesSource {
    fn client(&self) -> Result<Client, Error> {
        let builder = reqwest::Client::builder()
            .https_only(true)
            .connect_timeout(Duration::from_secs(8))
            .read_timeout(Duration::from_secs(3))
            .user_agent(format!("SoundFlow/{}", env!("CARGO_PKG_VERSION")));
        reqwest::blocking::ClientBuilder::from(builder)
            .timeout(None)
            .build()
            .map_err(|e| Error::Other(e.to_string()))
    }
    fn feed(&self) -> Result<VelopackAssetFeed, Error> {
        cancelled(&self.cancel)?;
        let mut response = self
            .client()?
            .get(format!("{FEED}releases.win.json"))
            .header("Cache-Control", "no-cache")
            .send()
            .and_then(|r| r.error_for_status())
            .map_err(|e| Error::Other(e.to_string()))?;
        let mut bytes = Vec::new();
        response
            .by_ref()
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        cancelled(&self.cancel)?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err(Error::Other("Завеликий список оновлень".into()));
        }
        let feed = parse_feed(&bytes)?;
        *self.assets.lock().unwrap_or_else(|e| e.into_inner()) = feed.Assets.clone();
        Ok(feed)
    }
}
impl UpdateSource for PagesSource {
    fn get_release_feed(
        &self,
        channel: &str,
        _app: &velopack::bundle::Manifest,
        _staged_user_id: &str,
    ) -> Result<VelopackAssetFeed, Error> {
        if channel != "win" {
            return Err(Error::NotSupported("Невідомий канал оновлень".into()));
        }
        self.feed()
    }
    fn download_release_entry(
        &self,
        asset: &VelopackAsset,
        local_file: &Path,
        progress: Option<Sender<i16>>,
    ) -> Result<(), Error> {
        cancelled(&self.cancel)?;
        validate_asset(asset)?;
        let mut response = self
            .client()?
            .get(format!("{FEED}{}", asset.FileName))
            .send()
            .and_then(|r| r.error_for_status())
            .map_err(|e| Error::Other(e.to_string()))?;
        if response.content_length().is_some_and(|n| n != asset.Size) {
            return Err(Error::Other("Розмір пакета не збігається".into()));
        }
        let mut file = File::create(local_file)?;
        copy_package(&mut response, &mut file, asset.Size, &self.cancel, |n| {
            if let Some(tx) = &progress {
                let _ = tx.send((n.saturating_mul(100) / asset.Size) as i16);
            }
        })?;
        file.sync_all()?;
        Ok(())
    }
}
fn copy_package(
    input: &mut impl Read,
    output: &mut impl Write,
    size: u64,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64),
) -> Result<(), Error> {
    let mut received = 0;
    let mut buffer = [0u8; 65536];
    loop {
        cancelled(cancel)?;
        let count = input.read(&mut buffer)?;
        cancelled(cancel)?;
        if count == 0 {
            break;
        }
        received += count as u64;
        if received > size {
            return Err(Error::Other("Пакет перевищує заявлений розмір".into()));
        }
        output.write_all(&buffer[..count])?;
        progress(received);
    }
    if received != size {
        return Err(Error::Other("Завантаження перервано".into()));
    }
    cancelled(cancel)
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Fingerprint {
    size: u64,
    sha256: String,
}
impl Fingerprint {
    fn from_feed(asset: &VelopackAsset) -> Self {
        Self {
            size: asset.Size,
            sha256: asset.SHA256.clone(),
        }
    }
    fn valid(&self) -> bool {
        (1..=MAX_PACKAGE).contains(&self.size)
            && self.sha256.len() == 64
            && self.sha256.bytes().all(|b| b.is_ascii_hexdigit())
    }
}
#[derive(serde::Serialize, serde::Deserialize)]
struct ReadyReceipt {
    target: VelopackAsset,
    package: Fingerprint,
}
fn same_identity(a: &VelopackAsset, b: &VelopackAsset) -> bool {
    a.PackageId == b.PackageId
        && a.Version == b.Version
        && a.FileName == b.FileName
        && a.Type == b.Type
}
fn fingerprint(path: &Path, cancel: &AtomicBool) -> Result<Fingerprint, Error> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    if !(1..=MAX_PACKAGE).contains(&size) {
        return Err(Error::InvalidPackage(
            "Недійсний розмір локального пакета".into(),
        ));
    }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut received = 0;
    loop {
        cancelled(cancel)?;
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        received += count as u64;
        if received > size {
            return Err(Error::Other("Пакет змінився під час перевірки".into()));
        }
        hash.update(&buffer[..count]);
    }
    if received != size {
        return Err(Error::Other("Пакет змінився під час перевірки".into()));
    }
    cancelled(cancel)?;
    Ok(Fingerprint {
        size,
        sha256: format!("{:x}", hash.finalize()),
    })
}
fn verify_prepared(
    path: &Path,
    asset: &VelopackAsset,
    expected: &Fingerprint,
    cancel: &AtomicBool,
) -> Result<(), Error> {
    validate_asset(asset)?;
    if !expected.valid() {
        return Err(Error::InvalidPackage("Недійсний запис кешу".into()));
    }
    let actual = fingerprint(path, cancel)?;
    if actual.size != expected.size || !actual.sha256.eq_ignore_ascii_case(&expected.sha256) {
        return Err(Error::Other(
            "Перевірка пакета не пройдена. Спробуй ще раз.".into(),
        ));
    }
    let mut bundle = velopack::bundle::load_bundle_from_file(path)?;
    let manifest = bundle.read_manifest()?;
    if manifest.id != "SoundFlow" || manifest.version.to_string() != asset.Version {
        return Err(Error::InvalidPackage(
            "Версія всередині пакета не збігається з оновленням".into(),
        ));
    }
    cancelled(cancel)
}
#[cfg(test)]
fn verify_cached(path: &Path, asset: &VelopackAsset, cancel: &AtomicBool) -> Result<(), Error> {
    verify_prepared(path, asset, &Fingerprint::from_feed(asset), cancel)
}

pub struct Updater {
    state: Mutex<Value>,
    info: Mutex<Option<UpdateInfo>>,
    manager: Option<UpdateManager>,
    packages: Option<PathBuf>,
    assets: Arc<Mutex<Vec<VelopackAsset>>>,
    cancel: Arc<AtomicBool>,
    busy: AtomicBool,
    sequence: AtomicU64,
    record: PathBuf,
    offline: bool,
    current: String,
}
impl Updater {
    pub fn new(data: &Path, offline: bool) -> Arc<Self> {
        let cancel = Arc::new(AtomicBool::new(false));
        let assets = Arc::new(Mutex::new(Vec::new()));
        let source = PagesSource {
            cancel: cancel.clone(),
            assets: assets.clone(),
        };
        let manager = UpdateManager::new(source, None, None)
            .ok()
            .filter(|m| m.get_app_id() == "SoundFlow");
        let packages = velopack::locator::auto_locate_app_manifest(
            velopack::locator::LocationContext::FromCurrentExe,
        )
        .ok()
        .map(|l| l.get_packages_dir());
        let current = manager
            .as_ref()
            .map(|m| m.get_current_version_as_string())
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").into());
        let updater = Arc::new(Self {
            state: Mutex::new(Value::Null),
            info: Mutex::new(None),
            manager,
            packages,
            assets,
            cancel,
            busy: AtomicBool::new(false),
            sequence: AtomicU64::new(0),
            record: data.join("updates/velopack-ready.json"),
            offline,
            current,
        });
        updater.set("idle", "", None, 0);
        updater.restore();
        updater
    }
    fn restore(&self) {
        let restore = || -> Option<UpdateInfo> {
            self.manager.as_ref()?;
            let receipt = self.read_receipt()?;
            let asset = &receipt.target;
            validate_asset(asset).ok()?;
            if stable(&asset.Version)? <= stable(&self.current)? {
                return None;
            }
            if fs::metadata(self.package_path(asset).ok()?).ok()?.len() != receipt.package.size {
                return None;
            }
            Some(UpdateInfo {
                TargetFullRelease: receipt.target,
                ..Default::default()
            })
        };
        if let Some(info) = restore() {
            self.set(
                "ready",
                "Оновлення готове",
                Some(&info.TargetFullRelease),
                info.TargetFullRelease.Size,
            );
            *self.info.lock().unwrap_or_else(|e| e.into_inner()) = Some(info);
        }
    }
    pub fn snapshot(&self) -> Value {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    fn set(&self, status: &str, message: &str, asset: Option<&VelopackAsset>, received: u64) {
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = json!({"status":status,"message":message,
            "sequence":self.sequence.fetch_add(1,Ordering::AcqRel)+1,"current":self.current,"installed":self.manager.is_some(),
            "portable":self.manager.as_ref().is_some_and(|m| m.get_is_portable()),
            "version":asset.map(|a| &a.Version),"received":received,"total":asset.map_or(0,|a| a.Size)});
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
    fn start(self: &Arc<Self>, task: impl FnOnce(&Self) -> Result<(), Error> + Send + 'static) {
        if self.busy.swap(true, Ordering::AcqRel) {
            return;
        }
        self.cancel.store(false, Ordering::Release);
        let worker = self.clone();
        std::thread::spawn(move || {
            if let Err(error) = task(&worker) {
                let info = worker
                    .info
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                let cancelled = worker.cancel.load(Ordering::Acquire);
                let message = if cancelled {
                    "Скасовано"
                } else {
                    match &error { Error::ChecksumInvalid(..) | Error::SizeInvalid(..) | Error::InvalidPackage(..) => "Перевірка пакета не пройдена. Спробуй ще раз.",
                        Error::NotInstalled(..) => "Автооновлення доступне у встановленій або portable-версії. Ця збірка призначена для розробки.",
                        _ => "Не вдалося оновити SoundFlow. Перевір підключення та вільне місце й повтори спробу." }
                };
                worker.set(
                    if cancelled { "cancelled" } else { "error" },
                    message,
                    info.as_ref().map(|i| &i.TargetFullRelease),
                    0,
                );
            }
            worker.busy.store(false, Ordering::Release);
        });
    }
    pub fn check(self: &Arc<Self>, download: bool) {
        self.start(move |worker| {
            if worker.offline {
                return Err(Error::NotSupported("Офлайн-режим".into()));
            }
            worker.set("checking", "Перевіряємо оновлення…", None, 0);
            let check = worker
                .manager
                .as_ref()
                .ok_or_else(|| Error::NotInstalled("Збірка для розробки".into()))?
                .check_for_updates()?;
            cancelled(&worker.cancel)?;
            match check {
                UpdateCheck::UpdateAvailable(info) => {
                    *worker.info.lock().unwrap_or_else(|e| e.into_inner()) = Some(*info.clone());
                    worker.set(
                        "available",
                        "Є нова версія",
                        Some(&info.TargetFullRelease),
                        0,
                    );
                    if download {
                        worker.fetch(&info)?;
                    }
                }
                UpdateCheck::NoUpdateAvailable => {
                    worker.set("current", "Встановлено найновішу версію", None, 0)
                }
                UpdateCheck::RemoteIsEmpty => {
                    return Err(Error::Other("Канал оновлень ще не опублікований".into()))
                }
            }
            Ok(())
        });
    }
    fn selected(&self) -> Result<UpdateInfo, Error> {
        self.info
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| Error::Other("Спочатку перевір оновлення".into()))
    }
    fn read_receipt(&self) -> Option<ReadyReceipt> {
        let receipt: ReadyReceipt =
            serde_json::from_reader(File::open(&self.record).ok()?.take(65536)).ok()?;
        validate_asset(&receipt.target).ok()?;
        receipt.package.valid().then_some(receipt)
    }
    fn cached_proof(&self, asset: &VelopackAsset) -> Option<Fingerprint> {
        let saved = self.read_receipt()?;
        (same_identity(&saved.target, asset)
            && saved.target.Size == asset.Size
            && saved.target.SHA256.eq_ignore_ascii_case(&asset.SHA256))
        .then_some(saved.package)
    }
    fn base_is_verified(&self, base: &VelopackAsset) -> Result<bool, Error> {
        // SDK base assets have no remote checksum. Bind them either to our prior
        // verified receipt or the canonical full entry from the current HTTPS feed.
        let proof = self
            .read_receipt()
            .filter(|r| same_identity(&r.target, base))
            .map(|r| (r.target, r.package))
            .or_else(|| {
                self.assets
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .iter()
                    .find(|a| same_identity(a, base))
                    .cloned()
                    .map(|a| {
                        let p = Fingerprint::from_feed(&a);
                        (a, p)
                    })
            });
        let Some((asset, proof)) = proof else {
            return Ok(false);
        };
        let valid =
            verify_prepared(&self.package_path(&asset)?, &asset, &proof, &self.cancel).is_ok();
        cancelled(&self.cancel)?;
        Ok(valid)
    }
    fn package_path(&self, asset: &VelopackAsset) -> Result<PathBuf, Error> {
        validate_asset(asset)?;
        self.packages
            .as_ref()
            .map(|p| p.join(&asset.FileName))
            .ok_or_else(|| Error::NotInstalled("Відсутня папка пакетів".into()))
    }
    fn fetch(&self, info: &UpdateInfo) -> Result<(), Error> {
        let manager = self
            .manager
            .as_ref()
            .ok_or_else(|| Error::NotInstalled("Збірка для розробки".into()))?;
        let asset = &info.TargetFullRelease;
        let path = self.package_path(asset)?;
        self.set("verifying", "Перевіряємо оновлення…", Some(asset), 0);
        // Rebuilt delta archives have different ZIP bytes than the published full
        // package. A receipt binds their local hash to the original remote target.
        let cached = if path.exists() {
            let proof = self
                .cached_proof(asset)
                .unwrap_or_else(|| Fingerprint::from_feed(asset));
            if verify_prepared(&path, asset, &proof, &self.cancel).is_ok() {
                Some(proof)
            } else {
                cancelled(&self.cancel)?;
                fs::remove_file(&path)?;
                None
            }
        } else {
            None
        };
        cancelled(&self.cancel)?;
        if self.offline && !path.exists() {
            return Err(Error::NotSupported("Офлайн-режим".into()));
        }
        let mut plan = info.clone();
        if cached.is_none() && !plan.DeltasToTarget.is_empty() {
            let verified = match &plan.BaseRelease {
                Some(base) => self.base_is_verified(base)?,
                None => false,
            };
            if !verified {
                plan.BaseRelease = None;
                plan.DeltasToTarget.clear();
            }
        }
        self.set("downloading", "Завантажуємо оновлення…", Some(asset), 0);
        let result = std::thread::scope(|scope| {
            let (tx, rx) = std::sync::mpsc::channel::<i16>();
            scope.spawn(move || {
                for percent in rx {
                    self.set(
                        "downloading",
                        "Завантажуємо оновлення…",
                        Some(asset),
                        asset.Size.saturating_mul(percent.clamp(0, 100) as u64) / 100,
                    );
                }
            });
            manager.download_updates(&plan, Some(tx))
        });
        cancelled(&self.cancel)?;
        result?;
        self.set(
            "verifying",
            "Перевіряємо оновлення…",
            Some(asset),
            asset.Size,
        );
        // Mint a fingerprint only after the SDK prepared fresh, verified full or
        // delta bytes, or after the existing file passed its prior proof above.
        let prepared = match cached {
            Some(proof) => proof,
            None => fingerprint(&path, &self.cancel)?,
        };
        verify_prepared(&path, asset, &prepared, &self.cancel)?;
        if let Some(parent) = self.record.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut record = tempfile::NamedTempFile::new_in(self.record.parent().unwrap())?;
        serde_json::to_writer(
            &mut record,
            &ReadyReceipt {
                target: asset.clone(),
                package: prepared,
            },
        )?;
        record.as_file().sync_all()?;
        record
            .persist(&self.record)
            .map_err(|e| Error::Io(e.error))?;
        self.set("ready", "Оновлення готове", Some(asset), asset.Size);
        Ok(())
    }
    pub fn download(self: &Arc<Self>) {
        self.start(|worker| worker.fetch(&worker.selected()?));
    }
    fn apply(&self, info: &UpdateInfo) -> Result<(), Error> {
        let manager = self
            .manager
            .as_ref()
            .ok_or_else(|| Error::NotInstalled("Потрібен Velopack".into()))?;
        let asset = &info.TargetFullRelease;
        self.set(
            "verifying",
            "Перевіряємо оновлення…",
            Some(asset),
            asset.Size,
        );
        let proof = self
            .cached_proof(asset)
            .unwrap_or_else(|| Fingerprint::from_feed(asset));
        verify_prepared(&self.package_path(asset)?, asset, &proof, &self.cancel)?;
        cancelled(&self.cancel)?;
        // Flutter closes engine/audio gracefully on `restarting`; Velopack waits.
        self.set(
            "installing",
            "Готуємо встановлення…",
            Some(asset),
            asset.Size,
        );
        manager.wait_exit_then_apply_updates(asset, true, true, Vec::<String>::new())?;
        self.set(
            "restarting",
            "Перезапускаємо SoundFlow…",
            Some(asset),
            asset.Size,
        );
        Ok(())
    }
    pub fn apply_async(self: &Arc<Self>) {
        self.start(|worker| worker.apply(&worker.selected()?));
    }
    pub fn install_async(self: &Arc<Self>) {
        self.start(|worker| {
            let info = worker.selected()?;
            worker.fetch(&info)?;
            cancelled(&worker.cancel)?;
            worker.apply(&info)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn asset() -> VelopackAsset {
        VelopackAsset {
            PackageId: "SoundFlow".into(),
            Version: "1.0.1".into(),
            Type: "Full".into(),
            FileName: "SoundFlow-1.0.1-full.nupkg".into(),
            SHA256: "a".repeat(64),
            Size: 4096,
            ..Default::default()
        }
    }
    #[test]
    fn feed_discovers_new_version_without_installer_or_portable() {
        let feed = parse_feed(&serde_json::to_vec(&json!({"Assets":[asset()]})).unwrap()).unwrap();
        assert_eq!(feed.Assets.len(), 1);
        assert!(stable(&feed.Assets[0].Version) > stable("1.0.0"));
        assert!(stable(&feed.Assets[0].Version) < stable("1.0.10"));
    }
    #[test]
    fn reject_foreign_paths_packages_and_unverified_assets() {
        for name in [
            "../SoundFlow-1.0.1-full.nupkg",
            "https://example.com/a.nupkg",
            "C:\\a.nupkg",
            "SoundFlow-1.0.1-Setup.exe",
            "SoundFlow-1.0.1-windows-x64.zip",
        ] {
            let mut a = asset();
            a.FileName = name.into();
            assert!(validate_asset(&a).is_err());
        }
        let mut a = asset();
        a.PackageId = "OtherApp".into();
        assert!(validate_asset(&a).is_err());
        let mut a = asset();
        a.SHA256.clear();
        assert!(validate_asset(&a).is_err());
        let mut a = asset();
        a.Version = "1.0.1-beta".into();
        assert!(validate_asset(&a).is_err());
        let mut a = asset();
        a.Size = MAX_PACKAGE + 1;
        assert!(validate_asset(&a).is_err());
    }
    #[test]
    fn cancellation_size_and_mid_read_are_enforced() {
        let check = |mut input: &[u8], size, flag: &AtomicBool| {
            copy_package(&mut input, &mut Vec::new(), size, flag, |_| {})
        };
        let flag = AtomicBool::new(false);
        assert!(check(b"abc", 3, &flag).is_ok());
        assert!(check(b"abc", 2, &flag).is_err());
        assert!(check(b"abc", 4, &flag).is_err());
        flag.store(true, Ordering::Release);
        assert!(check(b"abc", 3, &flag).is_err());
        struct CancelRead<'a>(&'a AtomicBool);
        impl Read for CancelRead<'_> {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                self.0.store(true, Ordering::Release);
                buffer[0] = 1;
                Ok(1)
            }
        }
        flag.store(false, Ordering::Release);
        let mut output = Vec::new();
        assert!(
            copy_package(&mut CancelRead(&flag), &mut output, 1, &flag, |_| panic!(
                "cancelled bytes must not be reported"
            ))
            .is_err()
        );
        assert!(output.is_empty());
    }
    fn manifest(version: &str) -> String {
        format!("<package><metadata><id>SoundFlow</id><version>{version}</version><mainExe>SoundFlow.exe</mainExe><channel>win</channel></metadata></package>")
    }
    fn fixture(root: &Path) -> velopack::locator::VelopackLocatorConfig {
        let current = root.join("current");
        fs::create_dir_all(&current).unwrap();
        let packages = root.join("packages");
        fs::create_dir_all(&packages).unwrap();
        fs::write(current.join("sq.version"), manifest("1.0.0")).unwrap();
        fs::write(
            root.join("Update.exe"),
            b"inert test fixture; never executed",
        )
        .unwrap();
        velopack::locator::VelopackLocatorConfig {
            RootAppDir: root.into(),
            UpdateExePath: root.join("Update.exe"),
            PackagesDir: packages,
            ManifestPath: current.join("sq.version"),
            CurrentBinaryDir: current,
            IsPortable: true,
        }
    }
    fn package(folder: &Path, version: &str) -> VelopackAsset {
        use zip::{write::SimpleFileOptions, ZipWriter};
        fs::create_dir_all(folder).unwrap();
        let mut a = asset();
        a.Version = version.into();
        a.FileName = format!("SoundFlow-{version}-full.nupkg");
        let path = folder.join(&a.FileName);
        let mut zip = ZipWriter::new(File::create(&path).unwrap());
        zip.start_file("SoundFlow.nuspec", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(manifest(version).as_bytes()).unwrap();
        zip.start_file("lib/app/SoundFlow.exe", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"inert application fixture").unwrap();
        zip.finish().unwrap();
        let bytes = fs::read(path).unwrap();
        a.Size = bytes.len() as u64;
        a.SHA256 = format!("{:x}", Sha256::digest(&bytes));
        a
    }
    #[test]
    fn sdk_checks_numeric_versions_and_downloads_without_any_installer() {
        let temp = tempfile::tempdir().unwrap();
        let config = fixture(&temp.path().join("app"));
        let remote = temp.path().join("feed");
        let older = package(&remote, "1.0.2");
        let target = package(&remote, "1.0.10");
        fs::write(
            remote.join("releases.win.json"),
            serde_json::to_vec(&json!({"Assets":[target.clone(),older]})).unwrap(),
        )
        .unwrap();
        let manager = UpdateManager::new(
            velopack::sources::FileSource::new(&remote),
            None,
            Some(config.clone()),
        )
        .unwrap();
        let UpdateCheck::UpdateAvailable(info) = manager.check_for_updates().unwrap() else {
            panic!("expected newer release")
        };
        assert_eq!(info.TargetFullRelease.Version, "1.0.10");
        manager.download_updates(&info, None).unwrap();
        verify_cached(
            &config.PackagesDir.join(&target.FileName),
            &target,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(
            manager.get_update_pending_restart().unwrap().Version,
            "1.0.10"
        );
        fs::write(
            remote.join("releases.win.json"),
            serde_json::to_vec(&json!({"Assets":[package(&remote,"1.0.0")]})).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            manager.check_for_updates().unwrap(),
            UpdateCheck::NoUpdateAvailable
        ));
    }
    #[test]
    fn sdk_rejects_bad_checksum_and_wrapper_repairs_corrupt_cached_package() {
        let temp = tempfile::tempdir().unwrap();
        let config = fixture(&temp.path().join("app"));
        let remote = temp.path().join("feed");
        let asset = package(&remote, "1.0.1");
        let manager = UpdateManager::new(
            velopack::sources::FileSource::new(&remote),
            None,
            Some(config.clone()),
        )
        .unwrap();
        let mut bad = asset.clone();
        bad.SHA256 = "0".repeat(64);
        assert!(matches!(
            manager.download_updates(
                &UpdateInfo {
                    TargetFullRelease: bad,
                    ..Default::default()
                },
                None
            ),
            Err(Error::ChecksumInvalid(..))
        ));
        let path = config.PackagesDir.join(&asset.FileName);
        assert!(!path.exists());
        fs::write(&path, vec![0; asset.Size as usize]).unwrap();
        assert!(verify_cached(&path, &asset, &AtomicBool::new(false)).is_err());
        let worker = Updater {
            state: Mutex::new(Value::Null),
            info: Mutex::new(None),
            manager: Some(manager),
            packages: Some(config.PackagesDir),
            assets: Arc::new(Mutex::new(Vec::new())),
            cancel: Arc::new(AtomicBool::new(false)),
            busy: AtomicBool::new(false),
            sequence: AtomicU64::new(0),
            record: temp.path().join("data/ready.json"),
            offline: false,
            current: "1.0.0".into(),
        };
        worker
            .fetch(&UpdateInfo {
                TargetFullRelease: asset.clone(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(worker.snapshot()["status"], "ready");
        verify_cached(&path, &asset, &worker.cancel).unwrap();
        worker.cancel();
        assert!(worker
            .apply(&UpdateInfo {
                TargetFullRelease: asset,
                ..Default::default()
            })
            .is_err());
        assert_ne!(worker.snapshot()["status"], "restarting");
    }
    #[test]
    fn rebuilt_package_receipt_binds_cache_and_base_without_changing_remote_hash() {
        use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};
        let temp = tempfile::tempdir().unwrap();
        let config = fixture(&temp.path().join("app"));
        let remote = temp.path().join("feed");
        let asset = package(&remote, "1.0.1");
        let path = config.PackagesDir.join(&asset.FileName);
        let mut source =
            ZipArchive::new(File::open(remote.join(&asset.FileName)).unwrap()).unwrap();
        let mut rebuilt = ZipWriter::new(File::create(&path).unwrap());
        rebuilt.set_comment("framework reconstruction").unwrap();
        for index in 0..source.len() {
            let mut entry = source.by_index(index).unwrap();
            rebuilt
                .start_file(entry.name(), SimpleFileOptions::default())
                .unwrap();
            std::io::copy(&mut entry, &mut rebuilt).unwrap();
        }
        rebuilt.finish().unwrap();
        let flag = AtomicBool::new(false);
        let prepared = fingerprint(&path, &flag).unwrap();
        assert_ne!(prepared.sha256.to_uppercase(), asset.SHA256.to_uppercase());
        assert!(verify_cached(&path, &asset, &flag).is_err());
        verify_prepared(&path, &asset, &prepared, &flag).unwrap();
        let manager = UpdateManager::new(
            velopack::sources::FileSource::new(remote),
            None,
            Some(config.clone()),
        )
        .unwrap();
        let worker = Updater {
            state: Mutex::new(Value::Null),
            info: Mutex::new(None),
            manager: Some(manager),
            packages: Some(config.PackagesDir.clone()),
            assets: Arc::new(Mutex::new(Vec::new())),
            cancel: Arc::new(AtomicBool::new(false)),
            busy: AtomicBool::new(false),
            sequence: AtomicU64::new(0),
            record: temp.path().join("ready.json"),
            offline: true,
            current: "1.0.0".into(),
        };
        fs::write(
            &worker.record,
            serde_json::to_vec(&ReadyReceipt {
                target: asset.clone(),
                package: prepared.clone(),
            })
            .unwrap(),
        )
        .unwrap();
        worker.restore();
        assert_eq!(worker.snapshot()["status"], "ready");
        assert_eq!(worker.cached_proof(&asset).unwrap().sha256, prepared.sha256);
        let mut changed = asset.clone();
        changed.SHA256 = "0".repeat(64);
        assert!(worker.cached_proof(&changed).is_none());
        let mut base = asset.clone();
        base.SHA256.clear();
        assert!(worker.base_is_verified(&base).unwrap());
        // A verified rebuilt cache is reusable offline; SDK's no-op must not mint
        // an unbound receipt or compare this archive to the canonical full ZIP.
        worker
            .fetch(&UpdateInfo {
                TargetFullRelease: asset.clone(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(worker.snapshot()["status"], "ready");
        let mut bytes = fs::read(&path).unwrap();
        bytes.push(0);
        fs::write(&path, bytes).unwrap();
        assert!(verify_prepared(&path, &asset, &prepared, &flag).is_err());
        assert!(!worker.base_is_verified(&base).unwrap());
    }
    #[test]
    fn sdk_delta_fallback_cannot_clear_cancellation() {
        struct CancelSource {
            flag: Arc<AtomicBool>,
            calls: Arc<AtomicU64>,
        }
        impl UpdateSource for CancelSource {
            fn get_release_feed(
                &self,
                _: &str,
                _: &velopack::bundle::Manifest,
                _: &str,
            ) -> Result<VelopackAssetFeed, Error> {
                unreachable!()
            }
            fn download_release_entry(
                &self,
                _: &VelopackAsset,
                _: &Path,
                _: Option<Sender<i16>>,
            ) -> Result<(), Error> {
                self.calls.fetch_add(1, Ordering::AcqRel);
                self.flag.store(true, Ordering::Release);
                cancelled(&self.flag)
            }
        }
        let temp = tempfile::tempdir().unwrap();
        let config = fixture(&temp.path().join("app"));
        let base = package(&config.PackagesDir, "1.0.0");
        let flag = Arc::new(AtomicBool::new(false));
        let calls = Arc::new(AtomicU64::new(0));
        let manager = UpdateManager::new(
            CancelSource {
                flag: flag.clone(),
                calls: calls.clone(),
            },
            None,
            Some(config.clone()),
        )
        .unwrap();
        let mut delta = asset();
        delta.Type = "Delta".into();
        delta.FileName = "SoundFlow-1.0.1-delta.nupkg".into();
        let info = UpdateInfo {
            TargetFullRelease: asset(),
            BaseRelease: Some(base),
            DeltasToTarget: vec![delta],
            IsDowngrade: false,
        };
        assert!(manager.download_updates(&info, None).is_err());
        assert_eq!(
            calls.load(Ordering::Acquire),
            2,
            "SDK attempts full fallback after delta cancellation"
        );
        assert!(flag.load(Ordering::Acquire));
        assert!(!config
            .PackagesDir
            .join(&info.TargetFullRelease.FileName)
            .exists());
    }
}
