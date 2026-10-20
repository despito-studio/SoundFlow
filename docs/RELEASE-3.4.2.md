# SoundFlow 3.4.2 — Release Notes

**Дата:** 20.10.2026 · **Платформа:** Windows x64 (portable)
**Артефакт:** `SoundFlow-3.4.2-windows-x64.zip` (53.7 MB)
**SHA-256:** `9e890279740a0833704e7be9d2876652fedf66c99a83b030b64b4ef7325bd834`

## 🇺🇦 Що нового

### Повернення після входу
- Вкладка браузера закривається сама після Google OAuth — більше немає екрана «Авторизація Google успішна».
- Вікно SoundFlow піднімається на передній план, щоб одразу повернутися до застосунку.

### Повідомлення
- Звичайні тости зникають через 3 секунди.
- Помилки тримаються 7 секунд і виділяються червоним кольором.

### Імпорт бібліотеки
- Після входу SoundFlow питає, чи імпортувати музику, і показує лише ті сервіси, які справді знайдені через Google-профіль.
- Нова опція «Більше не питати» зберігається в налаштуваннях.

### Чесна детекція акаунтів
- Кожна знахідка перевіряється реальним запитом до публічної сторінки профілю.
- Якщо сторінки не існує (404), акаунт більше не показується як «знайдено».

### Оновлення
- Застосунок сам перевіряє GitHub Releases у фоні.
- Коли є новіша версія, у меню «⋯» з'являється пункт «Доступне оновлення», а у вікні «Про SoundFlow» — версія й посилання.

### Інфраструктура
- GitHub Actions: збірка Windows-білда та прогін тестів (26 тестів) на кожен push і тег.

## 🇬🇧 What's new

- **Silent OAuth return** — the browser tab closes itself after Google sign-in and the app window comes back to the foreground.
- **Smarter toasts** — regular messages vanish after 3 s, errors stay 7 s and are shown in red.
- **Library import prompt** — asked once after Google sign-in, lists only accounts actually discovered through the Google profile, and can be disabled with "Don't ask again".
- **Honest account detection** — every discovered profile is verified against its public page; 404 means "not found".
- **Update checks** — the app polls GitHub Releases and surfaces a menu entry and About line when a newer version exists.
- **CI** — GitHub Actions builds the Windows package and runs the 26-test suite on every push and tag.

## Встановлення / Installation

1. Розпакуй архів у будь-яку теку.
2. Запусти `SoundFlow.exe` — `resources` і `media-provider` мають лежати поруч.
3. Бібліотека й налаштування зберігаються в `%APPDATA%\app.soundflow.desktop` і не видаляються під час оновлення.
