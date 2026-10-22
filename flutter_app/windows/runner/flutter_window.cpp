#include "flutter_window.h"
#include <dwmapi.h>
#include <commdlg.h>
#include <flutter_windows.h>
#include "utils.h"
#include <algorithm>
#include <filesystem>
#include <optional>
#include <iostream>
#include "flutter/generated_plugin_registrant.h"

namespace {
FlutterWindow* main_window = nullptr;
using Value = flutter::EncodableValue;
void CloseAudio() {
  auto module = GetModuleHandleW(L"soundflow_lib.dll");
  if (module) {
    auto close = reinterpret_cast<void (*)()>(GetProcAddress(module, "sf_close"));
    if (close) close();
  }
}
bool Boolean(const Value* value) {
  return value && std::holds_alternative<bool>(*value) && std::get<bool>(*value);
}
bool DesktopWindow(HWND window) {
  if (!window) return false;
  wchar_t name[64]{};
  GetClassNameW(GetAncestor(window, GA_ROOT), name, 64);
  return wcscmp(name, L"Progman") == 0 || wcscmp(name, L"WorkerW") == 0;
}
bool DesktopWorkspace(HWND island, HWND contour) {
  const HWND foreground = GetAncestor(GetForegroundWindow(), GA_ROOT);
  if (DesktopWindow(foreground)) return true;
  struct Workspace {
    HWND foreground, island, contour;
    bool desktop = false;
    RECT monitor{};
  } workspace{foreground, island, contour};
  MONITORINFO monitor{sizeof(monitor)};
  GetMonitorInfoW(MonitorFromWindow(island, MONITOR_DEFAULTTOPRIMARY), &monitor);
  workspace.monitor = monitor.rcMonitor;
  // Show Desktop can raise the shell without changing the foreground HWND.
  // Inspect the visible workspace, ignoring our own composited surfaces.
  EnumWindows([](HWND window, LPARAM data) -> BOOL {
    auto& value = *reinterpret_cast<Workspace*>(data);
    if (window == value.island || window == value.contour ||
        !IsWindowVisible(window) || IsIconic(window)) return TRUE;
    DWORD cloak = 0;
    DwmGetWindowAttribute(window, DWMWA_CLOAKED, &cloak, sizeof(cloak));
    if (cloak != 0) return TRUE;
    RECT bounds{}, intersection{};
    GetWindowRect(window, &bounds);
    if (!IntersectRect(&intersection, &bounds, &value.monitor)) return TRUE;
    if (DesktopWindow(window)) { value.desktop = true; return FALSE; }
    wchar_t name[64]{};
    GetClassNameW(window, name, 64);
    if (wcscmp(name, L"Shell_TrayWnd") == 0 ||
        wcscmp(name, L"Shell_SecondaryTrayWnd") == 0) return TRUE;
    if (window == value.foreground) return FALSE;
    const LONG_PTR style = GetWindowLongPtr(window, GWL_STYLE);
    const LONG_PTR extended = GetWindowLongPtr(window, GWL_EXSTYLE);
    if (!(extended & WS_EX_TOOLWINDOW) && (style & (WS_CAPTION | WS_SYSMENU)) &&
        bounds.right - bounds.left > 160 && bounds.bottom - bounds.top > 80)
      return FALSE;
    return TRUE;
  }, reinterpret_cast<LPARAM>(&workspace));
  return workspace.desktop;
}
}

FlutterWindow::FlutterWindow(const flutter::DartProject& project, bool island)
    : project_(project), is_island_(island) {
  if (!island) main_window = this;
}
FlutterWindow::~FlutterWindow() { if (main_window == this) main_window = nullptr; }

bool FlutterWindow::OnCreate() {
  if (!Win32Window::OnCreate()) return false;
  if (!is_island_) {
    foreground_hook_ = SetWinEventHook(EVENT_SYSTEM_FOREGROUND,
        EVENT_SYSTEM_FOREGROUND, nullptr, ForegroundEvent, 0, 0, WINEVENT_OUTOFCONTEXT);
    minimize_hook_ = SetWinEventHook(EVENT_SYSTEM_MINIMIZESTART,
        EVENT_SYSTEM_MINIMIZEEND, nullptr, ForegroundEvent, 0, 0, WINEVENT_OUTOFCONTEXT);
    BindDesktopEvents();
  }
  if (is_island_) {
    BOOL disabled = TRUE;
    DwmSetWindowAttribute(GetHandle(), DWMWA_TRANSITIONS_FORCEDISABLED, &disabled, sizeof(disabled));
    BOOL cloak = TRUE;
    DwmSetWindowAttribute(GetHandle(), DWMWA_CLOAK, &cloak, sizeof(cloak));
    SetWindowLongPtr(GetHandle(), GWL_STYLE, WS_POPUP);
    SetWindowLongPtr(GetHandle(), GWL_EXSTYLE, WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE);
    SetWindowPos(GetHandle(), nullptr, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
    island_surface_ = std::make_unique<IslandSurface>();
    ResizeIsland(0);
  }
  RECT frame = GetClientArea();
  flutter_controller_ = std::make_unique<flutter::FlutterViewController>(
      frame.right - frame.left, frame.bottom - frame.top, project_);
  if (!flutter_controller_->engine() || !flutter_controller_->view()) return false;
  RegisterPlugins(flutter_controller_->engine());
  SetChildContent(flutter_controller_->view()->GetNativeWindow());
  channel_ = std::make_unique<flutter::MethodChannel<Value>>(
      flutter_controller_->engine()->messenger(), "soundflow/window",
      &flutter::StandardMethodCodec::GetInstance());
  channel_->SetMethodCallHandler([this](const auto& call, auto result) {
    HandleMethod(call, std::move(result));
  });
  flutter_controller_->engine()->SetNextFrameCallback([this]() {
    if (is_island_) {
      island_ready_ = true;
      if (main_window && main_window->island_requested_) main_window->ShowIsland(keyboard_);
      else if (main_window) main_window->HideIsland();
    } else {
      main_started_ = true;
      Show();
      if (island_) island_->PlaceOnDesktop(true);
    }
  });
  flutter_controller_->ForceRedraw();
  return true;
}

void FlutterWindow::HandleMethod(
    const flutter::MethodCall<Value>& call,
    std::unique_ptr<flutter::MethodResult<Value>> result) {
  const auto& name = call.method_name();
  const auto args = call.arguments();
  if (name == "show") {
    ShowWindow(GetHandle(), SW_RESTORE);
    SetForegroundWindow(GetHandle());
  } else if (name == "quit") {
    auto owner = main_window ? main_window : this;
    owner->quitting_ = true;
    PostMessage(owner->GetHandle(), WM_CLOSE, 0, 0);
  } else if (name == "close-to-tray") {
    close_to_tray_ = Boolean(args);
  } else if (name == "theme") {
    BOOL dark = Boolean(args);
    DwmSetWindowAttribute(GetHandle(), 20, &dark, sizeof(dark));
    COLORREF caption = dark ? RGB(24,24,24) : RGB(245,245,247);
    DwmSetWindowAttribute(GetHandle(), 35, &caption, sizeof(caption));
  } else if (name == "pick-image") {
    wchar_t path[32768]{};
    OPENFILENAMEW dialog{sizeof(dialog)};
    dialog.hwndOwner = GetHandle();
    dialog.lpstrFilter = L"Зображення\0*.png;*.jpg;*.jpeg;*.webp;*.gif;*.bmp\0\0";
    dialog.lpstrFile = path;
    dialog.nMaxFile = 32768;
    dialog.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
    if (GetOpenFileNameW(&dialog)) result->Success(Value(Utf8FromUtf16(path)));
    else result->Success(Value());
    return;
  } else if (name == "show-main") {
    if (main_window) { ShowWindow(main_window->GetHandle(), SW_RESTORE); SetForegroundWindow(main_window->GetHandle()); }
  } else if (name == "island-shortcuts" && !is_island_) {
    for (int id : {3101,3102,3103}) UnregisterHotKey(GetHandle(), id);
    flutter::EncodableMap registered;
    if (Boolean(args)) {
      registered[Value("quick")] = Value(RegisterHotKey(GetHandle(), 3101, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 'M') != 0);
      registered[Value("keyboard")] = Value(RegisterHotKey(GetHandle(), 3102, MOD_CONTROL | MOD_ALT | MOD_SHIFT | MOD_NOREPEAT, 'M') != 0);
      registered[Value("fallback")] = Value(RegisterHotKey(GetHandle(), 3103, MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT, VK_SPACE) != 0);
    }
    result->Success(Value(registered));
    return;
  } else if (name == "app-icon" && args && std::holds_alternative<std::string>(*args)) {
    const auto icon_name = std::get<std::string>(*args);
    if (icon_name == "default" || icon_name == "rose" || icon_name == "violet" || icon_name == "ocean") {
      wchar_t executable[32768]{};
      if (GetModuleFileNameW(nullptr, executable, 32768)) {
        auto filename = icon_name == "default" ? L"icon.ico" : L"icon-" + std::wstring(icon_name.begin(), icon_name.end()) + L".ico";
        auto path = std::filesystem::path(executable).parent_path() / L"resources" / filename;
        auto icon = static_cast<HICON>(LoadImageW(nullptr, path.c_str(), IMAGE_ICON, 0, 0, LR_LOADFROMFILE | LR_DEFAULTSIZE));
        if (icon) {
          SendMessageW(GetHandle(), WM_SETICON, ICON_BIG, reinterpret_cast<LPARAM>(icon));
          SendMessageW(GetHandle(), WM_SETICON, ICON_SMALL, reinterpret_cast<LPARAM>(icon));
          if (app_icon_) DestroyIcon(app_icon_);
          app_icon_ = icon;
        }
      }
    }
  } else if (name == "prepare-island") {
    PrepareIsland();
  } else if (name == "toggle-island") {
    if (island_ && island_requested_) {
      if (island_->island_ready_) island_->channel_->InvokeMethod("dismiss", std::make_unique<Value>());
      else HideIsland();
    }
    else ShowIsland(Boolean(args));
  } else if (name == "show-island") {
    ShowIsland(Boolean(args));
  } else if (name == "hide-island") {
    if (main_window) main_window->HideIsland();
  } else if (name == "island-state") {
    island_state_ = std::make_unique<Value>(*args);
    if (island_ && island_->channel_ && IsWindowVisible(island_->GetHandle()))
      island_->channel_->InvokeMethod("state", std::make_unique<Value>(*args));
  } else if (name == "island-command") {
    if (main_window && main_window->channel_) main_window->channel_->InvokeMethod("core-command", std::make_unique<Value>(*args));
  } else if (name == "island-shape" && is_island_) {
    if (args) {
      if (auto value = std::get_if<double>(args)) ShapeIsland(*value);
    }
  } else if (name == "island-snapshot" && is_island_ && island_surface_) {
    const auto& surface = *island_surface_;
    RECT position{};
    GetWindowRect(GetHandle(), &position);
    flutter::EncodableMap frame;
    frame[Value("width")] = Value(surface.width());
    frame[Value("height")] = Value(surface.height());
    frame[Value("composed")] = Value(surface.composed());
    frame[Value("desktop")] = Value(desktop_mode_);
    DWORD cloaked = 0;
    DwmGetWindowAttribute(GetHandle(), DWMWA_CLOAKED, &cloaked, sizeof(cloaked));
    frame[Value("cloaked")] = Value(static_cast<int>(cloaked));
    frame[Value("desktopForeground")] = Value(DesktopWindow(GetForegroundWindow()));
    frame[Value("visible")] = Value(IsWindowVisible(GetHandle()) != FALSE);
    frame[Value("topmost")] = Value((GetWindowLongPtr(GetHandle(), GWL_EXSTYLE) & WS_EX_TOPMOST) != 0);
    frame[Value("foregroundHook")] = Value(main_window && main_window->foreground_hook_ != nullptr);
    bool below_foreground = false;
    HWND scan = GetAncestor(GetForegroundWindow(), GA_ROOT);
    for (int i = 0; scan && i < 512; ++i, scan = GetWindow(scan, GW_HWNDNEXT))
      if (scan == GetHandle()) { below_foreground = true; break; }
    frame[Value("belowForeground")] = Value(below_foreground);
    frame[Value("x")] = Value(static_cast<int>(position.left));
    frame[Value("y")] = Value(static_cast<int>(position.top));
    if (surface.pixels()) frame[Value("bgra")] = Value(std::vector<uint8_t>(
        surface.pixels(), surface.pixels() + surface.width() * surface.height() * 4));
    result->Success(Value(frame));
    return;
  } else if (name == "drag" && !is_island_) {
    ReleaseCapture();
    SendMessage(GetHandle(), WM_NCLBUTTONDOWN, HTCAPTION, 0);
  } else {
    result->NotImplemented();
    return;
  }
  result->Success();
}

void FlutterWindow::ResizeIsland(double logical_height) {
  AnchorIsland();
  ShapeIsland(logical_height);
}

void FlutterWindow::AnchorIsland() {
  if (island_anchoring_) return;
  island_anchoring_ = true;
  MONITORINFO monitor{sizeof(MONITORINFO)};
  if (!island_monitor_ || !GetMonitorInfo(island_monitor_, &monitor)) {
    island_monitor_ = MonitorFromWindow(GetHandle(), MONITOR_DEFAULTTOPRIMARY);
    GetMonitorInfo(island_monitor_, &monitor);
  }
  const double scale = FlutterDesktopGetDpiForMonitor(island_monitor_) / 96.0;
  island_small_ = (monitor.rcWork.bottom - monitor.rcWork.top) / scale <= 820 ||
                  (monitor.rcWork.right - monitor.rcWork.left) / scale <= 1440;
  const int width = std::min(static_cast<int>((island_small_ ? 288 : 312) * scale),
                            static_cast<int>(monitor.rcWork.right - monitor.rcWork.left - 24));
  const int height = static_cast<int>((island_small_ ? 144 : 156) * scale);
  const int x = monitor.rcMonitor.left + (monitor.rcMonitor.right - monitor.rcMonitor.left - width) / 2;
  const int y = monitor.rcMonitor.top + static_cast<int>(7 * scale);
  SetWindowPos(GetHandle(), HWND_TOPMOST, x, y, width, height, SWP_NOACTIVATE);
  island_anchoring_ = false;
  ShapeIsland(island_height_);
  if (desktop_mode_) PlaceOnDesktop();
}

void FlutterWindow::ShapeIsland(double logical_height) {
  island_height_ = std::clamp(logical_height, 0.0, 1.0);
  const double scale = static_cast<double>(GetDpiForWindow(GetHandle())) / 96.0;
  if (island_surface_) {
    island_surface_->SetDesktop(desktop_mode_);
    island_surface_->Paint(GetHandle(), island_height_, scale, island_small_);
  }
}

void FlutterWindow::PrepareIsland() {
  if (!island_) {
    flutter::DartProject project(L"data");
    std::vector<std::string> arguments{"--island"};
    const auto& parent_arguments = project_.dart_entrypoint_arguments();
    auto export_flag = std::find(parent_arguments.begin(), parent_arguments.end(), "--export-island");
    if (export_flag != parent_arguments.end() && export_flag + 1 != parent_arguments.end()) {
      arguments.push_back("--export-island");
      arguments.push_back(*(export_flag + 1));
    }
    project.set_dart_entrypoint_arguments(arguments);
    island_ = std::make_unique<FlutterWindow>(project, true);
    if (!island_->Create(L"SoundFlow Island", {0, 0}, {288, 144})) {
      island_.reset();
      return;
    }
    island_->SetQuitOnClose(false);
    island_->island_desktop_monitor_ = MonitorFromWindow(GetHandle(), MONITOR_DEFAULTTOPRIMARY);
    island_->island_monitor_ = island_->island_desktop_monitor_;
    island_->AnchorIsland();
    ShowWindow(island_->GetHandle(), SW_SHOWNOACTIVATE);
    island_->PlaceOnDesktop();
  }
}

void FlutterWindow::ShowIsland(bool keyboard) {
  HWND foreground = GetForegroundWindow();
  if (!island_ || foreground != island_->GetHandle()) previous_foreground_ = foreground;
  island_requested_ = true;
  PrepareIsland();
  if (!island_) return;
  island_->desktop_mode_ = false;
  island_->keyboard_ = keyboard;
  auto style = GetWindowLongPtr(island_->GetHandle(), GWL_EXSTYLE);
  SetWindowLongPtr(island_->GetHandle(), GWL_EXSTYLE,
                   keyboard ? style & ~WS_EX_NOACTIVATE : style | WS_EX_NOACTIVATE);
  island_->island_monitor_ = MonitorFromWindow(previous_foreground_, MONITOR_DEFAULTTOPRIMARY);
  if (IsWindowVisible(island_->GetHandle())) island_->AnchorIsland();
  else island_->ResizeIsland(0);
  if (!island_->island_ready_) {
    ShowWindow(island_->GetHandle(), SW_SHOWNOACTIVATE);
    island_->ShapeIsland(0);
    return;
  }
  if (island_state_ && island_->channel_)
    island_->channel_->InvokeMethod("state", std::make_unique<Value>(*island_state_));
  ShowWindow(island_->GetHandle(), keyboard ? SW_SHOW : SW_SHOWNOACTIVATE);
  island_->ShapeIsland(island_->island_height_);
  island_->CloakIsland(false);
  if (keyboard) {
    SetForegroundWindow(island_->GetHandle());
    SetFocus(island_->flutter_controller_->view()->GetNativeWindow());
  }
  if (island_->channel_) island_->channel_->InvokeMethod("shown", std::make_unique<Value>(keyboard));
}

void FlutterWindow::HideIsland() {
  island_requested_ = false;
  if (!island_) return;
  const bool restore = GetForegroundWindow() == island_->GetHandle();
  island_->keyboard_ = false;
  island_->desktop_mode_ = true;
  SetWindowLongPtr(island_->GetHandle(), GWL_EXSTYLE,
      GetWindowLongPtr(island_->GetHandle(), GWL_EXSTYLE) | WS_EX_NOACTIVATE);
  island_->island_monitor_ = island_->island_desktop_monitor_;
  if (island_state_ && island_->channel_)
    island_->channel_->InvokeMethod("state", std::make_unique<Value>(*island_state_));
  island_->island_height_ = 0;
  if (restore && previous_foreground_ && IsWindow(previous_foreground_)) SetForegroundWindow(previous_foreground_);
  island_->AnchorIsland();
  island_->PlaceOnDesktop(true);
}

void FlutterWindow::CloakIsland(bool value) {
  BOOL cloak = value;
  if (!value && island_surface_) island_surface_->SetCloaked(false);
  DwmSetWindowAttribute(GetHandle(), DWMWA_CLOAK, &cloak, sizeof(cloak));
  if (value && island_surface_) island_surface_->SetCloaked(true);
}

void FlutterWindow::PlaceOnDesktop(bool notify) {
  if (!desktop_mode_ || !IsWindow(GetHandle())) return;
  const bool show = DesktopWorkspace(GetHandle(), island_surface_ ? island_surface_->window() : nullptr);
  const bool changed = show != desktop_visible_;
  if (notify || show != desktop_visible_) {
    desktop_visible_ = show;
    island_height_ = 0;
    if (channel_) channel_->InvokeMethod(show ? "desktop" : "hidden", std::make_unique<Value>());
    ShapeIsland(0);
  }
  // The resident engine and surface stay ready. DWM reveals them only over an
  // active desktop; opening any application cloaks both without an entry effect.
  if (show) SetWindowPos(GetHandle(), HWND_TOPMOST, 0, 0, 0, 0,
      SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
  CloakIsland(!show);
  if ((notify || changed) && main_window) {
    const auto& arguments = main_window->project_.dart_entrypoint_arguments();
    if (std::find(arguments.begin(), arguments.end(), "--trace-island") != arguments.end()) {
      wchar_t foreground_class[128]{};
      GetClassNameW(GetForegroundWindow(), foreground_class, 128);
      DWORD content_cloak = 0, contour_cloak = 0;
      DwmGetWindowAttribute(GetHandle(), DWMWA_CLOAKED, &content_cloak, sizeof(content_cloak));
      if (island_surface_) DwmGetWindowAttribute(island_surface_->window(), DWMWA_CLOAKED,
          &contour_cloak, sizeof(contour_cloak));
      std::cout << "Island desktop=" << show << " foreground=" << Utf8FromUtf16(foreground_class)
          << " content-cloak=" << content_cloak << " contour-cloak=" << contour_cloak
          << " ready=" << island_ready_ << std::endl;
    }
  }
}

void FlutterWindow::BindDesktopEvents() {
  DWORD process = 0;
  GetWindowThreadProcessId(GetShellWindow(), &process);
  if (process == shell_process_ && shell_hook_) return;
  if (shell_hook_) UnhookWinEvent(shell_hook_);
  shell_process_ = process;
  shell_hook_ = process ? SetWinEventHook(EVENT_OBJECT_SHOW, EVENT_OBJECT_REORDER,
      nullptr, ForegroundEvent, process, 0, WINEVENT_OUTOFCONTEXT) : nullptr;
}

void CALLBACK FlutterWindow::ForegroundEvent(HWINEVENTHOOK, DWORD event, HWND window,
    LONG, LONG, DWORD, DWORD) {
  if (!main_window || !main_window->island_) return;
  main_window->BindDesktopEvents();
  auto island = main_window->island_.get();
  const HWND foreground = GetAncestor(GetForegroundWindow(), GA_ROOT);
  if (event == EVENT_SYSTEM_FOREGROUND && GetAncestor(window, GA_ROOT) != foreground) return;
  if (foreground == island->GetHandle()) return;
  if (main_window->island_requested_ && event == EVENT_SYSTEM_FOREGROUND) main_window->HideIsland();
  else island->PlaceOnDesktop();
}

void FlutterWindow::OnDestroy() {
  if (!is_island_) {
    if (foreground_hook_) { UnhookWinEvent(foreground_hook_); foreground_hook_ = nullptr; }
    if (minimize_hook_) { UnhookWinEvent(minimize_hook_); minimize_hook_ = nullptr; }
    if (shell_hook_) { UnhookWinEvent(shell_hook_); shell_hook_ = nullptr; }
    for (int id : {3101,3102,3103}) UnregisterHotKey(GetHandle(), id);
    island_.reset();
    CloseAudio();
  }
  island_surface_.reset();
  channel_.reset();
  flutter_controller_.reset();
  if (app_icon_) { DestroyIcon(app_icon_); app_icon_ = nullptr; }
  Win32Window::OnDestroy();
}

LRESULT FlutterWindow::MessageHandler(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam) noexcept {
  if (!is_island_ && message == WM_HOTKEY && (wparam == 3101 || wparam == 3102 || wparam == 3103)) {
    if (wparam == 3102) ShowIsland(true);
    else if (island_ && island_requested_) {
      if (island_->island_ready_) island_->channel_->InvokeMethod("dismiss", std::make_unique<Value>());
      else HideIsland();
    }
    else ShowIsland(false);
    return 0;
  }
  if (is_island_ && (message == WM_DISPLAYCHANGE || message == WM_DPICHANGED)) {
    AnchorIsland();
    ShapeIsland(island_height_);
    return 0;
  }
  if (is_island_ && message == WM_SYSCOMMAND && (wparam & 0xfff0) == SC_MOVE) return 0;
  if (is_island_ && desktop_mode_ && message == WM_SYSCOMMAND &&
      (wparam & 0xfff0) == SC_MINIMIZE) return 0;
  if (is_island_ && desktop_mode_ && message == WM_WINDOWPOSCHANGING)
    reinterpret_cast<WINDOWPOS*>(lparam)->flags &= ~SWP_HIDEWINDOW;
  if (is_island_ && message == WM_CLOSE) {
    if (main_window) main_window->HideIsland();
    return 0;
  }
  if (message == WM_CLOSE && !is_island_ && close_to_tray_ && !quitting_) {
    ShowWindow(hwnd, SW_HIDE);
    return 0;
  }
  if (message == WM_MOUSEACTIVATE && is_island_ && !keyboard_) return MA_NOACTIVATE;
  if (message == WM_GETMINMAXINFO && !is_island_) {
    auto info = reinterpret_cast<MINMAXINFO*>(lparam);
    const double scale = static_cast<double>(GetDpiForWindow(hwnd)) / 96.0;
    info->ptMinTrackSize = {static_cast<LONG>(780 * scale), static_cast<LONG>(560 * scale)};
    return 0;
  }
  if (flutter_controller_) {
    auto result = flutter_controller_->HandleTopLevelWindowProc(hwnd, message, wparam, lparam);
    if (result) return *result;
  }
  if (message == WM_FONTCHANGE && flutter_controller_) flutter_controller_->engine()->ReloadSystemFonts();
  return Win32Window::MessageHandler(hwnd, message, wparam, lparam);
}
