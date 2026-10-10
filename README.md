# SoundFlow

Нативний музичний застосунок для Windows: **Rust + Slint**, аудіо через Rodio/Symphonia. Інтерфейс не використовує HTML, браузер, Tauri або WebView2. Python вилучений із застосунку та збірки.

Компактна світла й темна теми без синього акценту, плаваюча навігація та плеєр, адаптивні обкладинки, клавіатурне керування й нативна доступність Windows.

## Завантажити для Windows x64

[Інсталятор SoundFlow 3.2.0](https://github.com/feodaI/SoundFlow/releases/download/v3.2.0/SoundFlow-3.2.0-Setup.exe) · [Portable ZIP](https://github.com/feodaI/SoundFlow/releases/download/v3.2.0/SoundFlow-3.2.0-windows-x64.zip) · [Нотатки релізу](https://github.com/feodaI/SoundFlow/releases/tag/v3.2.0)

Інсталятор встановлює застосунок для поточного користувача та створює ярлики. Для portable-версії розпакуй увесь архів і запусти `SoundFlow.exe`. Бібліотека залишається окремо від файлів програми.

![Попередній вигляд нативного інтерфейсу Windows](docs/images/windows-light.png)

## Можливості

- Пошук, відтворення та завантаження аудіо з YouTube і SoundCloud.
- Метадані Spotify та явний вибір версії треку з YouTube/SoundCloud. Spotify не надає оригінальні аудіофайли для завантаження.
- Локальна музика, улюблені треки, плейлисти, історія, імпорт і експорт бібліотеки.
- Flow: уривки треків, пропуск, лайк і збереження в плейлист. Без спеціальної розмітки уривок вибирається поблизу середини треку; це не гарантує приспів.
- Облік фактично прослуханого часу в SQLite та імпорт історії сервісів. Перегляд статистики потребує підтвердженого сервером SoundFlow Plus.
- Один екземпляр вікна. Для онлайн-музики запускається максимум один дочірній JavaScript-процес; він завершується разом із застосунком.

## Запуск і збірка

Готова portable-версія: `dist/windows-x64/SoundFlow.exe`. Інсталятор: `dist/SoundFlow-3.2.0-Setup.exe`. Python, Node.js і WebView2 встановлювати не потрібно.

Для розробки потрібні Rust 1.92+, MSVC Build Tools, Windows SDK і Node.js. SDK можна підготувати без адміністратора:

```powershell
.\scripts\setup-windows-sdk.ps1
npm.cmd ci --prefix media-provider
node scripts/build-provider.mjs
.\scripts\build-windows.ps1 -Action Dev
```

Збірка й перевірка:

```powershell
.\scripts\build-windows.ps1 -Action Build
.\scripts\build-windows.ps1 -Action Test
.\scripts\package-windows.ps1
npm.cmd test --prefix media-provider
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-windows.ps1 -Executable .\dist\windows-x64\SoundFlow.exe
```

Докладніше: [Windows](docs/WINDOWS.md), [архітектура](docs/ARCHITECTURE.md), [хмарний API](docs/CLOUD-API.md).

Перевірки релізу 3.2.0: 23 Rust-тести, 6 тестів медіарушія, запуск у Windows, UI Automation без безіменних кнопок, один екземпляр вікна, власні іконки, завершення дочірнього процесу, повне декодування AAC/MP3 та встановлення зі збереженням бібліотеки. [Історія змін](CHANGELOG.md).

## Структура

```text
src/                 Rust: застосунок, бібліотека, статистика, аудіо, Windows
ui/                  Slint: екрани, компоненти й тема
media-provider/      JavaScript-адаптери музичних сервісів і тести
resources/           іконки, обкладинки, ліцензії, runtime та інсталятор
scripts/             відтворювана збірка, пакування й перевірка
docs/                архітектура та контракти
```

Дані користувача зберігаються в `%APPDATA%\app.soundflow.desktop`; встановлення та видалення програми не видаляють бібліотеку чи музику. Стару JSON-бібліотеку можна імпортувати в налаштуваннях.

## Хмарні акаунти й Plus

Клієнт містить авторизацію, синхронізацію та перевірку підписки через власний HTTPS-сервіс. **Сервер акаунтів і платежів у цьому репозиторії не розгорнутий**. Ціна $1/місяць задається платіжним сервісом; клієнт не може підтвердити оплату самостійно. Локальні налаштування чи імпортований JSON не розблоковують Plus.

Аудіо та доступність каталогів залежать від джерела. Публічний профіль сервісу дозволяє імпортувати доступні треки; він не дає доступу до приватної історії прослуховувань або Spotify OAuth.

Slint використовується за Royalty-free Desktop, Mobile, and Web Applications License 2.0. Атрибуція доступна в меню «Про SoundFlow». Ліцензії Node.js, музичного адаптера й обкладинок включені до Windows-пакета.
