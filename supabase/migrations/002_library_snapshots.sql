-- SoundFlow — синхронізація бібліотеки між пристроями.
-- Один знімок бібліотеки на користувача; доступ лише власнику (auth.uid()).

create table if not exists public.library_snapshots (
    user_id     uuid        primary key,
    payload     jsonb       not null,
    revision    bigint      not null default 1,
    updated_at  timestamptz not null default now()
);

alter table public.library_snapshots enable row level security;

drop policy if exists "own snapshot read" on public.library_snapshots;
create policy "own snapshot read" on public.library_snapshots
    for select to authenticated using (auth.uid() = user_id);

drop policy if exists "own snapshot insert" on public.library_snapshots;
create policy "own snapshot insert" on public.library_snapshots
    for insert to authenticated with check (auth.uid() = user_id);

drop policy if exists "own snapshot update" on public.library_snapshots;
create policy "own snapshot update" on public.library_snapshots
    for update to authenticated using (auth.uid() = user_id) with check (auth.uid() = user_id);

grant select, insert, update on public.library_snapshots to authenticated;
