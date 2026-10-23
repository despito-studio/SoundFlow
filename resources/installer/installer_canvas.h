#pragma once
#include <d2d1.h>
#include <dwrite_3.h>
#include <wrl/client.h>
#include <tuple>
#include "installer_vectors.h"

// One physical-pixel backbuffer for the entire window, including native-button visuals.
// Text uses an app-private Onest collection; nothing is installed into Windows Fonts.
class InstallerCanvas {
  template<class T> using Ptr = Microsoft::WRL::ComPtr<T>;
  struct PrivateFonts {
    std::vector<fs::path> paths;
    ~PrivateFonts(){for(const auto& file:paths){std::error_code error;fs::remove(file,error);}}
  } files_;
  Ptr<ID2D1Factory> factory_;
  Ptr<IDWriteFactory3> write_;
  Ptr<IDWriteFontCollection1> fonts_;
  Ptr<ID2D1DCRenderTarget> target_;
  Ptr<ID2D1SolidColorBrush> brush_;
  std::map<std::tuple<float,bool,bool>,Ptr<IDWriteTextFormat>> formats_;
  std::map<uint64_t,Ptr<ID2D1Bitmap>> bitmaps_;
  std::map<const sf_vectors::Stroke*,Ptr<ID2D1PathGeometry>> vectors_;
  std::map<int,Ptr<ID2D1StrokeStyle>> strokes_;
  float opacity_=1;
  std::wstring bold_family_=L"Onest";
  static void check(HRESULT result) { if (FAILED(result)) {std::ostringstream error;error<<"Не вдалося підготувати інтерфейс (0x"<<std::hex<<static_cast<uint32_t>(result)<<")";throw std::runtime_error(error.str());} }
  static D2D1_COLOR_F color(Color value) { return D2D1::ColorF(value.GetR()/255.f,value.GetG()/255.f,value.GetB()/255.f,value.GetA()/255.f); }
  static D2D1_RECT_F rect(RectF value) { return D2D1::RectF(value.X,value.Y,value.GetRight(),value.GetBottom()); }
  ID2D1Brush* brush(Color value) { auto ink=color(value);ink.a*=opacity_;brush_->SetColor(ink);return brush_.Get(); }
  fs::path font_resource(HINSTANCE instance,int id) {
    auto resource=FindResourceW(instance,MAKEINTRESOURCEW(id),RT_RCDATA);
    if (!resource) throw std::runtime_error("В інсталяторі немає шрифту Onest");
    auto path=fs::temp_directory_path()/(L"SoundFlow-Onest-"+unique()+L".ttf");
    files_.paths.push_back(path);
    std::ofstream stream(path,std::ios::binary);
    stream.write(static_cast<const char*>(LockResource(LoadResource(instance,resource))),SizeofResource(instance,resource));
    stream.close();if (!stream) throw std::runtime_error("Не вдалося підготувати Onest");
    return path;
  }
 public:
  explicit InstallerCanvas(HINSTANCE instance) {
    check(D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED,factory_.GetAddressOf()));
    check(DWriteCreateFactory(DWRITE_FACTORY_TYPE_ISOLATED,__uuidof(IDWriteFactory3),reinterpret_cast<IUnknown**>(write_.GetAddressOf())));
    Ptr<IDWriteFontSetBuilder> builder;check(write_->CreateFontSetBuilder(&builder));
    for (int id:{504,505}) {
      auto path=font_resource(instance,id);Ptr<IDWriteFontFaceReference> face;
      check(write_->CreateFontFaceReference(path.c_str(),nullptr,0,DWRITE_FONT_SIMULATIONS_NONE,&face));
      check(builder->AddFontFaceReference(face.Get()));
    }
    Ptr<IDWriteFontSet> set;check(builder->CreateFontSet(&set));check(write_->CreateFontCollectionFromFontSet(set.Get(),&fonts_));
    UINT32 index=0;BOOL found=FALSE;check(fonts_->FindFamilyName(L"Onest",&index,&found));
    if(!found)throw std::runtime_error("Не вдалося завантажити Onest");
    Ptr<IDWriteFontFamily> family;check(fonts_->GetFontFamily(index,&family));Ptr<IDWriteFont> bold;
    check(family->GetFirstMatchingFont(DWRITE_FONT_WEIGHT_SEMI_BOLD,DWRITE_FONT_STRETCH_NORMAL,DWRITE_FONT_STYLE_NORMAL,&bold));
    if(bold->GetWeight()!=DWRITE_FONT_WEIGHT_SEMI_BOLD||bold->GetSimulations()!=DWRITE_FONT_SIMULATIONS_NONE) {
      check(fonts_->FindFamilyName(L"Onest SemiBold",&index,&found));
      if(!found)throw std::runtime_error("Не вдалося завантажити Onest SemiBold");bold_family_=L"Onest SemiBold";
    }
    auto properties=D2D1::RenderTargetProperties(D2D1_RENDER_TARGET_TYPE_DEFAULT,D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM,D2D1_ALPHA_MODE_IGNORE),96,96);
    check(factory_->CreateDCRenderTarget(&properties,&target_));
    check(target_->CreateSolidColorBrush(D2D1::ColorF(D2D1::ColorF::White),&brush_));
  }
  ~InstallerCanvas() {
    vectors_.clear();strokes_.clear();bitmaps_.clear();formats_.clear();fonts_.Reset();write_.Reset();brush_.Reset();target_.Reset();factory_.Reset();
  }
  void begin(HDC dc,int width,int height,float scale,float x=0,float y=0) {
    RECT bounds{0,0,width,height};check(target_->BindDC(dc,&bounds));
    target_->BeginDraw();target_->SetDpi(96,96);target_->SetTransform(D2D1::Matrix3x2F::Scale(scale,scale)*D2D1::Matrix3x2F::Translation(x*scale,y*scale));opacity_=1;
    target_->SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
    target_->SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE);
  }
  void end() { check(target_->EndDraw()); }
  void opacity(float value){opacity_=std::clamp(value,0.f,1.f);}
  void clear(Color value) { target_->Clear(color(value)); }
  void rounded(RectF value,float radius,Color fill,float stroke=0,Color outline=Color(0,0,0,0)) {
    D2D1_ROUNDED_RECT shape{rect(value),radius,radius};
    if (fill.GetA()) target_->FillRoundedRectangle(shape,brush(fill));
    if (stroke>0) target_->DrawRoundedRectangle(shape,brush(outline),stroke);
  }
  void text(const std::wstring& value,RectF bounds,float size,Color ink,bool bold,bool center) {
    const auto key=std::make_tuple(size,bold,center);auto& format=formats_[key];
    if (!format) {
      check(write_->CreateTextFormat(bold?bold_family_.c_str():L"Onest",fonts_.Get(),bold?DWRITE_FONT_WEIGHT_SEMI_BOLD:DWRITE_FONT_WEIGHT_NORMAL,DWRITE_FONT_STYLE_NORMAL,DWRITE_FONT_STRETCH_NORMAL,size,L"uk-UA",&format));
      format->SetTextAlignment(center?DWRITE_TEXT_ALIGNMENT_CENTER:DWRITE_TEXT_ALIGNMENT_LEADING);
      format->SetParagraphAlignment(center?DWRITE_PARAGRAPH_ALIGNMENT_CENTER:DWRITE_PARAGRAPH_ALIGNMENT_NEAR);
    }
    target_->DrawTextW(value.data(),static_cast<UINT32>(value.size()),format.Get(),rect(bounds),brush(ink),D2D1_DRAW_TEXT_OPTIONS_CLIP,DWRITE_MEASURING_MODE_NATURAL);
  }
  void image(int id,Bitmap* source,RectF bounds,Color tint=Color(0,0,0,0)) {
    uint64_t key=(static_cast<uint64_t>(id)<<32)|tint.GetValue();auto& bitmap=bitmaps_[key];
    if (!bitmap) {
      auto width=source->GetWidth(),height=source->GetHeight();Rect lock(0,0,width,height);BitmapData data{};
      if (source->LockBits(&lock,ImageLockModeRead,PixelFormat32bppPARGB,&data)!=Ok) throw std::runtime_error("Не вдалося завантажити іконку");
      std::vector<uint32_t> pixels(static_cast<size_t>(width)*height);
      for (UINT y=0;y<height;++y) {
        auto row=reinterpret_cast<const uint32_t*>(static_cast<const BYTE*>(data.Scan0)+static_cast<ptrdiff_t>(y)*data.Stride);
        for (UINT x=0;x<width;++x) {
          uint32_t pixel=row[x];
          if (tint.GetA()) {uint32_t a=(pixel>>24)*tint.GetA()/255;pixel=(a<<24)|(tint.GetR()*a/255<<16)|(tint.GetG()*a/255<<8)|(tint.GetB()*a/255);}
          pixels[static_cast<size_t>(y)*width+x]=pixel;
        }
      }
      source->UnlockBits(&data);
      check(target_->CreateBitmap(D2D1::SizeU(width,height),pixels.data(),width*4,D2D1::BitmapProperties(D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM,D2D1_ALPHA_MODE_PREMULTIPLIED)),&bitmap));
    }
    target_->DrawBitmap(bitmap.Get(),rect(bounds),opacity_,D2D1_BITMAP_INTERPOLATION_MODE_LINEAR);
  }
  void vector(const sf_vectors::Frame& frame,RectF bounds,Color ink) {
    D2D1_MATRIX_3X2_F previous;target_->GetTransform(&previous);
    target_->SetTransform(D2D1::Matrix3x2F::Scale(bounds.Width/24,bounds.Height/24)*D2D1::Matrix3x2F::Translation(bounds.X,bounds.Y)*previous);
    for(unsigned i=0;i<frame.count;++i) {
      const auto& part=frame.strokes[i];if(part.count<2||part.opacity<=0)continue;
      auto& path=vectors_[&part];
      if(!path) {
        check(factory_->CreatePathGeometry(&path));Ptr<ID2D1GeometrySink> sink;check(path->Open(&sink));
        sink->BeginFigure(D2D1::Point2F(part.points[0].x,part.points[0].y),part.filled?D2D1_FIGURE_BEGIN_FILLED:D2D1_FIGURE_BEGIN_HOLLOW);
        for(unsigned p=1;p<part.count;++p)sink->AddLine(D2D1::Point2F(part.points[p].x,part.points[p].y));
        sink->EndFigure(part.closed?D2D1_FIGURE_END_CLOSED:D2D1_FIGURE_END_OPEN);check(sink->Close());
      }
      auto& style=strokes_[part.cap*3+part.join];
      if(!style) {
        auto cap=part.cap==1?D2D1_CAP_STYLE_ROUND:part.cap==2?D2D1_CAP_STYLE_SQUARE:D2D1_CAP_STYLE_FLAT;
        auto join=part.join==1?D2D1_LINE_JOIN_ROUND:part.join==2?D2D1_LINE_JOIN_BEVEL:D2D1_LINE_JOIN_MITER;
        check(factory_->CreateStrokeStyle(D2D1::StrokeStyleProperties(cap,cap,cap,join),nullptr,0,&style));
      }
      Color tint(static_cast<BYTE>(ink.GetA()*part.opacity),ink.GetR(),ink.GetG(),ink.GetB());
      if(part.filled)target_->FillGeometry(path.Get(),brush(tint));else target_->DrawGeometry(path.Get(),brush(tint),part.width,style.Get());
    }
    target_->SetTransform(previous);
  }
  void line(float x1,float y1,float x2,float y2,Color ink,float width) {target_->DrawLine(D2D1::Point2F(x1,y1),D2D1::Point2F(x2,y2),brush(ink),width);}
  void ellipse(float x,float y,float width,float height,Color ink,float stroke) {target_->DrawEllipse(D2D1::Ellipse(D2D1::Point2F(x+width/2,y+height/2),width/2,height/2),brush(ink),stroke);}
  void arc(float x,float y,float width,float height,float start,float sweep,Color ink,float stroke) {
    if (sweep<=0) return;if (sweep>=359.9f) {ellipse(x,y,width,height,ink,stroke);return;}
    constexpr float pi=3.14159265358979323846f;
    auto point=[&](float angle){float r=angle*pi/180;return D2D1::Point2F(x+width/2+std::cos(r)*width/2,y+height/2+std::sin(r)*height/2);};
    Ptr<ID2D1PathGeometry> path;Ptr<ID2D1GeometrySink> sink;check(factory_->CreatePathGeometry(&path));check(path->Open(&sink));
    sink->BeginFigure(point(start),D2D1_FIGURE_BEGIN_HOLLOW);
    sink->AddArc(D2D1::ArcSegment(point(start+sweep),D2D1::SizeF(width/2,height/2),0,D2D1_SWEEP_DIRECTION_CLOCKWISE,sweep>180?D2D1_ARC_SIZE_LARGE:D2D1_ARC_SIZE_SMALL));
    sink->EndFigure(D2D1_FIGURE_END_OPEN);check(sink->Close());target_->DrawGeometry(path.Get(),brush(ink),stroke);
  }
};

class InstallerSurface {
  HDC dc_=CreateCompatibleDC(nullptr);HBITMAP bitmap_{};HGDIOBJ old_{};uint32_t* pixels_{};int width_=0,height_=0;
 public:
  ~InstallerSurface(){if(old_)SelectObject(dc_,old_);if(bitmap_)DeleteObject(bitmap_);if(dc_)DeleteDC(dc_);}
  void resize(int width,int height) {
    if(width==width_&&height==height_)return;
    if(old_){SelectObject(dc_,old_);old_=nullptr;}if(bitmap_)DeleteObject(bitmap_);
    BITMAPINFO info{};info.bmiHeader.biSize=sizeof(BITMAPINFOHEADER);info.bmiHeader.biWidth=width;info.bmiHeader.biHeight=-height;info.bmiHeader.biPlanes=1;info.bmiHeader.biBitCount=32;info.bmiHeader.biCompression=BI_RGB;
    bitmap_=CreateDIBSection(dc_,&info,DIB_RGB_COLORS,reinterpret_cast<void**>(&pixels_),nullptr,0);
    if(!bitmap_)throw std::runtime_error("Не вдалося підготувати вікно інсталятора");
    old_=SelectObject(dc_,bitmap_);width_=width;height_=height;
  }
  HDC dc()const{return dc_;}
  int width()const{return width_;}int height()const{return height_;}
  uint32_t pixel(int x,int y)const{return pixels_[static_cast<size_t>(y)*width_+x];}
  void mask(RectF bounds,float radius) {
    // ClearType is rendered on an opaque card; only the contour receives fractional alpha.
    for(int y=0;y<height_;++y)for(int x=0;x<width_;++x){
      float alpha=x+.5f>=bounds.X&&x+.5f<=bounds.GetRight()&&y+.5f>=bounds.Y&&y+.5f<=bounds.GetBottom()?1.f:0.f;
      float dx=std::max({bounds.X+radius-(x+.5f),x+.5f-(bounds.GetRight()-radius),0.f});
      float dy=std::max({bounds.Y+radius-(y+.5f),y+.5f-(bounds.GetBottom()-radius),0.f});
      if(dx>0&&dy>0) {
        float distance=std::sqrt(dx*dx+dy*dy);
        alpha*=std::clamp(radius+.5f-distance,0.f,1.f);
      }
      auto& pixel=pixels_[static_cast<size_t>(y)*width_+x];uint32_t a=static_cast<uint32_t>(std::lround(alpha*255));
      pixel=(a<<24)|(((pixel>>16)&255)*a/255<<16)|(((pixel>>8)&255)*a/255<<8)|((pixel&255)*a/255);
    }
  }
  void present(HWND window,BYTE opacity=255) {
    RECT bounds{};GetWindowRect(window,&bounds);POINT target{bounds.left,bounds.top},source{};SIZE size{width_,height_};BLENDFUNCTION blend{AC_SRC_OVER,0,opacity,AC_SRC_ALPHA};
    if(!UpdateLayeredWindow(window,nullptr,&target,&size,dc_,&source,0,&blend,ULW_ALPHA))throw std::runtime_error("Не вдалося намалювати інсталятор");
  }
  Status save(const std::wstring& file,const CLSID& encoder){Bitmap image(width_,height_,width_*4,PixelFormat32bppPARGB,reinterpret_cast<BYTE*>(pixels_));return image.Save(file.c_str(),&encoder,nullptr);}
  uint64_t hash()const{uint64_t value=1469598103934665603ULL;for(size_t i=0;i<static_cast<size_t>(width_)*height_;++i)value=(value^pixels_[i])*1099511628211ULL;return value;}
};
