# SoundFlow cloud contract — v1

The native Rust client uses an optional first-party HTTPS service. This repository
contains the client, not a deployed account/payment service. Clerk is not used.

The configured base URL is a dedicated HTTPS origin. Redirects and credentials
embedded in URLs are rejected. Authenticated requests carry
`Authorization: Bearer <access_token>`; tokens are stored in the operating
system credential vault and never exposed to UI models or the media worker.

| Endpoint | Request | Response |
| --- | --- | --- |
| `POST /v1/auth/register` | `{name,email,password}` | `{user:{id,name,email},access_token,expires_at}` |
| `POST /v1/auth/login` | `{email,password}` | Same session shape |
| `GET /v1/auth/me` | Authenticated | `{user:{id,name,email}}` |
| `GET /v1/entitlement` | Authenticated | `{user_id,plan:"soundflow_plus",active,expires_at}` |
| `POST /v1/checkout` | `{plan:"soundflow_plus"}` | `{user_id,url}` |
| `GET /v1/library` | Authenticated | `{library,revision}` |
| `PUT /v1/library` | `{library,revision}` and `If-Match` | `{revision}` |

`expires_at` is a Unix timestamp in seconds or an RFC3339 date. IDs and
revisions are opaque nonempty strings. Libraries use the validated local
SoundFlow JSON schema; unknown authority fields such as roles and entitlements
must never grant access. Accounts have separate local libraries.

The server must derive the user from the verified token on every request,
enforce authorization, validate library sizes/schema, securely hash passwords,
rate-limit authentication, and derive entitlements from verified payment
events. The client does not grant access on a network error, an expired
entitlement, a different user ID, or an unrecognized plan.

Checkout responses currently accept only HTTPS `checkout.stripe.com` or
`billing.stripe.com` URLs. The user is taken to the external browser. Plus is
enabled only after a later authenticated entitlement check confirms payment;
an opened checkout URL alone is not proof of purchase.

For synchronization, the server must atomically compare the submitted revision
before accepting a write. Respond `409` or `412` when the library has changed.
Never accept a stale revision silently. The client keeps its local library
on a conflict. Authentication errors use `401`/`403`, malformed payloads use
`400`, and non-success responses may include `{error:"safe message"}`.

The client exposes explicit push and pull actions. Pull requires a native local
backup before replacing the library. A first push cannot overwrite an existing
remote library without reconciliation: the client requires a pull/merge first
and keeps the revision unknown until that reconciliation succeeds.

Future mobile purchases need server-verified Apple/Google receipt handling and
the same account entitlement. They are not implemented by the Windows client.

## Supabase & Google Cloud Integration

SoundFlow integrates with **Google Cloud** (OAuth 2.0 Client credentials) and **Supabase** (Auth + PostgreSQL database) for identity and multi-account music synchronization:

1. **Google OAuth 2.0 & Supabase Auth**:
   - The desktop client launches a local loopback server (`127.0.0.1:43821` with dynamic port fallback) and opens the system browser to the Supabase / Google OAuth authorize URL.
   - Upon authentication, Supabase or Google returns tokens to the local callback.
   - User profile details (`id`, `email`, `name`, `avatar_url`) are fetched and persisted in the local accounts store as well as the Supabase `profiles` table.

2. **Immediate Multi-Service Music Detection**:
   - Immediately following Google sign-in, SoundFlow runs `detect_connected_accounts` across YouTube Music, SoundCloud, and Spotify.
   - If accounts exist under that user identity or previous linked services, they are automatically resolved and linked.

3. **Mandatory Confirmation & Account Customization Modal**:
   - Whether accounts were found automatically or not, the "Connected Music Accounts" modal window opens.
   - The user confirms the identified accounts or connects separate custom handles, playlists, or channels for each service individually.
   - Accounts are synced to Supabase `connected_accounts` table:
     - `user_id`: UUID/string
     - `provider`: `youtube` | `soundcloud` | `spotify`
     - `identifier`: string handle, URL, or channel ID
     - `confirmed`: boolean
     - `updated_at`: timestamp

