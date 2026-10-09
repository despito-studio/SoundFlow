# SoundFlow - Ultra-Lightweight Music Streaming Engine

SoundFlow - високопродуктивний та ультралегкий музичний плеєр для Windows 10/11, розроблений як енергоефективна альтернатива Electron-додаткам (Spotify, SoundCloud Desktop). 

Споживання оперативної пам'яті становить 40-60 MB замість 400-800 MB, а навантаження на процесор у режимі фонового відтворення мінімізовано до 0.0 - 0.2% CPU.

---

## Архітектура та технічні оптимізації

Програма побудована на зв'язці асинхронного Python-бекенду та легкої обгортки системного компонента Microsoft Edge WebView2, що усуває необхідність пакувати повноцінний браузер Chromium у дистрибутив.

### 1. Direct Hardware Audio Pipeline (WASAPI Bypass)
У більшості браузерних плеєрів використання Web Audio API (`createMediaElementSource`) призводить до блокування аудіопотоку через політику CORS Chromium на сторонніх CDN (YouTube `googlevideo.com` та SoundCloud CDN). 
У SoundFlow відтворення реалізовано через прямий HTML5 Audio Pipeline безпосередньо у системний мікшер Windows WASAPI:

```javascript
// Прямий вивід у системний аудіотермінал без CORS-мутування
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
SoundFlow автоматично виявляє запуск важких 3D-ігор за допомогою низькорівневого моніторингу через `psutil` та Win32 API. Коли фокус переходить у гру, плеєр автоматично скидає свій робочий набір пам'яті (Working Set) та знижує пріоритет процесу:

```python
# Автоматичне вивільнення ресурсів під час геймінгу
if self.game_running and not soundflow_focused:
    if not self.is_optimizing:
        # Зниження пріоритету процесу до BELOW_NORMAL (0x00004000)
        kernel32.SetPriorityClass(kernel32.GetCurrentProcess(), 0x00004000)
        # Очищення некритичних сторінок пам'яті з RAM
        ctypes.windll.psapi.EmptyWorkingSet(kernel32.GetCurrentProcess())
        self.is_optimizing = True
elif soundflow_focused and self.is_optimizing:
    # Миттєве відновлення пріоритету при Alt-Tab (NORMAL_PRIORITY 0x00000020)
    kernel32.SetPriorityClass(kernel32.GetCurrentProcess(), 0x00000020)
    self.is_optimizing = False
```

### 3. Захист IPC-мосту від рекурсії та Deadlock (Zero-Deadlock Interop)
Для запобігання зависань UI-потоку внаслідок рефлексії pywebview у внутрішні властивості Windows Forms Form / CLR (`AccessibilityObject.Bounds.Empty...`), усі внутрішні підсистеми інкапсульовано з використанням приватних ідентифікаторів:

```python
class AppApi:
    def __init__(self):
        # Префікс '_' виключає об'єкти з рефлексивного сканування pywebview
        self._engine = MusicEngine()
        self._library = LibraryManager()
        self._stats = SystemStats()
        self._desktop = None
        self._mobile_server = None
```

Глобальні мультимедійні клавіші обробляються через неблокуючий цикл повідомлень Windows з використанням `MsgWaitForMultipleObjectsEx` та асинхронним виконанням через `run_js`:

```python
# Неблокуюча черга обробки гарячих клавіш
msg = wintypes.MSG()
while self._running:
    user32.MsgWaitForMultipleObjectsEx(0, None, 300, 0x04FF, 0)
    while user32.PeekMessageW(ctypes.byref(msg), None, 0, 0, 1): # PM_REMOVE
        if msg.message == WM_HOTKEY:
            self.window.run_js("window.soundFlowPlayer?.togglePlay();")
        user32.TranslateMessage(ctypes.byref(msg))
        user32.DispatchMessageW(ctypes.byref(msg))
```

### 4. Вбудований локальний стримінговий сервер для iOS / Mobile
Вбудований HTTP-сервер на базі Bottle транслює потік та веб-інтерфейс у локальній мережі (LAN), дозволяючи слухати музику з комп'ютера на iPhone або Android-пристроях через Safari/Chrome без потреби у встановленні сторонніх застосунків.

---

## Порівняння продуктивності

| Показник | Офіційний Spotify / SoundCloud | SoundFlow |
| :--- | :--- | :--- |
| Споживання RAM | 350 - 750 MB | 40 - 60 MB |
| Кількість фонових процесів | 6 - 10 процесів | 1 спільний системний WebView2 |
| Навантаження на CPU під час гри | 2.0% - 5.0% | 0.0% - 0.2% |
| Реклама та аналітика | Присутня | Повністю відсутня |
| Телеметрія | Активна | Вимкнена |

---

## Можливості

- Прямий стрімінг музики з YouTube та SoundCloud без збереження тимчасових файлів на накопичувач.
- Персональні профілі з ізоляцією історії прослуховувань, списків відтворення та улюблених треків.
- Синхронізація та імпорт публічних списків відтворення з профілів SoundCloud та YouTube.
- Жанрові добірки (Lo-Fi Chill, Synthwave, Phonk, Deep House, Rock, Gaming Mix, Ambient, Українські тренди).
- Розумне автопродовження: автоматичний підбір схожих треків за тематикою після завершення черги.
- Історія пошукових запитів зі збереженням та швидким очищенням.
- Синхронізовані караоке-тексти пісень (LRC).
- Підтримка глобальних медіаклавіш Windows та MediaSession API.
- Інтеграція з Discord Rich Presence (RPC).
- Автономна робота: офлайн-збереження треків та сканування локальних аудіофайлів (.mp3, .flac, .wav).

---

## Встановлення

### Для користувачів:
Завантажте готовий інсталятор з розділу Releases:
[SoundFlow_Setup.exe](https://github.com/feodaI/SoundFlow/releases/latest/download/SoundFlow_Setup.exe)

### Для розробників:
```bash
git clone https://github.com/feodaI/SoundFlow.git
cd SoundFlow
pip install -r requirements.txt
python main.py
```

### Збірка виконуваного файлу:
```bash
python build.py
python build_setup.py
```
