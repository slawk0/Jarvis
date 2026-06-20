# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Full dev (Vite + Rust backend + typesafe-i18n watcher)
pnpm tauri dev

# Frontend-only dev (Vite + i18n watcher in parallel)
pnpm dev

# Type-check frontend
pnpm check

# Production build
pnpm tauri build
```

The `dev` script runs both `vite dev` and `typesafe-i18n` watcher in parallel — both must run together when adding translation keys so generated types stay in sync.

## Architecture Overview

**Jarvis** is a Tauri v2 desktop app: a Svelte 5 frontend communicating with a Rust backend over `invoke()` calls and Tauri events.

### Frontend (`src/`)

- **Svelte 5 runes** (`$state`, `$derived`, `$effect`) — not Svelte 4 stores. The entire workspace state lives in `src/routes/+page.svelte`.
- **Multi-pane workspace**: up to 4 panes arranged on a 120×120 unit CSS grid. Each `Pane` object has `(r1, c1, r2, c2)` grid coordinates and an `activeTab`. Panes can be split, resized (drag on resizer elements), and rearranged via drag-and-drop.
- **Tab IDs** are the canonical set defined in [`src/lib/i18n/nav.ts`](src/lib/i18n/nav.ts) as `TAB_IDS`. Each tab maps to a feature component rendered inside the active pane.
- **SFTP transfers** use a Svelte store in [`src/lib/sftp/transferStore.svelte.ts`](src/lib/sftp/transferStore.svelte.ts) and drive the `SftpTransferPanel` overlay.
- **i18n**: typesafe-i18n with EN and PL locales in `src/lib/i18n/en/` and `src/lib/i18n/pl/`. Types are auto-generated into `i18n-types.ts` by the watcher. Adding a key requires updating both locale files.
- **Back navigation**: `src/lib/backNavigation.svelte.ts` provides a push/pop stack that feature components can use. Mouse button 3 and the workspace-bar back button both call `performBack()`.

### Rust backend (`src-tauri/src/`)

- **`lib.rs`**: All Tauri commands. `AppState` (managed singleton) holds the SSH connection, sudo password (with 15-min expiry and 5-failure lockout), terminal session maps, and SFTP transfer cancellation flags.
- **`ssh.rs`**: `SshConnection` wraps `russh`. Each command grabs a clone of the connection from `AppState` rather than holding the lock.
- **Terminal sessions**: each `TerminalView` pane spawns a *new* SSH connection (separate from the main one) and a PTY channel. Output streams via `terminal-stdout-{sessionId}` Tauri events. Sessions are keyed by UUID in `terminal_txs/resizes/cancels` maps.
- **SFTP transfers**: `sftp_transfer.rs` handles upload/download/move/delete batches. Progress events are `sftp-transfer-progress`/`sftp-transfer-done`. Cancellation uses an `AtomicBool`.
- **Sudo flow**: commands that need elevated access call `get_sudo_password(&state)`, which returns `SUDO_PASSWORD_REQUIRED` if none is cached. The frontend catches this error code and opens `SudoModal`. The validated password is stored in `AppState` and expires after 900 seconds.
- **Input validation**: every Tauri command that accepts user-derived strings (hostname, username, service name, container ID, etc.) has a dedicated `validate_*` function. All sudo-injected commands use `shell_single_quote()` from `du_size.rs` to prevent injection.

### Key patterns

- **Error codes**: the backend returns structured `AppError` with a code string (e.g., `SUDO_PASSWORD_REQUIRED`, `NO_SSH_CONNECTION`). The frontend uses `formatInvokeError()` from `src/lib/i18n/backendErrors.ts` to turn them into localized messages.
- **Profile credentials**: SSH passwords and key passphrases are stored in the OS keyring (service `"JarvisServerManager"`), not in `profiles.json`.
- **Closing the window** hides to system tray rather than quitting. Tray left-click toggles visibility; tray menu has Show/Quit.
- **Streaming commands**: `exec_custom_command_stream` emits output line-by-line via a Tauri event identified by `event_id` (used for Docker compose pull, log tailing, etc.).

write code, write comments, generate output files, print logs, and author artifacts in English at all times.