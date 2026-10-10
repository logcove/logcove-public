# CLI file credentials

Decision: 2026-10-10. Implemented and locally verified, not released; published
v0.3.2 binaries still use the OS credential store. This change applies to the
CLI only.

## Contract

- Browser login stores its signed Bearer Session in `~/.logcove/auth.json`
  (`%USERPROFILE%\.logcove\auth.json` on Windows). It does not store account
  passwords. The JSON contains a `sessions` map keyed by canonical API origin,
  including scheme and port.
- Existing API configuration locations, `--config-dir` and `LOGCOVE_CONFIG_DIR`
  are unchanged. They select non-secret configuration, not the credential file.
  Invocations for the same API and OS user share their saved Session.
- The CLI no longer reads or writes Keychain, Windows Credential Manager or
  Linux Secret Service. Upgrade users run `logcove login` once. There is no
  automatic migration or deletion of old OS-store entries; desktop login is
  unchanged.
- `LOGCOVE_TOKEN` remains the higher-priority, environment-only PAT. It bypasses
  credential file reads and writes, including Session renewal persistence.

## Persistence and permissions

The directory is `0700` and credential files are `0600` on Unix. Windows
credential files have a protected owner-only DACL, reusing the write-key file
implementation. Tokens are plaintext on disk: processes running as the same
user can read them. Do not put this file in repositories or agent conversations.

All store operations use `~/.logcove/credentials.lock`. Read-modify-write occurs
inside the lock so updating one API origin preserves the others. Writers create
a private temporary file in the same directory, write and sync it, close it,
then replace `auth.json` with a rename. Readers also take the lock to avoid
Windows file-sharing conflicts during replacement. The temporary file is
removed on normal failure; a subsequent write removes a crash leftover while
holding the lock. The lock file contains no secrets.

A missing credential file means logged out. Invalid JSON, symlinks where the
credential directory/file is expected, and storage errors fail explicitly;
they do not silently reset credentials or expose file contents in errors.
Conditional deletion checks the saved Session under the same lock, preventing
an old process's logout/401 from deleting a newer login. Session renewal and
server-side logout retain their existing API behavior. Failed initial login
persistence attempts to revoke the newly issued server Session.

## Implementation and acceptance

Replace `OsCredentials` with `FileCredentials` behind the existing
`CredentialStore` interface. Remove `keyring` and the CI D-Bus build dependency;
no new third-party library is needed. Update help, error messages, package text,
and version-aware Skill guidance without changing published release notes.

Validate process-to-process persistence, concurrent writers for separate
origins, atomic replacement, permissions, corrupted-file failures, conditional
deletion, login/renewal/logout through HTTP fixtures, and environment PATs that
leave local credentials untouched. Use isolated test directories and fake
tokens. Run formatting, strict Clippy, the full Rust suite and Skill/package
checks. Cross-platform runtime results must be recorded separately from local
verification; publishing or installing the new binary is a separate action.

## Verification (2026-10-10)

- macOS ARM64 and Linux ARM64 (Rust 1.90 Bookworm container): all 97 Rust tests
  passed, including login/renewal/logout with actual file persistence and local
  HTTP fixtures. Eight concurrent child processes each performed 20 saves and
  reads; all API-origin entries were retained.
- macOS formatting, strict Clippy and release compilation passed. Seven packaging
  tests, two Skill/DuckDB checks and two publishing-workflow checks passed. The
  macOS ARM64 source-build archive passed extracted binary/help/default-API and
  bundled Skill command checks; it is a local test artifact, not a new release.
- The actual credential/config/error/private-file modules and credential tests
  passed an isolated Windows GNU target type check (using the URL type without
  the unrelated HTTP/TLS build). This does not establish Windows runtime success.
  Windows CI includes owner-only ACL checks before and after replacing auth.json.
- No real account credentials were read, migrated or changed. The installed CLI,
  published releases and desktop credential handling are unchanged. Browser UI
  approval and native Windows runtime acceptance remain separate verification.
