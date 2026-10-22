#ifndef RUNNER_ISLAND_SURFACE_H_
#define RUNNER_ISLAND_SURFACE_H_

#include <windows.h>

// The Flutter HWND supplies the opaque interior. This layer supplies only its
// subpixel contour, so animation never reads the Flutter GPU surface back.
class IslandSurface {
 public:
  IslandSurface();
  ~IslandSurface();
  void Paint(HWND content, double progress, double scale, bool compact_screen);
  void Show(HWND content);
  void Hide();
  const unsigned char* pixels() const { return reinterpret_cast<const unsigned char*>(pixels_); }
  int width() const { return width_; }
  int height() const { return height_; }
  bool composed() const { return composed_; }
  HWND window() const { return window_; }
  void SetDesktop(bool value) { desktop_ = value; }
  void SetCloaked(bool value);

 private:
  HWND window_ = nullptr;
  HDC dc_ = nullptr;
  HBITMAP bitmap_ = nullptr;
  HGDIOBJ previous_bitmap_ = nullptr;
  unsigned int* pixels_ = nullptr;
  int width_ = 0;
  int height_ = 0;
  bool composed_ = false;
  bool desktop_ = true;
  bool Resize(int width, int height);
  static LRESULT CALLBACK WindowProc(HWND window, UINT message, WPARAM wparam, LPARAM lparam);
};

#endif
