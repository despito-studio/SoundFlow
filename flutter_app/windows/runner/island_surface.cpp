#include "island_surface.h"
#include <dwmapi.h>

#include <algorithm>
#include <cmath>
#include <cstring>
#include <vector>

IslandSurface::IslandSurface() {
  static const bool registered = [] {
    WNDCLASSW definition{};
    definition.hInstance = GetModuleHandleW(nullptr);
    definition.lpszClassName = L"SoundFlowIslandContour";
    definition.lpfnWndProc = WindowProc;
    return RegisterClassW(&definition) != 0;
  }();
  if (!registered) return;
  window_ = CreateWindowExW(
      WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE |
          WS_EX_TRANSPARENT | WS_EX_TOPMOST,
      L"SoundFlowIslandContour", L"SoundFlow Island contour", WS_POPUP,
      0, 0, 1, 1, nullptr, nullptr, GetModuleHandleW(nullptr), this);
  BOOL disabled = TRUE;
  DwmSetWindowAttribute(window_, DWMWA_TRANSITIONS_FORCEDISABLED, &disabled, sizeof(disabled));
  SetCloaked(true);
  dc_ = CreateCompatibleDC(nullptr);
}

LRESULT CALLBACK IslandSurface::WindowProc(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
  if (message == WM_NCCREATE) SetWindowLongPtr(window, GWLP_USERDATA,
      reinterpret_cast<LONG_PTR>(reinterpret_cast<CREATESTRUCT*>(lparam)->lpCreateParams));
  auto self = reinterpret_cast<IslandSurface*>(GetWindowLongPtr(window, GWLP_USERDATA));
  if (message == WM_MOUSEACTIVATE) return MA_NOACTIVATE;
  if (message == WM_NCHITTEST) return HTTRANSPARENT;
  if (self && self->desktop_) {
    if (message == WM_SYSCOMMAND && (wparam & 0xfff0) == SC_MINIMIZE) return 0;
    if (message == WM_WINDOWPOSCHANGING) reinterpret_cast<WINDOWPOS*>(lparam)->flags &= ~SWP_HIDEWINDOW;
  }
  return DefWindowProcW(window, message, wparam, lparam);
}

IslandSurface::~IslandSurface() {
  if (previous_bitmap_) SelectObject(dc_, previous_bitmap_);
  if (bitmap_) DeleteObject(bitmap_);
  if (dc_) DeleteDC(dc_);
  if (window_) DestroyWindow(window_);
}

bool IslandSurface::Resize(int width, int height) {
  if (width == width_ && height == height_) return pixels_ != nullptr;
  if (previous_bitmap_) SelectObject(dc_, previous_bitmap_);
  if (bitmap_) DeleteObject(bitmap_);
  pixels_ = nullptr;
  BITMAPINFO info{};
  info.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
  info.bmiHeader.biWidth = width;
  info.bmiHeader.biHeight = -height;
  info.bmiHeader.biPlanes = 1;
  info.bmiHeader.biBitCount = 32;
  info.bmiHeader.biCompression = BI_RGB;
  bitmap_ = CreateDIBSection(dc_, &info, DIB_RGB_COLORS,
                            reinterpret_cast<void**>(&pixels_), nullptr, 0);
  if (!bitmap_) return false;
  previous_bitmap_ = SelectObject(dc_, bitmap_);
  width_ = width;
  height_ = height;
  return true;
}

void IslandSurface::Paint(HWND content, double progress, double scale, bool compact_screen) {
  RECT bounds{};
  GetClientRect(content, &bounds);
  if (!window_ || !dc_ || !Resize(bounds.right, bounds.bottom)) return;
  std::memset(pixels_, 0, static_cast<size_t>(width_) * height_ * 4);
  const double t = std::clamp(progress, 0.0, 1.0);
  const double compact_width = compact_screen ? 136 : 144;
  const double compact_height = compact_screen ? 32 : 34;
  const double w = compact_width * scale + (width_ - compact_width * scale) * t;
  const double h = compact_height * scale + (height_ - compact_height * scale) * t;
  const double radius = (compact_height / 2 + (compact_screen ? 12 : 13) * t) * scale;
  const double exponent = 2 + .5 * t;
  const double left = (width_ - w) / 2;
  std::vector<RECT> rectangles;
  rectangles.reserve(height_);
  constexpr int samples = 8;
  for (int y = 0; y < std::min(height_, static_cast<int>(std::ceil(h))); ++y) {
    double edges[samples][2];
    double first = width_, last = 0, full_left = 0, full_right = width_;
    bool full_row = true;
    for (int s = 0; s < samples; ++s) {
      const double py = y + (s + .5) / samples;
      if (py >= h) { edges[s][0] = edges[s][1] = 0; full_row = false; continue; }
      const double dy = std::max({radius - py, py - (h - radius), 0.0});
      const double inset = dy > 0
          ? radius * (1 - std::pow(std::max(0.0, 1 - std::pow(dy / radius, exponent)), 1 / exponent))
          : 0;
      edges[s][0] = left + inset;
      edges[s][1] = left + w - inset;
      first = std::min(first, edges[s][0]);
      last = std::max(last, edges[s][1]);
      full_left = std::max(full_left, edges[s][0]);
      full_right = std::min(full_right, edges[s][1]);
    }
    int opaque_first = width_, opaque_last = -1;
    for (int x = std::max(0, static_cast<int>(std::floor(first)));
         x < std::min(width_, static_cast<int>(std::ceil(last))); ++x) {
      double coverage = 0;
      if (full_row && x >= full_left && x + 1 <= full_right) coverage = 1;
      else {
        for (const auto& edge : edges)
          coverage += std::clamp(std::min(x + 1.0, edge[1]) - std::max(static_cast<double>(x), edge[0]), 0.0, 1.0) / samples;
      }
      const auto alpha = static_cast<unsigned int>(std::lround(coverage * 255));
      pixels_[static_cast<size_t>(y) * width_ + x] = alpha << 24;
      if (alpha == 255) { opaque_first = std::min(opaque_first, x); opaque_last = x; }
    }
    // Keep the binary HWND clip strictly inside fully covered black pixels.
    // Its stair steps then meet black, while DWM composites the actual edge.
    if (y > 0 && y + 1 < h && opaque_last - opaque_first > 2) {
      RECT run{opaque_first + 1, y, opaque_last, y + 1};
      if (!rectangles.empty() && rectangles.back().left == run.left &&
          rectangles.back().right == run.right) rectangles.back().bottom = y + 1;
      else rectangles.push_back(run);
    }
  }
  std::vector<unsigned char> storage(sizeof(RGNDATAHEADER) + rectangles.size() * sizeof(RECT));
  auto region_data = reinterpret_cast<RGNDATA*>(storage.data());
  region_data->rdh = {sizeof(RGNDATAHEADER), RDH_RECTANGLES,
      static_cast<DWORD>(rectangles.size()),
      static_cast<DWORD>(rectangles.size() * sizeof(RECT)), bounds};
  std::memcpy(region_data->Buffer, rectangles.data(), rectangles.size() * sizeof(RECT));
  HRGN region = ExtCreateRegion(nullptr, static_cast<DWORD>(storage.size()), region_data);
  if (region && !SetWindowRgn(content, region, TRUE)) DeleteObject(region);
  RECT screen{};
  GetWindowRect(content, &screen);
  POINT destination{screen.left, screen.top}, source{};
  SIZE size{width_, height_};
  BLENDFUNCTION blend{AC_SRC_OVER, 0, 255, AC_SRC_ALPHA};
  composed_ = UpdateLayeredWindow(window_, nullptr, &destination, &size, dc_, &source, 0, &blend, ULW_ALPHA) != FALSE;
  if (IsWindowVisible(content)) Show(content);
}

void IslandSurface::Show(HWND content) {
  const bool topmost = (GetWindowLongPtr(content, GWL_EXSTYLE) & WS_EX_TOPMOST) != 0;
  const bool was_topmost = (GetWindowLongPtr(window_, GWL_EXSTYLE) & WS_EX_TOPMOST) != 0;
  if (topmost != was_topmost) SetWindowPos(window_, topmost ? HWND_TOPMOST : HWND_NOTOPMOST,
      0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
  SetWindowPos(window_, content, 0, 0, 0, 0,
      SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW);
}

void IslandSurface::SetCloaked(bool value) {
  BOOL cloak = value;
  DwmSetWindowAttribute(window_, DWMWA_CLOAK, &cloak, sizeof(cloak));
}

void IslandSurface::Hide() { SetCloaked(true); }
