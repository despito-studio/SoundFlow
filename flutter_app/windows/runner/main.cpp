#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <windows.h>
#include <algorithm>

#include "flutter_window.h"
#include "utils.h"

int APIENTRY wWinMain(_In_ HINSTANCE instance, _In_opt_ HINSTANCE prev,
                      _In_ wchar_t *command_line, _In_ int show_command) {
  // Attach to console when present (e.g., 'flutter run') or create a
  // new console when running with a debugger.
  if (!::AttachConsole(ATTACH_PARENT_PROCESS) && ::IsDebuggerPresent()) {
    CreateAndAttachConsole();
  }

  // Initialize COM, so that it is available for use in the library and/or
  // plugins.
  ::CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);

  flutter::DartProject project(L"data");

  std::vector<std::string> command_line_arguments =
      GetCommandLineArguments();

  project.set_dart_entrypoint_arguments(std::move(command_line_arguments));

  FlutterWindow window(project);
  Win32Window::Point origin(70, 50);
  Win32Window::Size size(1280, 750);
  if (!window.Create(L"SoundFlow", origin, size)) {
    return EXIT_FAILURE;
  }
  window.SetQuitOnClose(true);
  MONITORINFO monitor{sizeof(MONITORINFO)};
  GetMonitorInfo(MonitorFromWindow(window.GetHandle(), MONITOR_DEFAULTTOPRIMARY), &monitor);
  RECT bounds{};
  GetWindowRect(window.GetHandle(), &bounds);
  const int width = std::min(static_cast<int>(bounds.right - bounds.left), static_cast<int>(monitor.rcWork.right - monitor.rcWork.left - 32));
  const int height = std::min(static_cast<int>(bounds.bottom - bounds.top), static_cast<int>(monitor.rcWork.bottom - monitor.rcWork.top - 32));
  SetWindowPos(window.GetHandle(), nullptr, monitor.rcWork.left + (monitor.rcWork.right - monitor.rcWork.left - width) / 2,
               monitor.rcWork.top + (monitor.rcWork.bottom - monitor.rcWork.top - height) / 2,
               width, height, SWP_NOZORDER | SWP_NOACTIVATE);

  ::MSG msg;
  while (::GetMessage(&msg, nullptr, 0, 0)) {
    ::TranslateMessage(&msg);
    ::DispatchMessage(&msg);
  }

  ::CoUninitialize();
  return EXIT_SUCCESS;
}
