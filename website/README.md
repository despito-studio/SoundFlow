# SoundFlow website — deployment

English-first site with Ukrainian toggle, real Windows screenshots, supplied
background artwork, reduced-motion support and Supabase email accounts.

## Ready-to-upload package

Use output/SoundFlow-site-ready.zip. Extract its contents directly into the host's
site root. Its index.html is already the NEW music design; no renaming is needed.
The previous minimalist design is available at minimal.html. The release ZIP
contains only deployment assets, no test/preparation scripts.

Preview source at http://127.0.0.1:8088/soundcloud.html. No DNS or public hosting
changes have been made. Enable HTTPS for soundflow.bond at your static host.
Preserve /updates/win-x64/ if deploying beside the existing update channel.
Do not overwrite its packages with a website-only Pages artifact.

## Supabase setup

Project: ieafdutkxpyqwmfedwqy. The bundled sb_publishable key is public, not an
administrative secret. Sign-in and registration call Supabase Auth directly.
In Supabase Authentication > URL Configuration set Site URL to
https://soundflow.bond and allow https://soundflow.bond/ as a redirect URL.
Only add www if the host actually serves it; configure matching redirects there.
Email signup requires confirmation according to the project settings. Confirm
the email first, then sign in. Passwords are never stored. Session tokens use
sessionStorage, are reverified with /auth/v1/user on reload, and are discarded
on logout/tab closure. They are not shared automatically with the Windows app.

Admin labels require POST /rest/v1/rpc/soundflow_is_admin to return literal true
for the user's JWT. Missing RPC/network errors do not grant privileges. This
site does not contain administrator mutation actions or an admin dashboard.
Use the existing SQL migration/role assignment from the repository if desired.
No real accounts were created by browser tests; auth tests mock server responses.
Actual Auth settings connectivity was previously checked with the public key.

## Download fix

The old alternate page linked index.html#download, which navigated to the old
layout. All Download buttons now point directly to the current installer with
build-specific immutable filename. release.json records its SHA-256. Replace
release.json and link filenames deliberately for the next build; do not cache
HTML permanently. Current local preview version remains 1.1.1, not a new public
release. Installer is not publisher-signed. Exit SoundFlow before installing.

## Artwork and animation

Supplied charcoal background: attachment
394090a07a4a0476b5dfd1d3e9194d80fc501dd74754f302bcdf548fe2008701.
The artwork is dimmed behind translucent page panels; a browser page cannot be
transparent through to the Windows desktop. Other supplied portraits are editorial
cards. Provenance is not a claim of ownership. App screenshots are actual local
Release captures; music preview cards are illustrative and produce no sound.
Onest SIL OFL license is included in assets. See resources/ARTWORK.md for covers.

Tests: output/test_site_upgrade.py exercises both layouts, responsive widths,
immutable downloads and mocked login/role/session/confirmation. It does not prove
production email delivery, DNS, HTTPS or completed admin-role assignment.
