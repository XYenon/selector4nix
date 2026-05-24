# Credentials Reference

`selector4nix` reads an optional TOML credentials file from the first of these locations:

1. The path specified by the `--credential-file` command line argument
2. The path specified by the `SELECTOR4NIX_CREDENTIAL_FILE` environment variable
3. `./credentials.toml` in the current directory
4. `/etc/selector4nix/credentials.toml`

If no credentials file is found, `selector4nix` continues without credentials. All upstream requests will be unauthenticated.

Credentials are only used for `/nix-cache-info` access and NAR info queries. NAR file downloads do not use credentials because they typically rely on pre-signed URLs.

## `credentials`

Authentication entries for upstream substituters.

### `credentials[].url`

- Type: URL

The URL prefix to match against upstream substituter request URLs.

When making a request to an upstream substituter, `selector4nix` selects the credential whose `url` is the longest URL prefix of the request URL. Path segment boundaries are respected: a credential for `/nix/cache1` will not match `/nix/cache10`.

### `credentials[].login`

- Type: String

The username or access key for authentication.

### `credentials[].secret`

- Type: String | None
- Default: none

The password or secret token for authentication. Omit when the upstream only requires a login (e.g. some public caches with access keys).
