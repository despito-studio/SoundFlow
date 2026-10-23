#ifndef RUNNER_FLUTTER_WINDOW_H_
#define RUNNER_FLUTTER_WINDOW_H_

#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <flutter/method_channel.h>
#include <flutter/standard_method_codec.h>

#include <memory>

#include "win32_window.h"
#include "island_surface.h"

// A window that does nothing but host a Flutter view.
class FlutterWindow : public Win32Window {
 public:
  // Creates a new FlutterWindow hosting a Flutter view running |project|.
  explicit FlutterWindow(const flutter::DartProject& project, bool island = false);
  virtual ~FlutterWindow();

 protected:
  // Win32Window:
  bool OnCreate() override;
  void OnDestroy() override;
  LRESULT MessageHandler(HWND window, UINT const message, WPARAM const wparam,
                         LPARAM const lparam) noexcept override;

 private:
  // The project to run.
  flutter::DartProject project_;

  // The Flutter instance hosted by this window.
  std::unique_ptr<flutter::FlutterViewController> flutter_controller_;
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>> channel_;
  std::unique_ptr<FlutterWindow> island_;
  bool is_island_ = false;
  bool island_ready_ = false;
  bool island_requested_ = false;
  bool keyboard_ = false;
  bool close_to_tray_ = false;
  bool quitting_ = false;
  bool destroying_ = false;
  HWND previous_foreground_ = nullptr;
  HICON app_icon_ = nullptr;
  HMONITOR island_monitor_ = nullptr;
  HMONITOR island_desktop_monitor_ = nullptr;
  HWINEVENTHOOK foreground_hook_ = nullptr;
  HWINEVENTHOOK minimize_hook_ = nullptr;
  HWINEVENTHOOK shell_hook_ = nullptr;
  DWORD shell_process_ = 0;
  bool desktop_mode_ = true;
  bool desktop_visible_ = false;
  bool main_started_ = false;
  std::unique_ptr<IslandSurface> island_surface_;
  bool island_small_ = true;
  bool island_anchoring_ = false;
  double island_height_ = 0;
  std::unique_ptr<flutter::EncodableValue> island_state_;
  void HandleMethod(const flutter::MethodCall<flutter::EncodableValue>& call,
                    std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result);
  void PrepareIsland();
  void ShowIsland(bool keyboard);
  void HideIsland();
  void PlaceOnDesktop(bool notify = false);
  void CloakIsland(bool value);
  void BindDesktopEvents();
  static void CALLBACK ForegroundEvent(HWINEVENTHOOK hook, DWORD event, HWND window,
      LONG object, LONG child, DWORD thread, DWORD time);
  void ResizeIsland(double height);
  void AnchorIsland();
  void ShapeIsland(double height);
};

#endif  // RUNNER_FLUTTER_WINDOW_H_
