# SoundFlow Supabase email login

This local build targets `https://ieafdutkxpyqwmfedwqy.supabase.co` using a public
publishable key. Existing cloud.json/environment overrides still take precedence.
No service-role or database secret belongs in a desktop build.

Open Profile > Connections > Server account. Register or log in there; the older
local account form remains separate. The project currently requires confirmation
by email: a successful signup without tokens displays confirmation instructions
and does not pretend the user is signed in. Server UUID selects its own library;
local profiles are preserved. Passwords are sent only to the configured HTTPS Auth
endpoint. Access/refresh tokens are saved to the OS credential vault, scoped to the
application data directory. If the vault cannot save, the session is memory-only.
Legacy plaintext session.json is discarded rather than trusted as credentials.

The Check server admin permissions button invokes soundflow_is_admin using the
logged-in user's bearer token. Errors, missing session, malformed responses, or a
missing RPC never grant privileges. There are no admin mutation endpoints or user
management panel in this build; role verification is not full service administration.

To assign the role, an owner must first create/confirm the account in Supabase Auth,
then execute migrations/003_service_administrators.sql and grant-feodal-admin.sql
in that project. A local account with the same email is insufficient. The app
cannot grant itself this role. Project connectivity was checked through Auth settings;
no account was created and no authenticated admin role was confirmed by the agent.
