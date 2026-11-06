# Security

## What the current version is

A local demonstration of the ingestion contract. The two API keys are public demo values written in the source code. Do not expose this version to the internet or put real customer data in it.

## What it already does

- The project comes from the API key on the server, never from the request body.
- Event ids are scoped per project, so one project cannot read or change another's events.
- Input is strict: unknown fields are rejected and string lengths are bounded.

## Planned before any hosted use

- Keys stored hashed, with rotation, instead of fixed values in code.
- Rate limits per key.
- Request tracing without event contents in logs.

## Reporting a vulnerability

Please do not open a public issue. Report it privately through the repository's [Security tab](https://github.com/acamilateixeira/metertrail/security), or contact the maintainer through her [GitHub profile](https://github.com/acamilateixeira). Include steps to reproduce and the affected commit.
