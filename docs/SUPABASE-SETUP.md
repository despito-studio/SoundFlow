# Підключення власного Supabase до SoundFlow

## Що це дає

У SoundFlow Supabase зберігає **підключені музичні акаунти** (YouTube Music / SoundCloud / Spotify) для профілю.
Вхід через Google іде напряму в Google OAuth (застосунок сам обмінює код на токен), а Supabase — це сховище,
яке синхронізує ці прив'язки між пристроями.

## Крок 1. Створити проєкт

1. https://supabase.com → New project.
2. Регіон — найближчий (наприклад, Frankfurt).
3. Пароль бази зберегти собі (у застосунок він **не потрібен**).

## Крок 2. Створити таблицю

Project → **SQL Editor** → New query → вставити вміст [supabase/schema.sql](../supabase/schema.sql) → **Run**.

Має створитися таблиця `public.connected_accounts` з політиками для anon-ключа.

## Крок 3. Забрати URL і anon-ключ

Project Settings → **API**:

- **Project URL** — `https://<project-ref>.supabase.co`
- **anon public** ключ — довгий JWT, що починається з `eyJ...`

Ці два значення публічні: вони й так вшиваються в клієнтські застосунки. Їх можна передавати розробнику.

**Ніколи не передавай:**
- `service_role` ключ (повний доступ до бази, обходить RLS);
- пароль бази даних;
- JWT secret / SMTP-паролі.

## Крок 4. Прописати конфігурацію

Найпростіше — скриптом:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\configure-supabase.ps1 `
  -Url "https://xxxx.supabase.co" `
  -AnonKey "eyJhbGciOi..."
```

Скрипт пише `%APPDATA%\app.soundflow.desktop\cloud.json` і зберігає інші поля, якщо вони вже там є.

Вручну файл виглядає так:

```json
{
  "supabase_url": "https://xxxx.supabase.co",
  "supabase_anon_key": "eyJhbGciOi...",
  "google_client_id": "",
  "google_client_secret": ""
}
```

Порожні поля Google означають «використати вбудований клієнт SoundFlow».

Альтернатива без файлу — змінні середовища:
`SOUNDFLOW_SUPABASE_URL`, `SOUNDFLOW_SUPABASE_ANON_KEY`, `SOUNDFLOW_GOOGLE_CLIENT_ID`, `SOUNDFLOW_GOOGLE_CLIENT_SECRET`.

## Крок 5 (необов'язково). Власний Google OAuth клієнт

Якщо хочеш, щоб вхід ішов через **твій** Google-проєкт:

1. https://console.cloud.google.com → APIs & Services → Credentials → Create credentials → OAuth client ID.
2. Type: **Web application**.
3. Authorized redirect URIs: `http://127.0.0.1:43821/callback`.
4. OAuth consent screen: scopes `openid`, `email`, `profile`; додати тестових користувачів, поки застосунок не верифіковано.
5. Отримані Client ID і Client secret передати в `configure-supabase.ps1` (`-GoogleClientId`, `-GoogleClientSecret`).

Для десктопного застосунку client secret не є таємницею (він лежить у бінарнику), але публічно його краще не викладати.

## Перевірка

1. Запустити SoundFlow → увійти через Google.
2. У Supabase → Table Editor → `connected_accounts` має з'явитися рядок з `user_id`, `provider`, `identifier`.
3. Якщо рядків немає — перевірити `cloud.json` і що SQL-політики створилися.

## Безпека

Поточна схема дозволяє anon-ключу читати та писати `connected_accounts` (так працює клієнт: він не має Supabase-сесії).
Ключ не дає доступу до інших таблиць, але будь-хто з бінарника може прочитати цю таблицю.
Для публічного релізу варто перейти на Supabase Auth (`signInWithIdToken`) і політики `auth.uid() = user_id` — це окремий етап.
