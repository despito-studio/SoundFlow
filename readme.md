# 🎵 SoundFlow — Ультралегкий Музичний Плеєр

> **Сучасний плеєр у стилі Spotify та SoundCloud, розроблений для максимальної економії оперативної пам'яті (RAM) та процесора (CPU).**

<p align="center">
  <a href="https://github.com/feodaI/SoundFlow/releases/latest/download/SoundFlow_Setup.exe">
    <img src="https://img.shields.io/badge/⬇️_ЗАВАНТАЖИТИ_SOUNDFLOW_(SETUP.EXE)-2ea44f?style=for-the-badge&logo=windows&logoColor=white" alt="Завантажити SoundFlow для Windows" height="46">
  </a>
</p>

<p align="center">
  <a href="https://github.com/feodaI/SoundFlow/releases/latest">
    <img src="https://img.shields.io/github/v/release/feodaI/SoundFlow?label=Версія&color=ff2d75&style=flat-square" alt="Release">
  </a>
  <img src="https://img.shields.io/badge/Платформа-Windows_10_%2F_11-blue?style=flat-square" alt="Platform">
  <img src="https://img.shields.io/badge/ОЗП-менше_60_MB-success?style=flat-square" alt="RAM">
  <img src="https://img.shields.io/badge/Ліцензія-Free-orange?style=flat-square" alt="License">
</p>

---

## ⚡ Чому SoundFlow такий швидкий та легкий?

| Показник | Офіційний Spotify / SoundCloud Desktop | SoundFlow |
| :--- | :--- | :--- |
| **Споживання пам'яті (RAM)** | **350 – 700+ MB** | **~40 – 60 MB** (в 10 разів менше!) |
| **Кількість фонових процесів** | 6 – 10 процесів Chromium/Electron | 1 спільний системний WebView2 |
| **Навантаження на процесор (CPU)** | 1 – 4% у фоні | **0.0% – 0.3%** під час програвання |
| **Реклама та телеметрія** | Присутні | **Повністю відсутні** |
| **Потреба у підписці** | Потрібна Premium | **Безкоштовно** |

---

## ✨ Основні можливості

- **🎧 Прямий стрімінг без завантаження на диск**:
  - Пошук та відтворення будь-яких треків із **SoundCloud** та **YouTube**.
  - Фільтри пошуку: `Всі джерела`, тільки `SoundCloud` або тільки `YouTube`.
- **📻 Категорії та рекомендації (Discover)**:
  - Готові жанрові добірки в один клік: *Lo-Fi Chill, Synthwave, Phonk Drift, Deep House, Rock, Gaming Mix, Ambient, Українська музика*.
- **📊 SoundCloud Waveform Visualizer**:
  - Живий аудіо-візуалізатор хвиль у стилі SoundCloud, який автоматично зупиняє анімацію на паузі для економії 100% ресурсів процесора.
- **❤️ Власна медіатека**:
  - Додавання в «Улюблені» (лайк треку).
  - Створення необмеженої кількості власних плейлистів.
  - Історія нещодавно прослуханих пісень та історія пошукових запитів.
  - Автоматичне генерування плейлистів найбільш прослуховуваних треків.
  - Збереження бібліотеки в локальний файл.
- **🎛️ Повноцінний плеєр**:
  - Shuffle (випадковий порядок), Repeat (повторювати все / повторювати цей трек / вимкнено).
  - Розумне автопродовження: автоматичний підбір схожих треків за тематикою після завершення черги.
  - Розумне попереднє кешування (pre-fetching): наступний трек підвантажується за 15 секунд до завершення поточного для безперервного відтворення.
  - Регулювання гучності та швидкий Mute.
  - Підтримка мультимедійних клавіш Windows (MediaSession API).
- **🔐 Авторизація та профілі**:
  - Швидкий та безпечний вхід в 1 клік через Google чи GitHub, або через Email.
  - Режим швидкого входу гостем (Guest Mode).
  - Синхронізація публічних списків відтворення та треків із профілів SoundCloud та YouTube.
  - Ізоляція бібліотеки та історії прослуховувань для кожного користувача.
- **👑 Панель адміністратора**:
  - Доступна виключно для ролей адміністратора та модератора.
  - Апаратний Studio Bass Boost (DSP) на базі WebAudio Biquad Filter із регулюванням до +14 dB.
  - Studio HQ Audio (320 kbps) без компресії.
  - Очищення кешу та оптимізація в один клік.
  - Експорт та імпорт бібліотеки (JSON бекап).
- **⚡ Вбудований монітор ресурсів**:
  - Віджет у нижньому лівому куті програми показує реальне споживання ОЗП та ЦП у реальному часі.
- **📱 Локальний мобільний стрімінг (iOS / Android)**:
  - Вбудований LAN-сервер для прослуховування музики з ПК на iPhone/Android через мобільний браузер.

---

## Архітектура та низькорівневі оптимізації

### 1. Direct Hardware Audio Pipeline (WASAPI Bypass)
Використання Web Audio API (`createMediaElementSource`) у Chromium призводить до блокування аудіопотоку через політику CORS на сторонніх CDN (YouTube `googlevideo.com` та SoundCloud CDN). У SoundFlow аудіо транслюється через прямий HTML5 Audio Pipeline безпосередньо у системний мікшер Windows WASAPI:

```javascript
if (!this.audio) {
    this.audio = document.getElementById('soundflow-audio') || new Audio();
    this.audio.preload = 'auto';
}
this.audio.muted = this.isMuted;
this.audio.volume = this.isMuted ? 0 : (this.lastVolume || 0.8);
this.audio.src = streamUrl;
this.audio.play();
```

### 2. Автоматичний Game Optimizer (Dota 2, CS2 та інші 3D-ігри)
SoundFlow автоматично виявляє активні 3D-ігри через `psutil` та Win32 API. Коли фокус переходить у гру, плеєр автоматично скидає свій робочий набір пам'яті (Working Set) та знижує пріоритет процесу:

```python
if self.game_running and not soundflow_focused:
    if not self.is_optimizing:
        # Зниження пріоритету процесу до BELOW_NORMAL (0x00004000)
        kernel32.SetPriorityClass(kernel32.GetCurrentProcess(), 0x00004000)
        # Очищення некритичних сторінок пам'яті з RAM
        ctypes.windll.psapi.EmptyWorkingSet(kernel32.GetCurrentProcess())
        self.is_optimizing = True
elif soundflow_focused and self.is_optimizing:
    # Миттєве відновлення пріоритету при фокусі
    kernel32.SetPriorityClass(kernel32.GetCurrentProcess(), 0x00000020)
    self.is_optimizing = False
```

### 3. Захист IPC-мосту від рекурсії та Deadlock
Для запобігання зависанню UI-потоку внаслідок рефлексивного сканування pywebview у внутрішні властивості Windows Forms CLR, внутрішні менеджери інкапсульовано з префіксом `_`:

```python
class AppApi:
    def __init__(self):
        self._engine = MusicEngine()
        self._library = LibraryManager()
        self._stats = SystemStats()
        self._desktop = None
        self._mobile_server = None
```

### 4. Неблокуюча черга обробки Win32-повідомлень
Глобальні мультимедійні клавіші Windows обробляються через чергу `MsgWaitForMultipleObjectsEx` та `PeekMessageW` без блокування основного циклу:

```python
msg = wintypes.MSG()
while self._running:
    user32.MsgWaitForMultipleObjectsEx(0, None, 300, 0x04FF, 0)
    while user32.PeekMessageW(ctypes.byref(msg), None, 0, 0, 1): # PM_REMOVE
        if msg.message == WM_HOTKEY:
            self.window.run_js("window.soundFlowPlayer?.togglePlay();")
        user32.TranslateMessage(ctypes.byref(msg))
        user32.DispatchMessageW(ctypes.byref(msg))
```

---

## 🚀 Встановлення та запуск

### 1. Швидке встановлення для користувачів (Рекомендовано):
1. Завантажте файл [**`SoundFlow_Setup.exe`**](https://github.com/feodaI/SoundFlow/releases/latest/download/SoundFlow_Setup.exe).
2. Запустіть його та завершіть встановлення в 1 клік.
3. Іконка програми з'явиться на Робочому столі та в меню Пуск.

### 2. Запуск із сирцевого коду (для розробників):
```bash
git clone https://github.com/feodaI/SoundFlow.git
cd SoundFlow
pip install -r requirements.txt
python main.py
```
*(або двічі клікніть на `run.bat`)*

---

## ⌨️ Гарячі клавіші

- **`Пробіл (Space)`** — Пауза / Відтворення
- **`Стрілка вправо (→)`** — Перемотати на 5 секунд уперед
- **`Стрілка вліво (←)`** — Перемотати на 5 секунд назад
- **`Стрілка вгору (↑)`** — Збільшити гучність на 5%
- **`Стрілка вниз (↓)`** — Зменшити гучність на 5%
- **`M`** — Вимкнути / увімкнути звук (Mute)
- **`/`** — Швидкий перехід у поле пошуку

---

## 📂 Структура проекту

```
c:\flowproject\
├── run.bat                 # Запуск плеєра в 1 клік
├── main.py                 # Головний файл (налаштування вікна та оптимізації WebView2)
├── backend/
│   ├── api.py              # Міст між Python та JavaScript інтерфейсом
│   ├── music_engine.py     # Пошук і отримання стрімів (SoundCloud + YouTube)
│   ├── library.py          # Збереження улюблених, плейлистів та історії
│   ├── system_stats.py     # Моніторинг RAM і CPU та Game Optimizer
│   ├── desktop_integration.py # Глобальні медіаклавіші Windows
│   └── mobile_server.py    # Локальний сервер для мобільних браузерів
├── frontend/
│   ├── index.html          # Розмітка сучасного інтерфейсу
│   ├── css/
│   │   ├── style.css       # Темна стилістика Spotify/SoundCloud
│   │   └── player.css      # Нижній бар плеєра, хвилі та черга
│   └── js/
│       ├── app.js          # Логіка додатку та навігація
│       ├── auth.js         # Авторизація та гостьовий режим
│       ├── player.js       # Двигун відтворення аудіо
│       └── visualizer.js   # Анімовані хвилі звуку
└── data/
    └── library.json        # Файл бази даних користувача
```
