-- SoundFlow — схема для синхронізації підключених музичних акаунтів.
-- Виконати у Supabase: SQL Editor → New query → вставити → Run.

create table if not exists public.connected_accounts (
    user_id     text        not null,
    provider    text        not null check (provider in ('youtube', 'soundcloud', 'spotify')),
    identifier  text        not null,
    confirmed   boolean     not null default true,
    updated_at  timestamptz not null default now(),
    primary key (user_id, provider)
);

create index if not exists connected_accounts_user_idx
    on public.connected_accounts (user_id);

alter table public.connected_accounts enable row level security;

-- Застосунок звертається до таблиці з публічним anon-ключем і без Supabase-сесії,
-- тому дозволяємо читання та upsert саме для anon. Ключ не відкриває інші таблиці.
drop policy if exists "soundflow read" on public.connected_accounts;
create policy "soundflow read" on public.connected_accounts
    for select to anon using (true);

drop policy if exists "soundflow insert" on public.connected_accounts;
create policy "soundflow insert" on public.connected_accounts
    for insert to anon with check (true);

drop policy if exists "soundflow update" on public.connected_accounts;
create policy "soundflow update" on public.connected_accounts
    for update to anon using (true) with check (true);

-- Явні права для ролей Supabase (RLS усе одно обмежує доступ політиками).
grant usage on schema public to anon, authenticated;
grant select, insert, update on public.connected_accounts to anon, authenticated;

-- Видалення не дозволяємо: застосунок ним не користується.

-- Перевірка після запуску:
--   select * from public.connected_accounts;
