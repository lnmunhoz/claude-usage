<p align="center">
  <img src="design/icon-original.png" alt="Token Juice" width="128" height="128" />
</p>

<h1 align="center">Token Juice</h1>

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
git clone https://github.com/lnmunhoz/token-juice.git
cd token-juice

# Install dependencies
pnpm install

# Run in development mode
pnpm tauri dev
```

## Build for Production

```bash
pnpm tauri build
```

The bundled app will be available in `src-tauri/target/release/bundle/`.

## Tech Stack

| Layer    | Technology                  |
| -------- | --------------------------- |
| Frontend | React 19, TypeScript, Vite  |
| Backend  | Rust, Tauri 2               |
| Styling  | Custom CSS with animations  |

## License

MIT
