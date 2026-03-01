<h1 align="center">Claude Usage</h1>

<p align="center">
  A lightweight macOS menu bar app that monitors your Claude AI usage in real time.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Tauri-2.0-blue" alt="Tauri 2.0" />
  <img src="https://img.shields.io/badge/React-19-61DAFB" alt="React 19" />
  <img src="https://img.shields.io/badge/Rust-orange" alt="Rust" />
  <img src="https://img.shields.io/badge/TypeScript-blue" alt="TypeScript" />
</p>

---

## Features

- Session (5h) and weekly (7d) usage tracking via the Claude OAuth API
- Animated progress bars with color-coded usage levels
- OAuth PKCE login with automatic token refresh
- Plan detection (Pro, Max 5X/20X, Team, Enterprise)
- Toggle between "usage" and "remaining" display modes
- Configurable refresh intervals
- Menu bar tray icon with live usage percentage

## Prerequisites

- [Node.js](https://nodejs.org/) (LTS recommended)
- [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/tools/install)
- Tauri 2 system dependencies ([see guide](https://v2.tauri.app/start/prerequisites/))

## Install & Run

```bash
# Clone the repo
git clone https://github.com/lnmunhoz/claude-usage.git
cd claude-usage

# Install dependencies
pnpm install

# Run in development mode
pnpm tauri dev
```

## Build for Production

### macOS (Apple Silicon)

```bash
pnpm build:mac
```

The output will be in `src-tauri/target/aarch64-apple-darwin/release/bundle/` and includes:

- `.dmg` installer
- `.app` bundle

### Linux (x86_64)

```bash
pnpm build:linux
```

> Requires the `x86_64-unknown-linux-gnu` Rust target. Install it with:
>
> ```bash
> rustup target add x86_64-unknown-linux-gnu
> ```

The output will be in `src-tauri/target/x86_64-unknown-linux-gnu/release/bundle/` and includes:

- `.deb` package
- `.AppImage`

### Generic build

```bash
pnpm tauri build
```

Builds for your current platform and architecture. The output will be in `src-tauri/target/release/bundle/`.

## Tech Stack

| Layer    | Technology                 |
| -------- | -------------------------- |
| Frontend | React 19, TypeScript, Vite |
| Backend  | Rust, Tauri 2              |
| Styling  | Custom CSS with animations |

## Debug Commands

You can run diagnostic commands from the browser DevTools console while the app is running in development mode (`pnpm tauri dev`):

```js
// Show token info (expiry, refresh status, rate limit tier)
window.__TAURI_INTERNALS__.invoke("debug_token_info").then(console.log);

// Force refresh the OAuth token
window.__TAURI_INTERNALS__.invoke("force_refresh_token").then(console.log);
```

## License

MIT
