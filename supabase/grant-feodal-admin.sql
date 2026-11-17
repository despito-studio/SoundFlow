-- Target project: ieafdutkxpyqwmfedwqy
-- Run AFTER migrations/003_service_administrators.sql in the project's SQL Editor.
-- A local SoundFlow account is not an auth.users account.
-- Aborts instead of creating an identity or granting an unconfirmed email.
begin;
do $$
declare
    target_id uuid;
    matches integer;
begin
    select count(*) into matches from auth.users
    where lower(email) = 'feodal@tutamail.com' and deleted_at is null;
    if matches <> 1 then
        raise exception 'Expected exactly one Supabase Auth user for feodal@tutamail.com; found %. Register/sign in through Supabase Auth first.', matches;
    end if;
    select id into target_id from auth.users
    where lower(email) = 'feodal@tutamail.com'
      and deleted_at is null and email_confirmed_at is not null;
    if target_id is null then
        raise exception 'The Supabase Auth email must be confirmed before granting administrator membership.';
    end if;
    insert into soundflow_private.service_administrators(user_id)
    values(target_id) on conflict(user_id) do nothing;
end;
$$;

select u.id as user_id, u.email, a.granted_at
from soundflow_private.service_administrators a
join auth.users u on u.id = a.user_id
where lower(u.email) = 'feodal@tutamail.com';
commit;

-- To revoke later, run separately:
-- delete from soundflow_private.service_administrators where user_id = '<verified-user-uuid>'::uuid;
