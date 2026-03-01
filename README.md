<p align="center">
  <img src="design/icon-original.png" alt="Claude Usage" width="128" height="128" />
</p>

<h1 align="center">Claude Usage</h1>

<p align="center">
  A sleek, always-on-top desktop widget that monitors your real-time API usage for <strong>Cursor</strong> and <strong>Claude</strong>.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Tauri-2.0-blue" alt="Tauri 2.0" />
  <img src="https://img.shields.io/badge/React-19-61DAFB" alt="React 19" />
  <img src="https://img.shields.io/badge/Rust-orange" alt="Rust" />
  <img src="https://img.shields.io/badge/TypeScript-blue" alt="TypeScript" />
</p>

---

> [!WARNING]
> This project was **vibed into existence**. The code was largely generated through AI-assisted development. Expect rough edges, unconventional patterns, and the occasional "it works, don't touch it" moment. Use at your own risk.

## Features

- Real-time usage monitoring for Cursor IDE and Claude AI
- Animated progress bars with color-coded usage levels
- Support for multiple billing modes (Plan usage + On-demand for Cursor)
- Separate session (5h) and weekly (7d) tracking for Claude
- Toggle between "usage" and "remaining" display modes
- Configurable refresh intervals
- Glassmorphic UI with shimmer and pulse animations
- Tiny, frameless, always-on-top window

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

| Layer    | Technology                  |
| -------- | --------------------------- |
| Frontend | React 19, TypeScript, Vite  |
| Backend  | Rust, Tauri 2               |
| Styling  | Custom CSS with animations  |

## Debug Commands

You can run diagnostic commands from the browser DevTools console while the app is running in development mode (`pnpm tauri dev`):

```js
// Show token info (expiry, refresh status, rate limit tier)
window.__TAURI_INTERNALS__.invoke('debug_token_info').then(console.log)

// Force refresh the OAuth token
window.__TAURI_INTERNALS__.invoke('force_refresh_token').then(console.log)
```

## License

MIT
