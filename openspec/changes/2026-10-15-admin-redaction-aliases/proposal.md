# Redact common Admin credential and SQL aliases

## Why

Authenticated Admin JSON responses currently redact exact keys such as `url`, `sql`, and `lastErrorMessage`, but leave common aliases such as `databaseUrl`, `connectionString`, `statementSql`, and `exceptionMessage` visible. These aliases can expose database credentials, SQL text, and driver diagnostics.

## What changes

- Classify normalized URL, connection-string, username, SQL, parameter, and error-message aliases as sensitive in secure Admin responses.
- Keep numeric counters and other safe fields visible; retain compatibility mode behavior.
- Test nested objects and arrays with aliases and safe fields.

## Scope

Admin JSON response redaction only. This does not certify authentication, TLS, or all P06 security requirements.
