# SoundFlow 3.5.0 — Release Notes

**Дата:** 21.10.2026 · **Платформа:** Windows x64 (portable)

## 🇺🇦 Що нового

### Акаунти та хмара
- **Supabase Auth** — Google `id_token` обмінюється на сесію Supabase, запити йдуть із Bearer-токеном, доступ до даних закритий політикою `auth.uid()`.
- **Синхронізація бібліотеки** між пристроями: «Надіслати в хмару» / «Отримати з хмари» з ревізіями.
- Власний Supabase підключається через `cloud.json` (скрипт і SQL-міграції в репозиторії).

### Інтерфейс
- **Акценти** — mono, blue, violet, rose, amber, green.
- **Command palette** — `Ctrl+K`: 16 команд, пошук по них.
- **Розумні добірки** — «Нещодавно додані», «Найчастіше слухаю», «Забуте».
- Плавна зміна обкладинки й назви треку, скелетони під час завантаження.
- Плеєр підфарбовується кольором поточної обкладинки.
- Щільніша типографіка.
- Офлайн-бейдж ↓ і «Завантажити все» для плейлиста.
- Локальний пошук по бібліотеці разом із результатами провайдера.

### Звук
- **Нормалізація гучності** (RMS, 0.35–4×) — вирівнює тихі та гучні треки.
- **Швидкість відтворення** 0.75×, 1×, 1.25×, 1.5×.

### Надійність
- Чесна детекція акаунтів: профіль вважається знайденим лише якщо публічна сторінка існує.
- Тости: 3 с для звичайних, 7 с і червоний для помилок.
- Автоматична перевірка оновлень на GitHub Releases.
- Нативні рядки перекладаються разом з інтерфейсом.

## 🇬🇧 What's new

- **Supabase Auth** — the Google `id_token` is exchanged for a Supabase session; RLS scopes rows to `auth.uid()`.
- **Cross-device library sync** with push/pull and revisions.
- **Accent palettes**, a **Ctrl+K command palette**, **smart picks** (recently added / most played / forgotten).
- Track cross-fade, loading skeletons and an artwork-tinted player bar.
- **Volume normalization** and **playback speed** 0.75–1.5×.
- Offline badges, "download all" for playlists and local library search.
- Honest account detection, smarter toasts and automatic update checks.

## Installation / Встановлення

1. Розпакуй архів у будь-яку теку.
2. Запусти `SoundFlow.exe` — `resources` і `media-provider` мають лежати поруч.
3. Бібліотека й налаштування зберігаються в `%APPDATA%\app.soundflow.desktop` і не видаляються під час оновлення.
