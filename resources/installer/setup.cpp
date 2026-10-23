// Reference-native frontend; Velopack owns installation and future updates.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <windowsx.h>
#include <objidl.h>
#include <gdiplus.h>
#include <dwmapi.h>
#include <shlobj.h>
#include <shobjidl.h>
#include <shellapi.h>
#include <tlhelp32.h>
#include <winhttp.h>
#include <bcrypt.h>
#include <commctrl.h>
#include <algorithm>
#include <atomic>
#include <cmath>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <memory>
#include <mutex>
#include <map>
#include <sstream>
#include <string>
#include <thread>
#include <vector>
#include "installer_config.h"
#include "installer_design.h"
#include "installer_vectors.h"
#ifdef small
#undef small
#endif

namespace fs = std::filesystem;
using namespace Gdiplus;
constexpr UINT kEvent = WM_APP + 1;

constexpr float kContentScale = sf_design::contentScale;
constexpr wchar_t kUninstallKey[] = L"Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\SoundFlow";
enum class Phase { Home, Consent, Download, Extract, Waiting, Commit, Success, Error };
struct Event { Phase phase; std::wstring text; double progress = -1; };
struct Model {
  HWND window{}; HINSTANCE instance{}; float dpi = kContentScale;
  Phase phase = Phase::Home; std::wstring text; double progress = -1;
  fs::path target;
  bool checked = false, preview = false;
  std::atomic<bool> cancelled{false}, committing{false}; std::thread worker;
  std::unique_ptr<Bitmap> icon; HWND primary{}, folder{}, close{}, link{}, consent{}, back{};
  std::map<int,std::unique_ptr<Bitmap>> glyphs;
  int hover = 0, pressed = 0;bool keyboard_focus = false, verify_ui = false;
  bool reduced_motion=false,verify_motion=false,dpi_override=false;fs::path motion_output;
  bool was_animating=false;
  ULONGLONG phase_started=0,opened=0,hover_started=0,progress_started=0;
  float progress_from=0;
  int preview_ms=-1;
} app;
std::mutex log_mutex;
fs::path install_log;
ULONGLONG install_started=0;

std::unique_ptr<Bitmap> png_resource(int id) {
  auto resource=FindResourceW(app.instance,MAKEINTRESOURCEW(id),RT_RCDATA);
  if (!resource) throw std::runtime_error("Не знайдено графіку інсталятора");
  DWORD size=SizeofResource(app.instance,resource);auto data=LockResource(LoadResource(app.instance,resource));
  HGLOBAL memory=GlobalAlloc(GMEM_MOVEABLE,size);if (!memory) throw std::bad_alloc();
  auto destination=GlobalLock(memory);memcpy(destination,data,size);GlobalUnlock(memory);
  IStream* stream=nullptr;if (FAILED(CreateStreamOnHGlobal(memory,TRUE,&stream))) {GlobalFree(memory);throw std::runtime_error("Не вдалося прочитати графіку");}
  std::unique_ptr<Bitmap> decoded(Bitmap::FromStream(stream));
  std::unique_ptr<Bitmap> bitmap(decoded->Clone(Rect(0,0,decoded->GetWidth(),decoded->GetHeight()),PixelFormat32bppPARGB));
  decoded.reset();stream->Release();return bitmap;
}

std::wstring wide(const std::string& text) {
  if (text.empty()) return {};
  int size = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), nullptr, 0);
  std::wstring value(size, 0);
  MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), value.data(), size);
  return value;
}
std::wstring quote(const fs::path& path) { return L"\"" + path.wstring() + L"\""; }
fs::path known(REFKNOWNFOLDERID id) {
  PWSTR value = nullptr;
  if (FAILED(SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, nullptr, &value))) throw std::runtime_error("Не вдалося знайти системну папку");
  fs::path path(value); CoTaskMemFree(value); return path;
}
std::wstring unique() {
  GUID guid{}; CoCreateGuid(&guid); wchar_t text[40]{}; StringFromGUID2(guid, text, 40); return text;
}
bool same_path(const fs::path& a, const fs::path& b) {
  return _wcsicmp(fs::absolute(a).lexically_normal().c_str(), fs::absolute(b).lexically_normal().c_str()) == 0;
}
void trace(const std::wstring& message) {
  std::lock_guard<std::mutex> lock(log_mutex);
  if(install_log.empty())return;
  int length=WideCharToMultiByte(CP_UTF8,0,message.data(),static_cast<int>(message.size()),nullptr,0,nullptr,nullptr);
  std::string encoded(length,0);WideCharToMultiByte(CP_UTF8,0,message.data(),static_cast<int>(message.size()),encoded.data(),length,nullptr,nullptr);
  std::ofstream output(install_log,std::ios::binary|std::ios::app);
  output<<GetTickCount64()-install_started<<"ms "<<encoded<<"\n";
}
void start_log() {
  try {
    auto folder=known(FOLDERID_LocalAppData)/L"SoundFlowInstaller"/L"logs";fs::create_directories(folder);
    install_log=folder/(L"setup-"+unique()+L".log");install_started=GetTickCount64();
    trace(L"SoundFlow " SF_VERSION L"; target="+app.target.wstring());
  } catch(...) {install_log.clear();}
}
void send(Phase phase, std::wstring text, double progress = -1) {
  trace(text);
  if(!app.window)return;
  auto event = new Event{phase, std::move(text), progress};
  if (!PostMessageW(app.window, kEvent, 0, reinterpret_cast<LPARAM>(event))) delete event;
}
void cancel_check() { if (app.cancelled) throw std::runtime_error("Встановлення скасовано"); }
struct Handle {
  HANDLE value = nullptr;
  explicit Handle(HANDLE h = nullptr) : value(h) {}
  ~Handle() { if (value && value != INVALID_HANDLE_VALUE) CloseHandle(value); }
  Handle(const Handle&) = delete;
};
std::string sha256(const fs::path& file) {
  BCRYPT_ALG_HANDLE algorithm{}; BCRYPT_HASH_HANDLE hash{};
  if (BCryptOpenAlgorithmProvider(&algorithm, BCRYPT_SHA256_ALGORITHM, nullptr, 0) < 0) throw std::runtime_error("Не вдалося перевірити пакет");
  DWORD length = 0, result = 0;
  BCryptGetProperty(algorithm, BCRYPT_OBJECT_LENGTH, reinterpret_cast<PUCHAR>(&length), sizeof(length), &result, 0);
  std::vector<UCHAR> object(length); UCHAR digest[32]{};
  if (BCryptCreateHash(algorithm, &hash, object.data(), length, nullptr, 0, 0) < 0) { BCryptCloseAlgorithmProvider(algorithm, 0); throw std::runtime_error("Не вдалося перевірити пакет"); }
  std::ifstream input(file, std::ios::binary); char buffer[65536]; bool ok = input.good();
  while (input) { input.read(buffer, sizeof(buffer)); auto count = input.gcount(); if (count && BCryptHashData(hash, reinterpret_cast<PUCHAR>(buffer), static_cast<ULONG>(count), 0) < 0) ok = false; }
  ok = ok && input.eof() && BCryptFinishHash(hash, digest, sizeof(digest), 0) >= 0;
  BCryptDestroyHash(hash); BCryptCloseAlgorithmProvider(algorithm, 0);
  if (!ok) throw std::runtime_error("Не вдалося прочитати пакет");
  std::ostringstream output; for (auto byte : digest) output << std::hex << std::setw(2) << std::setfill('0') << static_cast<int>(byte);
  return output.str();
}
bool embedded() { return FindResourceW(app.instance, MAKEINTRESOURCEW(501), RT_RCDATA) != nullptr; }
void package(const fs::path& output) {
  auto resource = FindResourceW(app.instance, MAKEINTRESOURCEW(501), RT_RCDATA);
  if (!resource) throw std::runtime_error("Пошкоджено інсталяційний файл");
  else {
    send(Phase::Extract, L"Готуємо файли…");
    auto data = static_cast<const char*>(LockResource(LoadResource(app.instance, resource)));
    const DWORD size = SizeofResource(app.instance, resource);
    if (!data || size != SF_PACKAGE_SIZE) throw std::runtime_error("Пошкоджено інсталяційний файл");
    std::ofstream file(output, std::ios::binary);
    for (DWORD at = 0; at < size;) {
      cancel_check(); DWORD count = std::min<DWORD>(65536, size - at); file.write(data + at, count); at += count;
      if (!file) throw std::runtime_error("Не вдалося підготувати пакет. Перевір вільне місце.");
    }
  }
  cancel_check();
  if (sha256(output) != SF_PACKAGE_SHA256) throw std::runtime_error("Перевірка пакета не пройдена. Завантаж інсталятор ще раз.");
}
bool identity(const fs::path& executable) {
  DWORD unused = 0, size = GetFileVersionInfoSizeW(executable.c_str(), &unused);
  if (!size) return false;
  std::vector<BYTE> data(size);
  if (!GetFileVersionInfoW(executable.c_str(), 0, size, data.data())) return false;
  struct Translation { WORD language, codepage; }* translation = nullptr;
  UINT length = 0;
  if (!VerQueryValueW(data.data(), L"\\VarFileInfo\\Translation", reinterpret_cast<void**>(&translation), &length) || length < sizeof(Translation)) return false;
  wchar_t query[100]; swprintf_s(query, L"\\StringFileInfo\\%04x%04x\\ProductName", translation->language, translation->codepage);
  wchar_t* name = nullptr;
  return VerQueryValueW(data.data(), query, reinterpret_cast<void**>(&name), &length) && name && wcscmp(name, L"SoundFlow") == 0;
}
std::wstring executable_version(const fs::path& executable){
  DWORD unused=0,size=GetFileVersionInfoSizeW(executable.c_str(),&unused);if(!size)return {};
  std::vector<BYTE> data(size);if(!GetFileVersionInfoW(executable.c_str(),0,size,data.data()))return {};
  VS_FIXEDFILEINFO* version=nullptr;UINT length=0;
  if(!VerQueryValueW(data.data(),L"\\",reinterpret_cast<void**>(&version),&length)||length<sizeof(*version)||version->dwSignature!=0xfeef04bd)return {};
  return std::to_wstring(HIWORD(version->dwProductVersionMS))+L"."+std::to_wstring(LOWORD(version->dwProductVersionMS))+L"."+std::to_wstring(HIWORD(version->dwProductVersionLS));
}
fs::path validate_target(const fs::path& chosen) {
  auto path = fs::absolute(chosen).lexically_normal();
  if (path.wstring().size() > 190 || path == path.root_path() || path.parent_path() == path.root_path() || path.wstring().rfind(L"\\\\", 0) == 0) throw std::runtime_error("Обери звичайну локальну папку для SoundFlow");
  for (auto parent = path; !parent.empty() && parent != parent.root_path(); parent = parent.parent_path()) {
    auto attributes = GetFileAttributesW(parent.c_str());
    if (attributes != INVALID_FILE_ATTRIBUTES && (attributes & FILE_ATTRIBUTE_REPARSE_POINT)) throw std::runtime_error("Папка встановлення не може бути посиланням на іншу папку");
  }
  if (fs::exists(path)) {
    bool owned=fs::is_regular_file(path/L"soundflow-install.json");
    wchar_t registered[32768];DWORD size=sizeof(registered);
    if (!owned && RegGetValueW(HKEY_CURRENT_USER,kUninstallKey,L"InstallLocation",RRF_RT_REG_SZ,nullptr,registered,&size)==ERROR_SUCCESS) owned=same_path(path,registered);
    if (!fs::is_directory(path) || (!fs::is_empty(path) && (!owned || !identity(path/L"SoundFlow.exe")))) throw std::runtime_error("У цій папці вже є інші файли. Обери окрему папку SoundFlow.");
  }
  return path;
}
struct Transaction {
  fs::path target, stage, backup; bool moved = false, committed = false, retain_stage = false;
  explicit Transaction(const fs::path& path) : target(path) {
    auto suffix = unique(); stage = target.parent_path() / (L".SoundFlow-stage-" + suffix); backup = target.parent_path() / (L".SoundFlow-backup-" + suffix);
    fs::create_directories(target.parent_path());
    if (!fs::create_directory(stage)) throw std::runtime_error("Не вдалося створити тимчасову папку");
  }
  void commit(bool inject_failure = false) {
    if (fs::exists(target)) { fs::rename(target, backup); moved = true; }
    try { if (inject_failure) throw std::runtime_error("injected rename failure"); fs::rename(stage, target); committed = true; }
    catch (...) { if (moved) { fs::rename(backup, target); moved = false; } throw; }
  }
  void rollback() {
    if (!committed) return;
    fs::rename(target, stage); committed = false;
    if (moved) { fs::rename(backup, target); moved = false; }
  }
  void finish() {
    // Older app folders may contain personal files. Retain that backup for recovery.
    // Recursive cleanup is limited to staging/temp directories created by this process.
    moved = false;
  }
  ~Transaction() {
    // stage/backup were generated here as fresh siblings; never recursively delete target.
    std::error_code error;if(!retain_stage)fs::remove_all(stage, error);
    // Preserve any backup if an exceptional restore failed.
  }
};
DWORD run_payload(const fs::path& payload,const fs::path& target,const fs::path& log,bool& retain_temporary) {
  auto command=quote(payload)+L" --silent --installto "+quote(target)+L" --log "+quote(log);
  STARTUPINFOW startup{sizeof(startup)};startup.dwFlags=STARTF_USESHOWWINDOW;startup.wShowWindow=SW_HIDE;
  PROCESS_INFORMATION process{};
  if(!CreateProcessW(payload.c_str(),command.data(),nullptr,nullptr,FALSE,CREATE_NO_WINDOW,nullptr,payload.parent_path().c_str(),&startup,&process))throw std::runtime_error("Не вдалося почати встановлення SoundFlow");
  Handle process_handle(process.hProcess),thread(process.hThread);
  trace(L"Velopack pid="+std::to_wstring(process.dwProcessId));
  // Backend mutates the final directory/registry. Never terminate it on Cancel
  // or a timer: it must complete its own install/rollback transaction.
  DWORD result=WaitForSingleObject(process.hProcess,INFINITE),code=1;
  if(result!=WAIT_OBJECT_0){retain_temporary=true;throw std::runtime_error("Не вдалося перевірити завершення встановлення");}
  if(!GetExitCodeProcess(process.hProcess,&code))throw std::runtime_error("Не вдалося прочитати результат встановлення");
  return code;
}
bool process_matches(HANDLE process, const fs::path& executable) {
  wchar_t path[32768]; DWORD count = 32768;
  return QueryFullProcessImageNameW(process, 0, path, &count) && same_path(fs::path(path), executable);
}
bool request_shutdown(DWORD pid) {
  struct Match {DWORD pid;UINT message;bool accepted=false;} match{pid,RegisterWindowMessageW(L"SoundFlow.Updater.Quit.v1")};
  EnumWindows([](HWND window,LPARAM value)->BOOL {
    auto match=reinterpret_cast<Match*>(value);DWORD pid=0;GetWindowThreadProcessId(window,&pid);
    if(pid==match->pid){DWORD_PTR result=0;if(SendMessageTimeoutW(window,match->message,0,0,SMTO_ABORTIFHUNG,1000,&result)&&result==1)match->accepted=true;}
    return TRUE;
  },reinterpret_cast<LPARAM>(&match));
  return match.accepted;
}
std::wstring waiting_text(bool acknowledged) {
  return acknowledged?L"Завершуємо роботу SoundFlow…":L"Обери «Вийти» у треї,\nщоб продовжити";
}
void wait_for_exit(HANDLE process,bool acknowledged) {
  for(int tries=0;WaitForSingleObject(process,100)==WAIT_TIMEOUT;++tries) {
    cancel_check();
    // Legacy versions have no updater protocol. The visible tray-exit
    // instruction stays actionable until the user exits or cancels.
    if(acknowledged&&tries>300)throw std::runtime_error("SoundFlow ще завершує роботу. Закрий його через меню в треї та повтори встановлення.");
  }
}
void wait_for_app(const fs::path& target) {
  DWORD incumbent = 0;
  std::ifstream(known(FOLDERID_RoamingAppData) / L"app.soundflow.desktop" / L"native-instance.pid") >> incumbent;
  wchar_t registered[32768];DWORD registered_size=sizeof(registered);fs::path registered_executable;
  if(RegGetValueW(HKEY_CURRENT_USER,kUninstallKey,L"InstallLocation",RRF_RT_REG_SZ,nullptr,registered,&registered_size)==ERROR_SUCCESS)registered_executable=fs::path(registered)/L"SoundFlow.exe";
  Handle snapshot(CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0));
  PROCESSENTRY32W entry{sizeof(entry)};
  if (snapshot.value == INVALID_HANDLE_VALUE || !Process32FirstW(snapshot.value, &entry)) throw std::runtime_error("Не вдалося перевірити запущені програми");
  do {
    if (_wcsicmp(entry.szExeFile, L"SoundFlow.exe") != 0) continue;
    Handle process(OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, FALSE, entry.th32ProcessID));
    if (!process.value) continue;
    bool registered_match=!registered_executable.empty()&&process_matches(process.value,registered_executable)&&identity(registered_executable);
    if (!process_matches(process.value, target / L"SoundFlow.exe")&&!registered_match) {
      if (entry.th32ProcessID != incumbent) continue;
      wchar_t running[32768]; DWORD length = 32768;
      if (!QueryFullProcessImageNameW(process.value, 0, running, &length) || !identity(running)) continue;
    }
    bool acknowledged=request_shutdown(entry.th32ProcessID);
    trace(L"Running SoundFlow pid="+std::to_wstring(entry.th32ProcessID)+(acknowledged?L"; shutdown acknowledged":L"; legacy shutdown requires tray exit"));
    send(Phase::Waiting,waiting_text(acknowledged));
    wait_for_exit(process.value,acknowledged);
  } while (Process32NextW(snapshot.value, &entry));
}
// Restore only the product's registration and exact shortcut files if installation fails.
struct IntegrationBackup {
  struct Value {std::wstring name;DWORD type;std::vector<BYTE> data;};
  struct Link {fs::path path;bool exists;std::vector<char> data;};
  bool existed=false;std::vector<Value> values;std::vector<Link> links;
  IntegrationBackup(){
    HKEY key{};auto result=RegOpenKeyExW(HKEY_CURRENT_USER,kUninstallKey,0,KEY_READ,&key);
    if(result==ERROR_SUCCESS){
      existed=true;DWORD count=0,max_name=0,max_data=0;
      if(RegQueryInfoKeyW(key,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,&count,&max_name,&max_data,nullptr,nullptr)!=ERROR_SUCCESS){RegCloseKey(key);throw std::runtime_error("Не вдалося зберегти попереднє встановлення");}
      for(DWORD i=0;i<count;++i){std::vector<wchar_t> name(max_name+2);Value value;value.data.resize(max_data);DWORD name_size=max_name+1,data_size=max_data;
        if(RegEnumValueW(key,i,name.data(),&name_size,nullptr,&value.type,value.data.data(),&data_size)!=ERROR_SUCCESS){RegCloseKey(key);throw std::runtime_error("Не вдалося зберегти попереднє встановлення");}
        value.name.assign(name.data(),name_size);value.data.resize(data_size);values.push_back(std::move(value));}
      RegCloseKey(key);
    }else if(result!=ERROR_FILE_NOT_FOUND)throw std::runtime_error("Не вдалося перевірити попереднє встановлення");
    for(auto path:{known(FOLDERID_Desktop)/L"SoundFlow.lnk",known(FOLDERID_Programs)/L"SoundFlow/SoundFlow.lnk"}){
      auto attributes=GetFileAttributesW(path.c_str());
      if(attributes!=INVALID_FILE_ATTRIBUTES&&(attributes&FILE_ATTRIBUTE_REPARSE_POINT))throw std::runtime_error("Ярлик SoundFlow не може бути посиланням на інший файл");
      Link link{path,fs::exists(path),{}};
      if(link.exists){if(!fs::is_regular_file(path)||fs::file_size(path)>1024*1024)throw std::runtime_error("Не вдалося зберегти ярлик SoundFlow");std::ifstream stream(path,std::ios::binary);if(!stream.is_open())throw std::runtime_error("Не вдалося прочитати ярлик SoundFlow");link.data.assign(std::istreambuf_iterator<char>(stream),{});if(stream.bad())throw std::runtime_error("Не вдалося прочитати ярлик SoundFlow");}
      links.push_back(std::move(link));
    }
  }
  void restore(){
    auto result=RegDeleteTreeW(HKEY_CURRENT_USER,kUninstallKey);
    if(result!=ERROR_SUCCESS&&result!=ERROR_FILE_NOT_FOUND)throw std::runtime_error("Не вдалося відновити попередню реєстрацію SoundFlow");
    if(existed){HKEY key{};if(RegCreateKeyExW(HKEY_CURRENT_USER,kUninstallKey,0,nullptr,0,KEY_WRITE,nullptr,&key,nullptr)!=ERROR_SUCCESS)throw std::runtime_error("Не вдалося відновити попередню реєстрацію SoundFlow");
      for(const auto& value:values){result=RegSetValueExW(key,value.name.c_str(),0,value.type,value.data.data(),static_cast<DWORD>(value.data.size()));if(result!=ERROR_SUCCESS){RegCloseKey(key);throw std::runtime_error("Не вдалося відновити попередню реєстрацію SoundFlow");}}RegCloseKey(key);}
    for(const auto& link:links){if(link.exists){fs::create_directories(link.path.parent_path());std::ofstream output(link.path,std::ios::binary);output.write(link.data.data(),link.data.size());output.close();if(!output)throw std::runtime_error("Не вдалося відновити ярлик SoundFlow");}else{std::error_code error;fs::remove(link.path,error);}}
  }
};
void launch(const fs::path& target) {
  auto executable = target / L"SoundFlow.exe";
  std::wstring command = quote(executable);
  STARTUPINFOW startup{sizeof(startup)}; PROCESS_INFORMATION process{};
  if (!CreateProcessW(executable.c_str(), command.data(), nullptr, nullptr, FALSE, 0, nullptr, target.c_str(), &startup, &process)) throw std::runtime_error("SoundFlow встановлено. Не вдалося запустити його автоматично — відкрий програму з робочого столу.");
  CloseHandle(process.hThread); CloseHandle(process.hProcess);
}
void install() {
  CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
  start_log();
  fs::path temporary;bool retain_temporary=false;
  try {
    auto target = validate_target(app.target);
    auto identity_path = known(FOLDERID_LocalAppData).wstring();
    std::transform(identity_path.begin(),identity_path.end(),identity_path.begin(),[](wchar_t c){return static_cast<wchar_t>(towlower(c));});
    // Different targets still share this user's registration and shortcut files.
    auto lock_name = L"Global\\SoundFlow.Setup." + std::to_wstring(std::hash<std::wstring>{}(identity_path));
    Handle install_lock(CreateMutexW(nullptr,TRUE,lock_name.c_str()));
    if (!install_lock.value || GetLastError()==ERROR_ALREADY_EXISTS) throw std::runtime_error("Інше встановлення SoundFlow вже працює. Зачекай, поки воно завершиться.");
    temporary = fs::temp_directory_path() / (L"SoundFlow-setup-" + unique());
    if (!fs::create_directory(temporary)) throw std::runtime_error("Не вдалося створити тимчасову папку");
    auto payload = temporary / L"payload.exe"; package(payload);
    Transaction transaction(target);
    IntegrationBackup registration;
    wait_for_app(target);cancel_check();
    app.committing=true;send(Phase::Commit,L"Встановлення…");
    transaction.commit();
    try{
      auto result=run_payload(payload,target,temporary/L"velopack.log",retain_temporary);
      trace(L"Velopack exit="+std::to_wstring(result));
      if(result!=0)throw std::runtime_error("Не вдалося встановити SoundFlow. Перевір вільне місце й повтори спробу.");
      for(auto name:{L"SoundFlow.exe",L"Update.exe",L"current/SoundFlow.exe",L"current/soundflow_lib.dll",L"current/flutter_windows.dll",L"current/data/app.so",L"current/sq.version"}){
        if(!fs::is_regular_file(target/name))throw std::runtime_error("Інсталяційний пакет неповний");}
      if(!identity(target/L"current/SoundFlow.exe")||executable_version(target/L"current/SoundFlow.exe")!=SF_VERSION)throw std::runtime_error("Версія встановленої програми не збігається з інсталятором");
    }catch(...){
      if(retain_temporary){transaction.retain_stage=true;transaction.finish();}
      else{transaction.rollback();registration.restore();}
      throw;
    }
    // Velopack owns uninstall registration and shortcuts. Keep any previous folder
    // as a recoverable backup, including user-added files.
    transaction.finish();
    send(Phase::Success, L"Готово. Приємного прослуховування");
    // Let the library's completion reveal finish before the app takes focus.
    if(!app.reduced_motion)Sleep(sf_vectors::circle_check.duration);
    launch(target);
  } catch (const fs::filesystem_error& error) {
    trace(wide(error.what()));app.committing=false;
    if(!app.cancelled)send(Phase::Error,L"Не вдалося записати файли SoundFlow. Перевір доступ до папки та вільне місце, потім повтори спробу.");
  } catch (const std::exception& error) { app.committing = false;if(app.cancelled)trace(wide(error.what()));else send(Phase::Error, wide(error.what())); }
  if (!temporary.empty()&&!retain_temporary) { std::error_code error; fs::remove_all(temporary, error); }
  if(app.cancelled)send(Phase::Home,L"");
  CoUninitialize();
}

#include "installer_canvas.h"
std::unique_ptr<InstallerCanvas> canvas;
InstallerSurface surface;
using Layout = sf_design::Layout;
using Box = sf_design::Box;
float curve(float value,const float* samples) {
  float index=std::clamp(value,0.f,1.f)*128;auto first=static_cast<unsigned>(index);
  return first>=128?1:samples[first]+(samples[first+1]-samples[first])*(index-first);
}
float ease(float value){return curve(value,sf_vectors::easeOut);}
float continuity(float value){return curve(value,sf_vectors::easeInOut);}
float mix(float a,float b,float t){return t<=0?a:t>=1?b:a+(b-a)*t;}
Box mix(Box a,Box b,float t){return {mix(a.x,b.x,t),mix(a.y,b.y,t),mix(a.width,b.width,t),mix(a.height,b.height,t)};}
Layout mix(Layout a,Layout b,float t){return {
  mix(a.width,b.width,t),mix(a.height,b.height,t),mix(a.radius,b.radius,t),
  mix(a.logo,b.logo,t),mix(a.title,b.title,t),mix(a.detail,b.detail,t),mix(a.action,b.action,t),
  mix(a.titleSize,b.titleSize,t),mix(a.actionRadius,b.actionRadius,t),mix(a.metadata,b.metadata,t),mix(a.folder,b.folder,t),
  mix(a.ring,b.ring,t),mix(a.check,b.check,t),mix(a.actionOpacity,b.actionOpacity,t),mix(a.outline,b.outline,t)};}
Layout layout_for(Phase value) {
  switch(value){
    case Phase::Home:return sf_design::readyLayout;
    case Phase::Consent:case Phase::Error:return sf_design::messageLayout;
    case Phase::Commit:return sf_design::commitLayout;
    case Phase::Success:return sf_design::completeLayout;
    default:return sf_design::progressLayout;
  }
}
struct Morph {bool active=false;Layout from=sf_design::readyLayout,to=sf_design::readyLayout;ULONGLONG started=0;} morph;
ULONGLONG phase_age(){return app.preview?(app.preview_ms<0?1000000:app.preview_ms):GetTickCount64()-app.phase_started;}
Layout resolve_layout(Layout from,Layout to,float elapsed){
  auto frame=mix(from,to,continuity(elapsed/sf_vectors::layoutMs));
  auto alpha=[&](float a,float b,float delay=sf_vectors::contentDelayMs){return mix(a,b,ease(b>a?(elapsed-delay)/sf_vectors::contentOpacityMs:elapsed/sf_vectors::contentExitMs));};
  frame.metadata=alpha(from.metadata,to.metadata,sf_vectors::revealMs);frame.folder=alpha(from.folder,to.folder,sf_vectors::revealMs);
  // Outgoing fields clear the action's path before the retained control crosses it.
  float secondary_exit=1-std::pow(1-ease(elapsed/sf_vectors::contentOpacityMs),3.f);
  if(to.metadata<from.metadata)frame.metadata=mix(from.metadata,to.metadata,secondary_exit);
  if(to.folder<from.folder)frame.folder=mix(from.folder,to.folder,secondary_exit);
  frame.ring=alpha(from.ring,to.ring);frame.check=alpha(from.check,to.check);frame.actionOpacity=alpha(from.actionOpacity,to.actionOpacity);
  return frame;
}
Layout display_layout(){
  if(app.preview){
    auto target=layout_for(app.phase);
    if(!app.reduced_motion&&app.preview_ms>=0&&(app.phase==Phase::Download||app.phase==Phase::Extract||app.phase==Phase::Success))
      return resolve_layout(app.phase==Phase::Success?sf_design::progressLayout:sf_design::readyLayout,target,static_cast<float>(app.preview_ms));
    return target;
  }
  return !morph.active||app.reduced_motion?morph.to:resolve_layout(morph.from,morph.to,static_cast<float>(GetTickCount64()-morph.started));
}
float display_width(){return display_layout().width;}
float display_height(){return display_layout().height;}
float display_progress(){return app.preview||app.reduced_motion?static_cast<float>(app.progress):app.progress_from+(static_cast<float>(app.progress)-app.progress_from)*ease(static_cast<float>(GetTickCount64()-app.progress_started)/sf_vectors::pressMs);}
RectF rect(Box box){return RectF(box.x,box.y,box.width,box.height);}
void audit_motion_frame(const Layout& frame,ULONGLONG sampled_at);void drive_motion();int motion_result=0;int encoder(CLSID* clsid);
bool same_layout(const Layout& a,const Layout& b){return memcmp(&a,&b,sizeof(Layout))==0;}

// Only changing labels resolve. Persistent logo/title/action geometry never fades out.
struct LabelMotion {
  std::wstring from,to;ULONGLONG started=0;
  void set(const std::wstring& value,bool animate) {
    if(value==to)return;
    if(animate&&active()&&GetTickCount64()-started<sf_vectors::contentExitMs){to=value;return;}
    from=to;to=value;started=animate&&!app.reduced_motion?GetTickCount64():0;
  }
  bool active()const{return started&&GetTickCount64()-started<sf_vectors::revealMs+sf_vectors::contentDelayMs;}
} detail_label,title_label,action_label;
std::wstring detail_for(Phase value,const std::wstring& message){
  if(value==Phase::Home||value==Phase::Success)return L"Музичний плеєр\nдля Windows";
  if(value==Phase::Consent)return L"Ознайомся з умовами на сайті SoundFlow\nта підтвердь згоду, щоб продовжити.";
  return message.empty()?L"Встановлення…":message;
}
std::wstring title_for(Phase value){return value==Phase::Consent?L"Перед початком":value==Phase::Error?L"Не вдалося завершити":L"SoundFlow";}
std::wstring action_for(Phase value){return value==Phase::Home?L"Завантажити":value==Phase::Consent?L"Погодитись і продовжити":value==Phase::Error?L"Спробувати ще раз":app.cancelled?L"Скасовуємо…":L"Скасувати";}

// Live windows and exports share one renderer, one physical-pixel surface and one layout.
Color background(sf_design::background), ink(sf_design::ink), muted(sf_design::muted), blue(sf_design::blue);
void rounded(InstallerCanvas& g, RectF rect, float radius, Color color, float border = 0) {
  g.rounded(rect,radius,color,border,muted);
}
void text(InstallerCanvas& g, const std::wstring& value, RectF rect, float size, Color color, bool bold = false, bool center = false) {
  g.text(value,rect,size,color,bold,center);
}
void icon(InstallerCanvas& g, float x, float y, float size) { if (app.icon) g.image(502,app.icon.get(),RectF(x,y,size,size)); }
void glyph(InstallerCanvas& g,int id,RectF rect,Color color) {
  if(id<601||id>607)return;
  const auto& icon=sf_vectors::icons[id-601];float progress=1;
  if(!app.reduced_motion) {
    if(id==607&&app.phase==Phase::Success)progress=std::min(1.f,static_cast<float>(phase_age())/icon.duration);
    else if(id==601&&app.hover==2)progress=std::min(1.f,static_cast<float>(GetTickCount64()-app.hover_started)/icon.duration);
    else if(id==603&&app.phase==Phase::Home)progress=std::min(1.f,static_cast<float>(phase_age())/icon.duration);
  }
  auto frame=std::min(icon.count-1,static_cast<unsigned>(std::lround(progress*(icon.count-1))));
  g.vector(icon.frames[frame],rect,color);
}
RectF button_rect(int id) {
  if (id == 1) return rect(layout_for(app.phase).action);
  if (id == 2) return RectF(sf_design::folderX,sf_design::folderY,sf_design::folderWidth,sf_design::folderHeight);
  if (id == 3) return RectF(207, 6, 17, 17);
  if (id == 4) return RectF(16, 99, 198, 22);
  if (id == 5) return RectF(16, 126, 198, 22);
  return RectF(6, 6, 17, 17);
}
bool visible(int id) {
  switch (id) {
    case 1: return app.phase != Phase::Success && app.phase != Phase::Commit;
    case 2: return app.phase == Phase::Home;
    case 3: return app.phase == Phase::Consent || app.phase == Phase::Error;
    case 4: case 5: case 6: return app.phase == Phase::Consent;
    default: return false;
  }
}
struct ControlValue {
  float from=0,to=0;ULONGLONG started=0;
  float value()const{return app.reduced_motion?to:mix(from,to,ease(static_cast<float>(GetTickCount64()-started)/sf_vectors::pressMs));}
  void set(float target){if(target!=to){from=value();to=target;started=GetTickCount64();}}
  bool active()const{return !app.reduced_motion&&GetTickCount64()-started<sf_vectors::pressMs;}
};
ControlValue hover_motion[7],press_motion[7];
void controls_changed(){for(int id=1;id<=6;++id){hover_motion[id].set(app.hover==id?1.f:0.f);press_motion[id].set(app.pressed==id?1.f:0.f);}}
Color blend(Color a,Color b,float t){return Color(static_cast<BYTE>(mix(a.GetA(),b.GetA(),t)),static_cast<BYTE>(mix(a.GetR(),b.GetR(),t)),static_cast<BYTE>(mix(a.GetG(),b.GetG(),t)),static_cast<BYTE>(mix(a.GetB(),b.GetB(),t)));}
void animated_text(InstallerCanvas& g,const LabelMotion& label,const std::wstring& fallback,RectF bounds,float size,Color color,bool bold=false,bool center=false,float opacity=1){
  if(app.preview&&app.preview_ms>=0&&!app.reduced_motion&&(app.phase==Phase::Download||app.phase==Phase::Extract||app.phase==Phase::Success)){
    bool completion=app.phase==Phase::Success;
    auto previous=&label==&detail_label?detail_for(completion?Phase::Extract:Phase::Home,L"Встановлення…"):&label==&action_label?action_for(completion?Phase::Extract:Phase::Home):title_for(Phase::Home);
    float elapsed=static_cast<float>(app.preview_ms);
    if(previous!=fallback){
      g.opacity(opacity*(1-std::clamp(elapsed/sf_vectors::contentExitMs,0.f,1.f)));text(g,previous,bounds,size,color,bold,center);
      g.opacity(opacity*ease((elapsed-sf_vectors::contentExitMs)/sf_vectors::contentOpacityMs));text(g,fallback,bounds,size,color,bold,center);return;
    }
  }
  if(app.preview||app.reduced_motion||!label.active()){
    g.opacity(opacity);text(g,fallback,bounds,size,color,bold,center);return;
  }
  auto elapsed=static_cast<float>(GetTickCount64()-label.started);
  g.opacity(opacity*(1-std::clamp(elapsed/sf_vectors::contentExitMs,0.f,1.f)));text(g,label.from,bounds,size,color,bold,center);
  g.opacity(opacity*ease((elapsed-sf_vectors::contentExitMs)/sf_vectors::contentOpacityMs));text(g,label.to,bounds,size,color,bold,center);
}
void draw_button(InstallerCanvas& g,int id,const Layout& frame,bool focus=false){
  auto bounds=id==1?rect(frame.action):button_rect(id);
  float alpha=id==1?frame.actionOpacity:id==2?frame.folder:1;
  if(alpha<=.001f)return;
  float pressed=press_motion[id].value(),hover=hover_motion[id].value();
  float scale=1-.03f*pressed;
  bounds.X+=bounds.Width*(1-scale)/2;bounds.Y+=bounds.Height*(1-scale)/2;bounds.Width*=scale;bounds.Height*=scale;
  const bool enabled=id!=1||app.phase!=Phase::Consent||(app.checked&&wcslen(SF_TERMS_URL)>0);
  g.opacity(alpha);
  if(id==1){
    auto fill=blend(blue,background,frame.outline);
    fill=blend(fill,ink,.05f*hover+.05f*pressed);
    g.rounded(bounds,frame.actionRadius,enabled?fill:Color(255,40,50,64),.65f*frame.outline,muted);
    animated_text(g,action_label,action_for(app.phase),bounds,9.2f,enabled?ink:muted,false,true,alpha);
  }else if(id==2){
    rounded(g,bounds,sf_design::folderRadius,blend(Color(255,42,42,46),ink,.05f*hover+.05f*pressed));glyph(g,601,RectF(bounds.X+4.5f,bounds.Y+4.5f,13,13),ink);
  }else if(id==3||id==6){
    glyph(g,id==3?604:605,RectF(bounds.X+3.5f,bounds.Y+3.5f,10,10),muted);
  }else if(id==4){
    text(g,wcslen(SF_TERMS_URL)?L"Умови використання  ↗":L"Посилання на умови ще не додано",bounds,8.5f,wcslen(SF_TERMS_URL)?blue:muted);
  }else if(id==5){
    rounded(g,RectF(16,130,11,11),3,app.checked?blue:Color(255,50,50,55));
    if(app.checked)glyph(g,606,RectF(17,131,9,9),ink);
    text(g,L"Приймаю умови використання",RectF(33,130,181,18),7.8f,ink);
  }
  g.opacity(alpha);
  if(focus)g.rounded(RectF(bounds.X-sf_design::focusGap,bounds.Y-sf_design::focusGap,bounds.Width+2*sf_design::focusGap,bounds.Height+2*sf_design::focusGap),id==1?frame.actionRadius+sf_design::focusGap:id==2?sf_design::folderFocusRadius:sf_design::compactControl,Color(0,0,0,0),.8f,Color(255,156,199,255));
  g.opacity(1);
}
void paint(InstallerCanvas& g,const Layout& frame){
  g.clear(background);
  // One retained logo and title, at the current interpolated geometry.
  icon(g,frame.logo.x,frame.logo.y,frame.logo.width);
  animated_text(g,title_label,title_for(app.phase),rect(frame.title),frame.titleSize,ink,true);
  animated_text(g,detail_label,detail_for(app.phase,app.text),rect(frame.detail),8.6f,muted);
  g.opacity(frame.metadata*frame.metadata*frame.metadata);
  text(g,L"SoundFlow",RectF(93,43,117,12),7.8f,Color(255,156,149,216));
  g.opacity(frame.metadata);
  text(g,SF_VERSION,RectF(15,99,58,17),7.6f,ink,false,true);
  text(g,L"Версія",RectF(15,118,58,13),7.2f,muted,false,true);
  g.line(76,105,76,121,Color(255,49,49,53),.8f);g.line(154,105,154,121,Color(255,49,49,53),.8f);
  glyph(g,603,RectF(110,99,14,14),muted);
  text(g,L"Автооновлення",RectF(80,118,70,13),7.2f,muted,false,true);
  glyph(g,602,RectF(181,99,14,14),muted);
  std::wostringstream size;size<<std::fixed<<std::setprecision(1)<<(SF_PACKAGE_SIZE/1048576.0)<<L" МБ";
  text(g,size.str(),RectF(162,118,52,13),7.2f,muted,false,true);
  g.opacity(frame.ring);
  if(frame.ring>.001f){
    auto x=frame.logo.x+frame.logo.width/2-24,y=frame.logo.y+frame.logo.height/2-24;
    g.ellipse(x,y,48,48,Color(255,48,48,54),1.8f);
    const float start=app.progress<0?app.reduced_motion?-90:static_cast<float>((GetTickCount64()/4)%360):-90;
    g.arc(x,y,48,48,start,app.progress<0?100.f:std::max(0.f,display_progress())*360,blue,1.8f);
  }
  g.opacity(frame.check);if(frame.check>.001f)glyph(g,607,RectF(241,29,20,20),ink);
  g.opacity(1);
  HWND buttons[]={app.primary,app.folder,app.close,app.link,app.consent,app.back};
  for(int id=1;id<=6;++id)if(id==1||id==2||visible(id))draw_button(g,id,frame,app.keyboard_focus&&(app.preview?id==1:GetFocus()==buttons[id-1]));
}
void render_surface(bool present){
  const auto sampled_at=GetTickCount64();
  const auto frame=display_layout();
  const float left=(sf_design::viewportWidth-frame.width)/2,top=(sf_design::viewportHeight-frame.height)/2;
  surface.resize(static_cast<int>(std::lround(sf_design::viewportWidth*app.dpi)),static_cast<int>(std::lround(sf_design::viewportHeight*app.dpi)));
  canvas->begin(surface.dc(),surface.width(),surface.height(),app.dpi,left,top);paint(*canvas,frame);canvas->end();
  surface.mask(RectF(left*app.dpi,top*app.dpi,frame.width*app.dpi,frame.height*app.dpi),std::min(frame.radius,frame.height/2)*app.dpi);
  // The HWND and DIB viewport stay fixed: a frame never exposes an old-size bitmap.
  if(present){float alpha=app.preview||app.reduced_motion?1:ease(static_cast<float>(GetTickCount64()-app.opened)/sf_vectors::dialogMs);surface.present(app.window,static_cast<BYTE>(std::lround(alpha*255)));if(app.verify_motion)audit_motion_frame(frame,sampled_at);}
}
void repaint_window(HWND window){
  try{if(canvas)render_surface(true);}
  catch(const std::exception& error){
    if(app.verify_motion)motion_result=25;
    else MessageBoxW(window,wide(error.what()).c_str(),L"SoundFlow",MB_OK|MB_ICONERROR);
    DestroyWindow(window);
  }
}
void layout() {
  const auto frame=layout_for(app.phase);float offset_x=(sf_design::viewportWidth-frame.width)/2,offset_y=(sf_design::viewportHeight-frame.height)/2;
  HWND buttons[] = {app.primary,app.folder,app.close,app.link,app.consent,app.back};
  for (int id=1;id<=6;++id) {
    auto rect = button_rect(id); auto button = buttons[id-1];
    int left=static_cast<int>(std::lround((offset_x+rect.X)*app.dpi)),top=static_cast<int>(std::lround((offset_y+rect.Y)*app.dpi));
    EnableWindow(button,!morph.active);
    MoveWindow(button,left,top,static_cast<int>(std::lround((offset_x+rect.GetRight())*app.dpi))-left,static_cast<int>(std::lround((offset_y+rect.GetBottom())*app.dpi))-top,FALSE);
    ShowWindow(button,visible(id) ? SW_SHOW : SW_HIDE);
  }
  EnableWindow(app.primary,!morph.active&&(app.phase != Phase::Consent || (app.checked && wcslen(SF_TERMS_URL)>0)));
  EnableWindow(app.folder,!morph.active);
  EnableWindow(app.consent,wcslen(SF_TERMS_URL)>0);
  SetWindowTextW(app.primary,app.phase==Phase::Consent?L"Погодитись і продовжити":app.phase==Phase::Home?L"Завантажити":app.phase==Phase::Error?L"Спробувати ще раз":L"Скасувати");
  InvalidateRect(app.window,nullptr,FALSE);
}
void phase(Phase value,const std::wstring& message=L"",double progress=-1){
  if(value!=Phase::Success)KillTimer(app.window,2);
  const bool changed=app.phase!=value;
  auto target=layout_for(value);
  // Retarget from the actual frame, including interrupted/reversed transitions.
  // Sibling compact phases share an endpoint and never restart its clock.
  if(!same_layout(target,morph.to)){
    auto previous=display_layout();morph={ !app.reduced_motion,previous,target,GetTickCount64()};
  }
  app.progress_from=app.progress<0?0:display_progress();app.progress_started=GetTickCount64();
  if(changed)app.phase_started=GetTickCount64();
  app.phase=value;app.text=message;app.progress=progress;
  title_label.set(title_for(value),changed);
  detail_label.set(detail_for(value,message),changed);
  action_label.set(action_for(value),changed);
  if(changed){app.hover=0;app.pressed=0;controls_changed();}
  SetWindowTextW(app.window,(message.empty()?L"SoundFlow — Встановлення":L"SoundFlow — "+message).c_str());
  if(value==Phase::Success)SetTimer(app.window,2,1800,nullptr);
  layout();
  repaint_window(app.window);
}
void begin() {
  if (app.worker.joinable()) app.worker.join();
  app.cancelled=false;app.committing=false;
  phase(embedded()?Phase::Extract:Phase::Download,embedded()?L"Готуємо файли…":L"Завантаження…");
  if(!app.verify_motion)app.worker=std::thread(install);
}
void pick_folder() {
  IFileDialog* dialog=nullptr;
  if (SUCCEEDED(CoCreateInstance(CLSID_FileOpenDialog,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&dialog)))) {
    DWORD options=0;dialog->GetOptions(&options);dialog->SetOptions(options|FOS_PICKFOLDERS|FOS_FORCEFILESYSTEM|FOS_PATHMUSTEXIST);
    dialog->SetTitle(L"Обери папку — SoundFlow буде встановлено всередині");dialog->SetOkButtonLabel(L"Обрати");
    if (SUCCEEDED(dialog->Show(app.window))) {
      IShellItem* item=nullptr;
      if (SUCCEEDED(dialog->GetResult(&item))) { PWSTR path=nullptr; if (SUCCEEDED(item->GetDisplayName(SIGDN_FILESYSPATH,&path))) {
        try { fs::path parent(path); app.target=validate_target(_wcsicmp(parent.filename().c_str(),L"SoundFlow")==0?parent:parent/L"SoundFlow"); }
        catch (const std::exception& e) { MessageBoxW(app.window,wide(e.what()).c_str(),L"SoundFlow",MB_OK|MB_ICONINFORMATION); }
        CoTaskMemFree(path); } item->Release(); }
    }
    dialog->Release();layout();
  }
}
LRESULT CALLBACK button_proxy(HWND window,UINT message,WPARAM wparam,LPARAM lparam,UINT_PTR id,DWORD_PTR) {
  // Native buttons retain keyboard/accessibility behavior, but never own visible pixels.
  switch(message) {
    case WM_PAINT:{PAINTSTRUCT paint{};BeginPaint(window,&paint);EndPaint(window,&paint);return 0;}
    case WM_ERASEBKGND:return 1;
    case WM_PRINTCLIENT:return 0;
    case WM_SETCURSOR:SetCursor(LoadCursorW(nullptr,IDC_HAND));return TRUE;
    case WM_MOUSEMOVE:
      if(app.hover!=static_cast<int>(id)){app.hover=static_cast<int>(id);app.hover_started=GetTickCount64();controls_changed();InvalidateRect(app.window,nullptr,FALSE);}
      {TRACKMOUSEEVENT track{sizeof(track),TME_LEAVE,window,0};TrackMouseEvent(&track);}break;
    case WM_MOUSELEAVE:app.hover=0;controls_changed();InvalidateRect(app.window,nullptr,FALSE);break;
    case WM_LBUTTONDOWN:app.keyboard_focus=false;app.pressed=static_cast<int>(id);controls_changed();InvalidateRect(app.window,nullptr,FALSE);break;
    case WM_LBUTTONUP:case WM_CAPTURECHANGED:case WM_CANCELMODE:app.pressed=0;controls_changed();InvalidateRect(app.window,nullptr,FALSE);break;
    case WM_SETFOCUS:case WM_KILLFOCUS:case WM_ENABLE:case WM_UPDATEUISTATE:InvalidateRect(app.window,nullptr,FALSE);break;
  }
  return DefSubclassProc(window,message,wparam,lparam);
}
LRESULT CALLBACK procedure(HWND window,UINT message,WPARAM wparam,LPARAM lparam) {
  switch (message) {
    case WM_ERASEBKGND:return 1;
    case WM_PAINT: {
      PAINTSTRUCT ps{};BeginPaint(window,&ps);EndPaint(window,&ps);
      repaint_window(window);
      return 0;
    }
    case WM_DRAWITEM:return TRUE;
    case WM_COMMAND:if (HIWORD(wparam)==BN_CLICKED) {
      if (app.preview) return 0;
      switch (LOWORD(wparam)) {
        case 1:
          if (app.phase==Phase::Home) {
            app.checked=false;
            if (wcslen(SF_TERMS_URL)>0) { phase(Phase::Consent);SetFocus(app.link); }
            else begin();
          }
          else if (app.phase==Phase::Consent && app.checked && wcslen(SF_TERMS_URL)>0) begin();
          else if (app.phase==Phase::Error) phase(Phase::Home);
          else if(!app.committing) { app.cancelled=true;action_label.set(action_for(app.phase),true);SetWindowTextW(app.primary,L"Скасовуємо…");EnableWindow(app.primary,FALSE); }
          break;
        case 2:pick_folder();break;
        case 3:PostMessageW(window,WM_CLOSE,0,0);break;
        case 4:if (wcslen(SF_TERMS_URL)) ShellExecuteW(window,L"open",SF_TERMS_URL,nullptr,nullptr,SW_SHOWNORMAL);break;
        case 5:app.checked=!app.checked;layout();break;
        case 6:phase(Phase::Home);SetFocus(app.primary);break;
      } return 0;
    }break;
    case kEvent: { std::unique_ptr<Event> event(reinterpret_cast<Event*>(lparam));phase(event->phase,event->text,event->progress);return 0; }
    case WM_TIMER:
      if(wparam==3){drive_motion();return 0;}
      if (wparam==2) { KillTimer(window,2);DestroyWindow(window); }
      else if(!app.reduced_motion) {
        bool moving=morph.active;
        if(morph.active&&GetTickCount64()-morph.started>=sf_vectors::layoutMs){morph.active=false;layout();}
        bool progress=app.progress<0&&(app.phase==Phase::Download||app.phase==Phase::Extract||app.phase==Phase::Waiting||app.phase==Phase::Commit);
        bool icon=app.phase==Phase::Success&&phase_age()<sf_vectors::circle_check.duration||app.phase==Phase::Home&&phase_age()<sf_vectors::refresh_cw.duration||app.hover==2&&GetTickCount64()-app.hover_started<sf_vectors::folder_open.duration;
        bool interaction=false;for(int id=1;id<=6;++id)interaction=interaction||hover_motion[id].active()||press_motion[id].active();
        bool active=moving||progress||icon||interaction||detail_label.active()||title_label.active()||action_label.active()||GetTickCount64()-app.opened<sf_vectors::dialogMs||GetTickCount64()-app.progress_started<sf_vectors::pressMs;
        if(active||app.was_animating)repaint_window(window);
        app.was_animating=active;
      }
      return 0;
    case WM_NCHITTEST: {
      LRESULT hit=DefWindowProcW(window,message,wparam,lparam);
      if (hit==HTCLIENT&&!morph.active) { POINT point{GET_X_LPARAM(lparam),GET_Y_LPARAM(lparam)};ScreenToClient(window,&point);auto frame=display_layout();if (point.y<static_cast<int>(((sf_design::viewportHeight-frame.height)/2+22)*app.dpi)) return HTCAPTION; }
      return hit;
    }
    case WM_DPICHANGED:{
      app.dpi=HIWORD(wparam)/96.0f*kContentScale;auto suggested=reinterpret_cast<RECT*>(lparam);
      SetWindowPos(window,nullptr,suggested->left,suggested->top,static_cast<int>(std::lround(sf_design::viewportWidth*app.dpi)),static_cast<int>(std::lround(sf_design::viewportHeight*app.dpi)),SWP_NOZORDER|SWP_NOACTIVATE);
      layout();return 0;
    }
    case WM_CLOSE:
      if (app.preview) { DestroyWindow(window);return 0; }
      if (app.phase==Phase::Download || app.phase==Phase::Extract || app.phase==Phase::Waiting) { if (!app.committing) app.cancelled=true;return 0; }
      if (app.phase!=Phase::Commit) DestroyWindow(window);return 0;
    case WM_DESTROY:PostQuitMessage(0);return 0;
  }
  return DefWindowProcW(window,message,wparam,lparam);
}
int encoder(CLSID* clsid) {
  UINT count=0,size=0;GetImageEncodersSize(&count,&size);std::vector<BYTE> data(size);auto info=reinterpret_cast<ImageCodecInfo*>(data.data());GetImageEncoders(count,size,info);
  for (UINT i=0;i<count;++i) if (wcscmp(info[i].MimeType,L"image/png")==0) { *clsid=info[i].Clsid;return 0; }return -1;
}
// Private live UI regression: real commands/events/presentation; no worker or install.
struct MotionAudit {
  std::ofstream log;RECT bounds{};ULONGLONG last_time=0,last_capture=0;Layout last=sf_design::readyLayout;int frames=0,step=0;bool delayed[2]={false,false};
} motion_audit;
void audit_motion_frame(const Layout& frame,ULONGLONG sampled_at){
  auto elapsed=sampled_at-app.opened;
  if(!motion_audit.log.is_open()){
    fs::create_directories(app.motion_output);motion_audit.log.open(app.motion_output/L"frames.csv");
    motion_audit.log<<"ms,phase,width,height,logo_x,logo_y,logo_size,title_x,title_y,action_x,action_y,logo_ink,title_ink,hash\n";
    GetWindowRect(app.window,&motion_audit.bounds);
  }
  RECT bounds{};GetWindowRect(app.window,&bounds);
  if(memcmp(&bounds,&motion_audit.bounds,sizeof(RECT))!=0)motion_result=21;
  auto bright=[&](Box box){
    int result=0;float left=(sf_design::viewportWidth-frame.width)/2,top=(sf_design::viewportHeight-frame.height)/2;
    for(int y=std::max(0,static_cast<int>((top+box.y)*app.dpi));y<std::min(surface.height(),static_cast<int>((top+box.y+box.height)*app.dpi));++y)
      for(int x=std::max(0,static_cast<int>((left+box.x)*app.dpi));x<std::min(surface.width(),static_cast<int>((left+box.x+box.width)*app.dpi));++x){auto pixel=surface.pixel(x,y);if(((pixel>>16)&255)>120&&((pixel>>8)&255)>120&&(pixel&255)>120)++result;}
    return result;
  };
  int logo_ink=bright(frame.logo),title_ink=bright(frame.title);
  if(logo_ink<10||title_ink<10)motion_result=22;
  if(!morph.active&&(app.phase==Phase::Home||app.phase==Phase::Extract||app.phase==Phase::Waiting)){
    RECT hit{};GetWindowRect(app.primary,&hit);MapWindowPoints(nullptr,app.window,reinterpret_cast<POINT*>(&hit),2);
    const auto target=layout_for(app.phase);auto action=target.action;
    int left=static_cast<int>(std::lround(((sf_design::viewportWidth-target.width)/2+action.x)*app.dpi));
    int top=static_cast<int>(std::lround(((sf_design::viewportHeight-target.height)/2+action.y)*app.dpi));
    if(!IsWindowEnabled(app.primary)||hit.left!=left||hit.top!=top)motion_result=26;
  }
  if(motion_audit.frames&&!app.reduced_motion){
    float allowed=1.5f+1.5f*static_cast<float>(elapsed-motion_audit.last_time);
    auto delta=[](Box a,Box b){return std::max({std::abs(a.x-b.x),std::abs(a.y-b.y),std::abs(a.width-b.width),std::abs(a.height-b.height)});};
    if(delta(frame.logo,motion_audit.last.logo)>allowed||delta(frame.title,motion_audit.last.title)>allowed||std::abs(frame.width-motion_audit.last.width)>allowed||std::abs(frame.height-motion_audit.last.height)>allowed)motion_result=23;
  }
  motion_audit.log<<elapsed<<','<<static_cast<int>(app.phase)<<','<<frame.width<<','<<frame.height<<','<<frame.logo.x<<','<<frame.logo.y<<','<<frame.logo.width<<','<<frame.title.x<<','<<frame.title.y<<','<<frame.action.x<<','<<frame.action.y<<','<<logo_ink<<','<<title_ink<<','<<surface.hash()<<'\n';
  if(elapsed-motion_audit.last_capture>=32){
    CLSID png{};encoder(&png);surface.save((app.motion_output/(std::to_wstring(elapsed)+L".png")).wstring(),png);motion_audit.last_capture=elapsed;
  }
  motion_audit.last=frame;motion_audit.last_time=elapsed;++motion_audit.frames;
}
void drive_motion(){
  auto elapsed=GetTickCount64()-app.opened;
  const unsigned moments[]={350,400,450,500,800,1020,1800,1940,2040,2500};
  // PNG capture can delay delivery at high DPI; finish after the final real frame,
  // rather than cutting a queued reversal off at an arbitrary wall-clock deadline.
  if(motion_audit.step==9&&(morph.active||app.phase!=Phase::Home))return;
  if(motion_audit.step<10&&elapsed>=moments[motion_audit.step]){
    switch(motion_audit.step++){
      case 0:PostMessageW(app.window,WM_COMMAND,MAKEWPARAM(1,BN_CLICKED),reinterpret_cast<LPARAM>(app.primary));break;
      case 1:send(Phase::Extract,L"Готуємо файли…");break;
      case 2:send(Phase::Waiting,L"Готуємо встановлення…");break;
      case 3:send(Phase::Extract,L"Встановлення…");break;
      case 4:send(Phase::Commit,L"Завершуємо встановлення…");break;
      case 5:send(Phase::Success,L"Готово");break;
      case 6:send(Phase::Home,L"");break;
      case 7:send(Phase::Extract,L"Готуємо файли…");break;
      case 8:send(Phase::Home,L"");break;
      case 9:
        if(motion_audit.frames<(app.reduced_motion?6:20)||!same_layout(display_layout(),sf_design::readyLayout))motion_result=24;
        motion_audit.log.flush();DestroyWindow(app.window);break;
    }
  }
  for(int i=0;i<2;++i)if(!motion_audit.delayed[i]&&elapsed>=static_cast<ULONGLONG>(i?1450:600)){motion_audit.delayed[i]=true;Sleep(45);}
}
int self_test() {
  auto root=fs::temp_directory_path()/(L"SoundFlow-transaction-test-"+unique());fs::create_directory(root);
  bool retain_root=false;
  try {
    auto target=root/L"SoundFlow";fs::create_directory(target);std::ofstream(target/L"old.txt")<<"old";
    { Transaction test(target);std::ofstream(test.stage/L"new.txt")<<"new";try { test.commit(true);return 2; }catch (...) {}if (!fs::exists(target/L"old.txt")) return 3; }
    { Transaction test(target);std::ofstream(test.stage/L"new.txt")<<"new";test.commit();if (!fs::exists(target/L"new.txt")) return 4;test.rollback();if (!fs::exists(target/L"old.txt")) return 5; }
    { Transaction test(target);std::ofstream(test.stage/L"new.txt")<<"new";test.commit();test.finish(); }
    if (!fs::exists(target/L"new.txt") || fs::exists(target/L"old.txt")) return 6;
    if(embedded()) {
      auto payload=root/L"payload.exe";package(payload);
      if(sha256(payload)!=SF_PACKAGE_SHA256)return 8;
    }
    fs::remove_all(root);return 0;
  }catch (...) { std::error_code error;if(!retain_root)fs::remove_all(root,error);return 7; }
}
bool verify_surface() {
  // Regression for parent repaints erasing the blue button and the folder glyph.
  const float original=app.dpi;
  for(float dpi:{1.f,1.25f,1.5f,2.f}) {
    app.dpi=kContentScale*dpi;app.hover=0;app.pressed=0;app.keyboard_focus=false;layout();render_surface(true);auto baseline=surface.hash();
    auto sample=surface.pixel(static_cast<int>((26+40)*app.dpi),static_cast<int>((2+154)*app.dpi));
    if(((sample>>16)&255)>60||((sample>>8)&255)<90||((sample>>8)&255)>180||(sample&255)<220){app.dpi=original;return false;}
    int folder_ink=0;
    for(int y=static_cast<int>((2+148)*app.dpi);y<static_cast<int>((2+160)*app.dpi);++y)
      for(int x=static_cast<int>((26+197)*app.dpi);x<static_cast<int>((26+211)*app.dpi);++x){auto pixel=surface.pixel(x,y);if(((pixel>>16)&255)>170&&((pixel>>8)&255)>170&&(pixel&255)>170)++folder_ink;}
    if(folder_ink<12){app.dpi=original;return false;}
    for(int iteration=0;iteration<12;++iteration){
      SendMessageW(app.primary,WM_PAINT,0,0);SendMessageW(app.folder,WM_PAINT,0,0);
      SendMessageW(app.window,WM_TIMER,1,0);render_surface(false);
      if(surface.hash()!=baseline){app.dpi=original;return false;}
    }
  }
  // Completion uses real library motion and reaches its exact static endpoint.
  bool reduced=app.reduced_motion;app.reduced_motion=false;
  app.phase=Phase::Success;
  app.preview_ms=0;render_surface(false);auto first=surface.hash();
  app.preview_ms=320;render_surface(false);auto middle=surface.hash();
  app.preview_ms=1000;render_surface(false);auto last=surface.hash();
  if(first==middle||middle==last)return false;
  app.reduced_motion=true;app.preview_ms=0;render_surface(false);
  if(surface.hash()!=last)return false;
  app.reduced_motion=false;app.phase=Phase::Download;app.progress=.42;
  app.preview_ms=0;render_surface(false);float start_width=display_width(),start_height=display_height();int viewport_width=surface.width(),viewport_height=surface.height();
  app.preview_ms=180;render_surface(false);float middle_width=display_width(),middle_height=display_height();
  app.preview_ms=360;render_surface(false);
  if(!(start_width<middle_width&&middle_width<display_width()&&start_height>middle_height&&middle_height>display_height())||viewport_width!=surface.width()||viewport_height!=surface.height())return false;
  app.preview_ms=-1;app.reduced_motion=reduced;app.phase=Phase::Home;
  app.progress=-1;layout();
  // A window without the updater protocol must not be mistaken for an ACK.
  if(request_shutdown(GetCurrentProcessId()))return false;
  WNDCLASSW responder{};responder.hInstance=app.instance;responder.lpszClassName=L"SoundFlow.Setup.TestResponder";
  responder.lpfnWndProc=[](HWND window,UINT message,WPARAM wparam,LPARAM lparam)->LRESULT {
    if(message==RegisterWindowMessageW(L"SoundFlow.Updater.Quit.v1"))return 1;
    return DefWindowProcW(window,message,wparam,lparam);
  };
  RegisterClassW(&responder);
  auto mock=CreateWindowW(responder.lpszClassName,L"",WS_POPUP,0,0,1,1,nullptr,nullptr,app.instance,nullptr);
  bool acknowledged=mock&&request_shutdown(GetCurrentProcessId());
  if(mock)DestroyWindow(mock);UnregisterClassW(responder.lpszClassName,app.instance);
  if(!acknowledged)return false;
  bool cancelled=false;app.cancelled=true;
  try{wait_for_exit(GetCurrentProcess(),false);}catch(const std::exception&){cancelled=true;}
  app.cancelled=false;if(!cancelled)return false;
  app.dpi=original;return true;
}
int WINAPI wWinMain(HINSTANCE instance,HINSTANCE,PWSTR,int) {
  app.instance=instance;SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);CoInitializeEx(nullptr,COINIT_APARTMENTTHREADED);
  BOOL animations=TRUE;SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION,0,&animations,0);app.reduced_motion=!animations;
  GdiplusStartupInput input;ULONG_PTR gdiplus=0;GdiplusStartup(&gdiplus,&input,nullptr);
  INITCOMMONCONTROLSEX controls{sizeof(controls),ICC_STANDARD_CLASSES};InitCommonControlsEx(&controls);
  try {canvas=std::make_unique<InstallerCanvas>(instance);}catch(const std::exception& error){
    std::cerr<<error.what()<<std::endl;
    if(!wcsstr(GetCommandLineW(),L"--self-test")&&!wcsstr(GetCommandLineW(),L"--verify-ui")&&!wcsstr(GetCommandLineW(),L"--verify-motion")&&!wcsstr(GetCommandLineW(),L"--capture"))MessageBoxW(nullptr,wide(error.what()).c_str(),L"SoundFlow",MB_OK|MB_ICONERROR);
    GdiplusShutdown(gdiplus);CoUninitialize();return 1;
  }
  app.icon=png_resource(502);
  for (int id=601;id<=607;++id) app.glyphs.emplace(id,png_resource(id));
  int count=0;auto args=CommandLineToArgvW(GetCommandLineW(),&count);std::wstring preview,capture;
  app.target=known(FOLDERID_LocalAppData)/L"SoundFlow";
  wchar_t installed[32768];DWORD bytes=sizeof(installed);
  if (RegGetValueW(HKEY_CURRENT_USER,kUninstallKey,L"InstallLocation",RRF_RT_REG_SZ,nullptr,installed,&bytes)==ERROR_SUCCESS) app.target=installed;
  for (int i=1;i<count;++i) {
    std::wstring arg=args[i];
    if (arg==L"--self-test") { int result=self_test();LocalFree(args);canvas.reset();app.glyphs.clear();app.icon.reset();GdiplusShutdown(gdiplus);CoUninitialize();return result; }
    else if (arg==L"--verify-ui") {app.verify_ui=true;app.preview=true;}
    else if (arg==L"--reduced-motion") app.reduced_motion=true;
    else if (i+1<count&&arg==L"--verify-motion") {app.verify_motion=true;app.motion_output=args[++i];}
    else if (i+1<count&&arg==L"--motion-ms") app.preview_ms=std::max(0,_wtoi(args[++i]));
    else if (i+1<count&&arg==L"--dpi-scale") {app.dpi_override=true;app.dpi=kContentScale*std::clamp(static_cast<float>(wcstod(args[++i],nullptr)),.75f,3.f);}
    else if (i+1<count && arg==L"--target") app.target=args[++i];
    else if (i+1<count && arg==L"--preview") preview=args[++i];
    else if (i+1<count && arg==L"--capture") capture=args[++i];
  }
  LocalFree(args);
  // Never hold the installation directory open while renaming it during an update.
  SetCurrentDirectoryW(fs::temp_directory_path().c_str());
  if (!preview.empty()) {
    app.preview=true;app.phase=preview==L"terms"?Phase::Consent:preview==L"progress"?Phase::Download:preview==L"waiting"?Phase::Waiting:preview==L"complete"?Phase::Success:preview==L"error"?Phase::Error:Phase::Home;
    app.text=app.phase==Phase::Waiting?waiting_text(false):app.phase==Phase::Download?L"Завантаження · 42%":app.phase==Phase::Error?L"Не вдалося завантажити SoundFlow. Перевір підключення до інтернету.":L"Готово. Приємного прослуховування";app.progress=.42;
    if (!capture.empty()) {
      render_surface(false);CLSID clsid{};encoder(&clsid);auto status=surface.save(capture,clsid);
      canvas.reset();app.glyphs.clear();app.icon.reset();GdiplusShutdown(gdiplus);CoUninitialize();return status==Ok?0:1;
    }
  }
  if(app.verify_motion){app.preview=false;app.phase=Phase::Home;}
  WNDCLASSEXW cls{sizeof(cls)};cls.lpfnWndProc=procedure;cls.hInstance=instance;cls.hCursor=LoadCursorW(nullptr,IDC_ARROW);cls.lpszClassName=L"SoundFlow.Setup.v1";cls.hIcon=LoadIconW(instance,MAKEINTRESOURCEW(101));RegisterClassExW(&cls);
  if(!app.dpi_override)app.dpi=GetDpiForSystem()/96.0f*kContentScale;int width=static_cast<int>(std::lround(sf_design::viewportWidth*app.dpi)),height=static_cast<int>(std::lround(sf_design::viewportHeight*app.dpi));
  RECT desktop{};SystemParametersInfoW(SPI_GETWORKAREA,0,&desktop,0);
  app.window=CreateWindowExW(WS_EX_LAYERED|WS_EX_APPWINDOW|WS_EX_CONTROLPARENT,cls.lpszClassName,L"SoundFlow — Встановлення",WS_POPUP|WS_SYSMENU,desktop.left+(desktop.right-desktop.left-width)/2,desktop.top+(desktop.bottom-desktop.top-height)/2,width,height,nullptr,nullptr,instance,nullptr);
  if (!app.window) return 1;
  if(app.verify_motion)SetWindowPos(app.window,nullptr,-32000,-32000,0,0,SWP_NOZORDER|SWP_NOACTIVATE|SWP_NOSIZE);
  int corner=2;DwmSetWindowAttribute(app.window,33,&corner,sizeof(corner));COLORREF border=RGB(43,43,47);DwmSetWindowAttribute(app.window,34,&border,sizeof(border));
  const wchar_t* labels[]={L"Завантажити",L"Обрати папку встановлення",L"Закрити",L"Умови використання — відкрити сайт",L"Я погоджуюся з умовами використання",L"Назад"};
  HWND* buttons[]={&app.primary,&app.folder,&app.close,&app.link,&app.consent,&app.back};
  for (int id=1;id<=6;++id) {
    *buttons[id-1]=CreateWindowExW(0,L"BUTTON",labels[id-1],WS_CHILD|WS_TABSTOP|BS_OWNERDRAW,0,0,0,0,app.window,reinterpret_cast<HMENU>(static_cast<INT_PTR>(id)),instance,nullptr);
    SetWindowSubclass(*buttons[id-1],button_proxy,id,0);
  }
  morph.from=morph.to=layout_for(app.phase);
  title_label.set(title_for(app.phase),false);detail_label.set(detail_for(app.phase,app.text),false);action_label.set(action_for(app.phase),false);
  layout();
  if(app.verify_ui){bool passed=verify_surface();DestroyWindow(app.window);canvas.reset();app.glyphs.clear();app.icon.reset();GdiplusShutdown(gdiplus);CoUninitialize();return passed?0:20;}
  app.phase_started=app.opened=GetTickCount64();render_surface(true);ShowWindow(app.window,app.verify_motion?SW_SHOWNOACTIVATE:SW_SHOWNORMAL);if(!app.verify_motion)SetFocus(app.primary);SetTimer(app.window,1,16,nullptr);if(app.verify_motion)SetTimer(app.window,3,10,nullptr);
  MSG message{};while (GetMessageW(&message,nullptr,0,0)>0) {
    if(message.message==WM_KEYDOWN&&(message.wParam==VK_TAB||message.wParam==VK_LEFT||message.wParam==VK_RIGHT)){app.keyboard_focus=true;InvalidateRect(app.window,nullptr,FALSE);}
    if (message.message==WM_KEYDOWN && message.wParam==VK_ESCAPE) { PostMessageW(app.window,WM_CLOSE,0,0);continue; }
    if (!IsDialogMessageW(app.window,&message)) { TranslateMessage(&message);DispatchMessageW(&message); }
  }
  app.cancelled=true;if (app.worker.joinable()) app.worker.join();canvas.reset();app.glyphs.clear();app.icon.reset();GdiplusShutdown(gdiplus);CoUninitialize();return motion_result;
}
