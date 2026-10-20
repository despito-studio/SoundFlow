-- SoundFlow — перехід на Supabase Auth (RLS за auth.uid()).
-- Виконати у SQL Editor після того, як у Supabase увімкнено Google-провайдера.
-- Увага: таблиця перестворюється, попередні рядки буде видалено.

drop table if exists public.connected_accounts;

create table public.connected_accounts (
    -- auth.uid() користувача Supabase (uuid з auth.users)
    user_id     uuid        not null,
    provider    text        not null check (provider in ('youtube', 'soundcloud', 'spotify')),
    identifier  text        not null,
    confirmed   boolean     not null default true,
    updated_at  timestamptz not null default now(),
    primary key (user_id, provider)
);

create index connected_accounts_user_idx
    on public.connected_accounts (user_id);

alter table public.connected_accounts enable row level security;

-- Кожен бачить і змінює лише свої рядки. Для anon доступу немає взагалі.
create policy "own rows read" on public.connected_accounts
    for select to authenticated using (auth.uid() = user_id);

create policy "own rows insert" on public.connected_accounts
    for insert to authenticated with check (auth.uid() = user_id);

create policy "own rows update" on public.connected_accounts
    for update to authenticated using (auth.uid() = user_id) with check (auth.uid() = user_id);

grant usage on schema public to authenticated;
grant select, insert, update on public.connected_accounts to authenticated;

-- Перевірка: під anon-ключем таблиця має бути недоступною,
-- під сесією користувача — показувати лише його рядки.
