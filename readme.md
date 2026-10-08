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
  - Історія нещодавно прослуханих пісень.
  - Збереження бібліотеки в локальний файл [data/library.json](file:///c:/soundproject/data/library.json).
- **🎛️ Повноцінний плеєр**:
  - Shuffle (випадковий порядок), Repeat (повторювати все / повторювати цей трек / вимкнено).
  - Розумне попереднє кешування (pre-fetching): наступний трек підвантажується за 15 секунд до завершення поточного для безперервного відтворення.
  - Регулювання гучності та швидкий Mute.
  - Підтримка мультимедійних клавіш Windows (MediaSession API).
- **🔐 Авторизація Clerk (Google та GitHub OAuth)**:
  - Швидкий та безпечний вхід в 1 клік через Google чи GitHub, або через Email.
  - Персональна бібліотека та налаштування, прив'язані до вашого акаунта.
- **👤 Профіль у стилі SoundCloud та система ролей**:
  - Естетичний профіль із кастомізацією банерів (*Beige Aesthetic, Neon Sunset, Cyberpunk* тощо).
  - Бейджі ролей: **Слухач**, **Виконавець**, **Модератор**, **Адміністратор (Admin)**.
  - Картка останнього відтвореного треку з інтерактивною SoundCloud звуковою хвилею.
- **👑 Ексклюзивна панель адміністратора (Admin Superpowers)**:
  - 🎛️ **Апаратний Studio Bass Boost (DSP)** на базі WebAudio Biquad Filter із регулюванням до **+14 dB** у реальному часі.
  - 💎 **Studio HQ Audio (320 kbps)** без компресії.
  - 🧹 Очищення кешу та оптимізація в один клік.
  - 💾 Експорт та імпорт бібліотеки (JSON бекап).
- **⚡ Вбудований монітор ресурсів**:
  - Віджет у нижньому лівому куті програми показує реальне споживання ОЗП та ЦП у реальному часі.
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
c:\soundproject\
├── run.bat                 # Запуск плеєра в 1 клік
├── main.py                 # Головний файл (налаштування вікна та оптимізації WebView2)
├── backend/
│   ├── api.py              # Міст між Python та JavaScript інтерфейсом
│   ├── music_engine.py     # Пошук і отримання стрімів (SoundCloud + YouTube)
│   ├── library.py          # Збереження улюблених, плейлистів та історії
│   └── system_stats.py     # Моніторинг RAM і CPU
├── frontend/
│   ├── index.html          # Розмітка сучасного інтерфейсу
│   ├── css/
│   │   ├── style.css       # Темна стилістика Spotify/SoundCloud
│   │   └── player.css      # Нижній бар плеєра, хвилі та черга
│   └── js/
│       ├── app.js          # Логіка додатку та навігація
│       ├── player.js       # Двигун відтворення аудіо
│       └── visualizer.js   # Анімовані хвилі звуку
└── data/
    └── library.json        # Файл бази даних користувача
```
