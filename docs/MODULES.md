# Modules and shared-code rules

## The seam

```
hosts/standalone ─┐                 ┌─ server/services/* (one module per backend service)
                  ├─ server/core ───┤
hosts/embedded ───┘                 └─ server/crypto (formats, key recovery)
```

The **core and services are shared code**: the exact same source runs on a PC (standalone host,
dev loop) and inside the game process on the phone (embedded host).

## Rules for shared code (`server/*`)

1. **Toolchain:** Rust, pinned in `rust-toolchain.toml` (1.97.0). `rust-version = 1.80` in the
   workspace is the floor. Every shared crate must pass
   `cargo check --target aarch64-linux-android`; `scripts/build.sh` enforces it.
2. **std only + workspace dependencies.** No async runtime, no TLS stack, no system libraries.
   Adding a dependency needs a reason in the PR.
3. **No panics across the boundary.** HTTP handlers are wrapped in `catch_unwind`; the embedded
   host must never take the game down. Return errors, log them.
4. **Bind 127.0.0.1 only**, port 18080 with fallback 18081; the port in use is logged.
5. **Time comes from `core::clock`** so simulated-clock runs work.
6. **Files are written with `core::storage`** (atomic write, rotating backups, never overwrite a
   file we cannot read).
7. **No keys and no game data in source.** Keys come from `keys.json` produced by `jdr-tool` from
   the player's own copy.

## Core APIs (Build 01)

| Module | API |
|---|---|
| `http` | `bind(ports)`, `serve(listener, handler)`, `Request`, `Response`, `parse_form` |
| `log` | `init(dir, echo)`, `log!(tag, ...)`, `Counters::status_line()` |
| `clock` | `now()`, `set_offset()`, `format()`, `compact()` (JST day boundaries) |
| `storage` | `write_atomic`, `rotate_backups`, `save_guarded` |
| `settings` | `Settings`, `Keys`, `load_keys` |
| `envelope` | request decode / response encode for the game's encrypted form posts |
