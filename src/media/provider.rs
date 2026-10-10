//! Provider boundary: extraction/search/downloads run in an isolated Node worker.
//! Identity, library and listening data remain in Rust.
use crate::library::CoreResult;
use serde_json::Value;
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::Duration,
};

const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;
const MAX_REPLY_BYTES: usize = 16 * 1024 * 1024;

fn read_reply(reader: &mut impl BufRead) -> CoreResult<Option<Value>> {
    let mut bytes = Vec::new();
    let mut limited = (&mut *reader).take((MAX_REPLY_BYTES + 1) as u64);
    limited
        .read_until(b'\n', &mut bytes)
        .map_err(|_| "Відповідь музичного рушія недоступна")?;
    if bytes.is_empty() {
        return Ok(None);
    }
    if bytes.len() > MAX_REPLY_BYTES || bytes.last() != Some(&b'\n') {
        return Err("Відповідь музичного рушія завелика або неповна".into());
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| "Недійсна відповідь музичного рушія".into())
}

struct RequestBuffer(Vec<u8>);
impl Write for RequestBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > (MAX_REQUEST_BYTES - 1).saturating_sub(self.0.len()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Request too large",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn encode_request(id: u64, method: &str, args: &[Value]) -> CoreResult<Vec<u8>> {
    #[derive(serde::Serialize)]
    struct Request<'a> {
        id: u64,
        method: &'a str,
        args: &'a [Value],
    }
    let mut buffer = RequestBuffer(Vec::new());
    serde_json::to_writer(&mut buffer, &Request { id, method, args })
        .map_err(|_| "Запит до музичного рушія завеликий".to_string())?;
    buffer.0.push(b'\n');
    Ok(buffer.0)
}

const METHODS: &[&str] = &[
    "ping",
    "search",
    "get_discover_tracks",
    "resolve_direct_url",
    "get_stream",
    "get_playback_file",
    "get_similar_tracks",
    "match_spotify_track",
    "download_track",
    "get_downloads",
    "get_download_job",
    "cancel_download",
    "import_external_profile",
];

struct WorkerProcess {
    child: Child,
    input: ChildStdin,
    replies: Receiver<CoreResult<Value>>,
    next_id: u64,
    #[cfg(windows)]
    _job: WindowsJob,
}

// Windows closes the entire provider process tree, including the bundled Node runtime.
// The handle is stored as an integer because the OS handle is safe to transfer across threads.
#[cfg(windows)]
struct WindowsJob(usize);
#[cfg(windows)]
impl WindowsJob {
    fn attach(child: &Child) -> CoreResult<Self> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err("Не вдалося створити ізольований процес музичного рушія".into());
            }
            let job = Self(handle as usize);
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as _,
                std::mem::size_of_val(&info) as u32,
            ) == 0
                || AssignProcessToJobObject(handle, child.as_raw_handle() as _) == 0
            {
                return Err("Не вдалося ізолювати процес музичного рушія".into());
            }
            Ok(job)
        }
    }
}
#[cfg(windows)]
impl Drop for WindowsJob {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0 as _);
        }
    }
}
pub struct ProviderBridge {
    resource_dir: PathBuf,
    data_dir: PathBuf,
    process: Option<WorkerProcess>,
}

impl ProviderBridge {
    pub fn new(resource_dir: PathBuf, data_dir: PathBuf) -> Self {
        Self {
            resource_dir,
            data_dir,
            process: None,
        }
    }
    fn executable(&self) -> CoreResult<Command> {
        let exe = if cfg!(windows) {
            "soundflow-node.exe"
        } else {
            "soundflow-node"
        };
        let app_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(std::path::Path::to_path_buf));
        let node = [
            self.resource_dir.join("resources/runtime").join(exe),
            self.resource_dir.join("runtime").join(exe),
            self.resource_dir.join(exe),
            app_dir.as_ref().map(|p| p.join(exe)).unwrap_or_default(),
            app_dir
                .as_ref()
                .map(|p| p.join("resources/runtime").join(exe))
                .unwrap_or_default(),
        ]
        .into_iter()
        .find(|p| p.is_file());
        let script = [
            self.resource_dir.join("media-provider/dist/worker.cjs"),
            self.resource_dir.join("media-provider/worker.cjs"),
            app_dir
                .as_ref()
                .map(|p| p.join("media-provider/dist/worker.cjs"))
                .unwrap_or_default(),
        ]
        .into_iter()
        .find(|p| p.is_file());
        if let (Some(node), Some(script)) = (node, script) {
            let mut command = Command::new(node);
            command.arg(script);
            return Ok(command);
        }
        #[cfg(debug_assertions)]
        {
            let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let binary = manifest.join("resources/runtime").join(exe);
            let script = [
                manifest.join("media-provider/dist/worker.cjs"),
                manifest.join("media-provider/worker.mjs"),
            ]
            .into_iter()
            .find(|p| p.is_file());
            if let Some(script) = script {
                let mut command = Command::new(if binary.is_file() {
                    binary
                } else {
                    PathBuf::from("node")
                });
                command.arg(script);
                return Ok(command);
            }
        }
        Err("Музичний рушій відсутній у збірці. Перевстановіть SoundFlow.".into())
    }
    fn start(&mut self) -> CoreResult<()> {
        std::fs::create_dir_all(self.data_dir.join("provider")).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(self.data_dir.join("downloads")).map_err(|e| e.to_string())?;
        let mut command = self.executable()?;
        command
            .arg("--data-dir")
            .arg(self.data_dir.join("provider"))
            .arg("--download-dir")
            .arg(self.data_dir.join("downloads"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("Не вдалося запустити музичний рушій: {e}"))?;
        #[cfg(windows)]
        let job = match WindowsJob::attach(&child) {
            Ok(job) => job,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        let input = child
            .stdin
            .take()
            .ok_or("Музичний рушій не має каналу запитів")?;
        let output = child
            .stdout
            .take()
            .ok_or("Музичний рушій не має каналу відповіді")?;
        let (sender, replies) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                match read_reply(&mut reader) {
                    Ok(None) => {
                        let _ = sender.send(Err("Музичний рушій завершив роботу".into()));
                        break;
                    }
                    Ok(Some(reply)) => {
                        if sender.send(Ok(reply)).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(Err(error));
                        break;
                    }
                }
            }
        });
        self.process = Some(WorkerProcess {
            child,
            input,
            replies,
            next_id: 1,
            #[cfg(windows)]
            _job: job,
        });
        Ok(())
    }
    pub fn call(&mut self, method: &str, args: &[Value]) -> CoreResult<Value> {
        if !METHODS.contains(&method) {
            return Err("Непідтримуваний метод музичного рушія".into());
        }
        if self.process.is_none() {
            self.start()?
        }
        let process = self.process.as_mut().unwrap();
        if process
            .child
            .try_wait()
            .map_err(|e| e.to_string())?
            .is_some()
        {
            self.process = None;
            return Err("Музичний рушій завершив роботу. Повторіть запит.".into());
        }
        let id = process.next_id;
        process.next_id += 1;
        let request = encode_request(id, method, args)?;
        if process
            .input
            .write_all(&request)
            .and_then(|_| process.input.flush())
            .is_err()
        {
            self.stop();
            return Err("Музичний рушій не відповідає. Повторіть запит.".into());
        }
        let response = process.replies.recv_timeout(Duration::from_secs(90));
        let reply = match response {
            Ok(Ok(reply)) => reply,
            Ok(Err(error)) => {
                self.stop();
                return Err(error);
            }
            Err(_) => {
                self.stop();
                return Err("Музичний сервіс не відповів вчасно. Повторіть запит.".into());
            }
        };
        if reply["id"].as_u64() != Some(id) {
            self.stop();
            return Err("Порушено порядок відповідей музичного рушія".into());
        }
        if let Some(error) = reply.get("error").filter(|v| !v.is_null()) {
            return Err(error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Помилка музичного сервісу")
                .chars()
                .take(1000)
                .collect());
        }
        Ok(reply.get("result").cloned().unwrap_or(Value::Null))
    }
    pub fn stop(&mut self) {
        if let Some(process) = self.process.take() {
            let WorkerProcess {
                mut child,
                input,
                replies: _,
                next_id: _,
                #[cfg(windows)]
                _job,
            } = process;
            // EOF asks the worker to cancel active downloads and remove partial files first.
            drop(input);
            for _ in 0..20 {
                if child.try_wait().ok().flatten().is_some() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            #[cfg(windows)]
            drop(_job);
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
impl Drop for ProviderBridge {
    fn drop(&mut self) {
        self.stop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;

    #[test]
    fn jsonl_requires_complete_utf8_json_frames() {
        let mut valid = Cursor::new("{\"id\":1,\"result\":\"Музика\"}\n");
        assert_eq!(read_reply(&mut valid).unwrap().unwrap()["result"], "Музика");
        assert!(read_reply(&mut valid).unwrap().is_none());
        assert!(read_reply(&mut Cursor::new(b"{\"id\":1}".as_slice())).is_err());
        assert!(read_reply(&mut Cursor::new(b"{\"id\":\"\xff\"}\n".as_slice())).is_err());
    }

    #[test]
    fn oversized_reply_stops_at_the_frame_limit() {
        let mut reader = Cursor::new(vec![b' '; MAX_REPLY_BYTES + 100]);
        assert!(read_reply(&mut reader).is_err());
        assert_eq!(reader.position(), (MAX_REPLY_BYTES + 1) as u64);
    }

    #[test]
    fn requests_are_bounded_and_keep_the_jsonl_contract() {
        let encoded = encode_request(7, "search", &[json!("Музика"), json!("all")]).unwrap();
        assert_eq!(encoded.last(), Some(&b'\n'));
        let decoded: Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded["id"], 7);
        assert_eq!(decoded["method"], "search");
        assert_eq!(decoded["args"][0], "Музика");
        assert!(encode_request(8, "search", &[json!("a".repeat(MAX_REQUEST_BYTES))]).is_err());
    }
}
