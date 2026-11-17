-- SoundFlow: server-owned administrator membership. Run as postgres in SQL Editor.
-- Does not grant Supabase dashboard access or bypass existing library RLS policies.
begin;
create schema if not exists soundflow_private;
revoke all on schema soundflow_private from public, anon, authenticated;

create table if not exists soundflow_private.service_administrators (
    user_id uuid primary key references auth.users(id) on delete cascade,
    granted_at timestamptz not null default now(),
    granted_by text not null default current_user
);
alter table soundflow_private.service_administrators enable row level security;
revoke all on table soundflow_private.service_administrators from public, anon, authenticated;

-- Only reports the caller's own role. No user-supplied UUID or editable metadata.
create or replace function public.soundflow_is_admin()
returns boolean
language sql stable security definer
set search_path = ''
as $$
    select exists (
        select 1 from soundflow_private.service_administrators a
        where a.user_id = auth.uid()
    );
$$;
revoke all on function public.soundflow_is_admin() from public, anon, authenticated;
grant execute on function public.soundflow_is_admin() to authenticated;
commit;
