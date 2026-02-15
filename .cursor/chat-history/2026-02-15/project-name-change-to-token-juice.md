# Project name change to token-juice

**Date**: 2026-02-15

---

### **User**

Okay, please take a look at the project, we need to rename it to token-juice instead of cursor-juice because now we support more than one provider

### **Assistant** `claude-4.6-opus-high-thinking`

[Thinking] The user wants to rename the project from "cursor-juice" to "token-juice". Let me explore the codebase to understand the full scope of changes needed.
Let me explore the project to understand the full scope of the rename.
[Tool: Task]
  subagent_type: explore
  description: Explore project structure and references
  prompt: Explore the project at /Users/lnmunhoz/projects/cursor-juice thoroughly. I need to find ALL references to "cursor-juice" and "cursor juice" and "Cursor Juice" (case-insensitive) across the entire codebase. 

Please:
1. List the full project directory structure
2. Search for all occurrences of "cursor-juice", "cursor juice", "Cursor Juice", "CursorJuice", "cursorJuice" in all files
3. Read package.json to understand the project name and scripts
4. Read any config files that reference the project name (vite.config, tsconfig, etc.)
5. Read the README if it exists
6. Read the main App.tsx and any component files that display the project name in the UI
7. Check the HTML files (index.html) for title/meta tags

Return a comprehensive list of every file and line that contains a reference to "cursor" in the context of the project name (not references to cursor as a CSS property or text cursor).
  model: fast

### **Assistant** `claude-4.6-opus-high-thinking`

Now let me also check the App.tsx for any UI references to the project name, and the assets folder.
[Tool: Read]
  path: /Users/lnmunhoz/projects/cursor-juice/src/App.tsx
  offset: 1
  limit: 30
[Tool: Read]
  path: /Users/lnmunhoz/projects/cursor-juice/src/assets/cursor-logo.svg
[Tool: Grep]
  pattern: cursor-juice|Cursor Juice|CursorJuice|cursorJuice|cursor_juice
  -i: false
  path: /Users/lnmunhoz/projects/cursor-juice/src

### **Assistant** `claude-4.6-opus-high-thinking`

[Tool: Grep]
  pattern: Cursor Juice|cursor.juice|cursorJuice
  -i: true
  path: /Users/lnmunhoz/projects/cursor-juice/src
[Tool: Read]
  path: /Users/lnmunhoz/projects/cursor-juice/src-tauri/tauri.conf.json
[Tool: Read]
  path: /Users/lnmunhoz/projects/cursor-juice/src-tauri/Cargo.toml

### **Assistant** `claude-4.6-opus-high-thinking`

[Tool: Read]
  path: /Users/lnmunhoz/projects/cursor-juice/package.json
[Tool: Read]
  path: /Users/lnmunhoz/projects/cursor-juice/index.html
[Tool: Read]
  path: /Users/lnmunhoz/projects/cursor-juice/src-tauri/src/main.rs

### **Assistant** `claude-4.6-opus-high-thinking`

Now let me check the log prefixes in `lib.rs` to understand the pattern used there.
[Tool: Grep]
  pattern: \[cursor-juice\]|cursor.juice|Cursor Juice|cursor_juice
  path: /Users/lnmunhoz/projects/cursor-juice/src-tauri/src/lib.rs
  output_mode: count
[Tool: Read]
  path: /Users/lnmunhoz/projects/cursor-juice/src-tauri/src/lib.rs
  offset: 1200
  limit: 70
[Tool: Read]
  path: /Users/lnmunhoz/projects/cursor-juice/src-tauri/src/lib.rs
  offset: 170
  limit: 10

### **Assistant** `claude-4.6-opus-high-thinking`

I now have a comprehensive understanding of all the changes needed. Let me create the plan.
[Tool: CreatePlan]
  name: Rename to Token Juice
  overview: Rename the project from "cursor-juice" to "token-juice" across all configuration files, Rust source code, and HTML -- covering package names, display names, log prefixes, config directory paths, identifiers, and Cargo crate names.
  plan: # Rename Project from "Cursor Juice" to "Token Juice"

## Scope

57 occurrences in `lib.rs` plus references in 6 other files. The frontend `src/` directory has **no** text references to the project name (only an SVG asset import for the Cursor logo, which stays since Cursor is still a supported provider).

## Files to Change

### 1. [package.json](package.json)
- Line 2: `"name": "cursor-juice"` -> `"name": "token-juice"`

### 2. [index.html](index.html)
- Line 6: `<title>Cursor Juice</title>` -> `<title>Token Juice</title>`

### 3. [src-tauri/tauri.conf.json](src-tauri/tauri.conf.json)
- Line 3: `"productName": "Cursor Juice"` -> `"productName": "Token Juice"`
- Line 5: `"identifier": "com.cursorjuice.app"` -> `"identifier": "com.tokenjuice.app"`
- Line 17: `"title": "Cursor Juice"` -> `"title": "Token Juice"`

### 4. [src-tauri/Cargo.toml](src-tauri/Cargo.toml)
- Line 2: `name = "cursor-juice"` -> `name = "token-juice"`
- Line 14: `name = "cursor_juice_lib"` -> `name = "token_juice_lib"`

### 5. [src-tauri/src/main.rs](src-tauri/src/main.rs)
- Line 5: `cursor_juice_lib::run()` -> `token_juice_lib::run()`

### 6. [src-tauri/src/lib.rs](src-tauri/src/lib.rs) (57 occurrences)
- **Config directory path** (line 173): `.join("cursor-juice")` -> `.join("token-juice")`
- **All `[cursor-juice]` log prefixes** (~53 occurrences): `[cursor-juice]` -> `[token-juice]`
- **Menu items** (lines 1211-1214):
  - `"About Cursor Juice"` -> `"About Token Juice"`
  - `"Quit Cursor Juice"` -> `"Quit Token Juice"`
  - `SubmenuBuilder::new(app, "Cursor Juice")` -> `SubmenuBuilder::new(app, "Token Juice")`

### 7. [src-tauri/Cargo.lock](src-tauri/Cargo.lock) (auto-generated)
- This will be regenerated automatically on next build. No manual edit needed.

## What is NOT changing
- `src/assets/cursor-logo.svg` -- this is the Cursor provider logo, not the app name
- `src/assets/claude-logo.svg` -- provider logo
- Any CSS cursor properties
- The `.cursor/` directory (IDE config, not project-related)

## Migration note
Users who have existing settings stored at `~/.config/cursor-juice/settings.json` will lose them after the rename (new path: `~/.config/token-juice/settings.json`). This is acceptable for a pre-1.0 app.

  todos: [{"id":"rename-package-json","content":"Rename in package.json (name field)"},{"id":"rename-index-html","content":"Rename in index.html (title tag)"},{"id":"rename-tauri-conf","content":"Rename in tauri.conf.json (productName, identifier, title)"},{"id":"rename-cargo-toml","content":"Rename in Cargo.toml (package name, lib name)"},{"id":"rename-main-rs","content":"Rename in main.rs (lib crate reference)"},{"id":"rename-lib-rs","content":"Rename all occurrences in lib.rs (config path, log prefixes, menu items)"}]

### **User**

1|# Rename Project from "Cursor Juice" to "Token Juice"
     2|
     3|## Scope
     4|
     5|57 occurrences in `lib.rs` plus references in 6 other files. The frontend `src/` directory has **no** text references to the project name (only an SVG asset import for the Cursor logo, which stays since Cursor is still a supported provider).
     6|
     7|## Files to Change
     8|
     9|### 1. [package.json](package.json)
    10|
    11|- Line 2: `"name": "cursor-juice"` -> `"name": "token-juice"`
    12|
    13|### 2. [index.html](index.html)
    14|
    15|- Line 6: `<title>Cursor Juice</title>` -> `<title>Token Juice</title>`
    16|
    17|### 3. [src-tauri/tauri.conf.json](src-tauri/tauri.conf.json)
    18|
    19|- Line 3: `"productName": "Cursor Juice"` -> `"productName": "Token Juice"`
    20|- Line 5: `"identifier": "com.cursorjuice.app"` -> `"identifier": "com.tokenjuice.app"`
    21|- Line 17: `"title": "Cursor Juice"` -> `"title": "Token Juice"`
    22|
    23|### 4. [src-tauri/Cargo.toml](src-tauri/Cargo.toml)
    24|
    25|- Line 2: `name = "cursor-juice"` -> `name = "token-juice"`
    26|- Line 14: `name = "cursor_juice_lib"` -> `name = "token_juice_lib"`
    27|
    28|### 5. [src-tauri/src/main.rs](src-tauri/src/main.rs)
    29|
    30|- Line 5: `cursor_juice_lib::run()` -> `token_juice_lib::run()`
    31|
    32|### 6. [src-tauri/src/lib.rs](src-tauri/src/lib.rs) (57 occurrences)
    33|
    34|- **Config directory path** (line 173): `.join("cursor-juice")` -> `.join("token-juice")`
    35|- **All `[cursor-juice]` log prefixes** (~53 occurrences): `[cursor-juice]` -> `[token-juice]`
    36|- **Menu items** (lines 1211-1214):
    37|  - `"About Cursor Juice"` -> `"About Token Juice"`
    38|  - `"Quit Cursor Juice"` -> `"Quit Token Juice"`
    39|  - `SubmenuBuilder::new(app, "Cursor Juice")` -> `SubmenuBuilder::new(app, "Token Juice")`
    40|
    41|### 7. [src-tauri/Cargo.lock](src-tauri/Cargo.lock) (auto-generated)
    42|
    43|- This will be regenerated automatically on next build. No manual edit needed.
    44|
    45|## What is NOT changing
    46|
    47|- `src/assets/cursor-logo.svg` -- this is the Cursor provider logo, not the app name
    48|- `src/assets/claude-logo.svg` -- provider logo
    49|- Any CSS cursor properties
    50|- The `.cursor/` directory (IDE config, not project-related)
    51|
    52|## Migration note
    53|
    54|Users who have existing settings stored at `~/.config/cursor-juice/settings.json` will lose them after the rename (new path: `~/.config/token-juice/settings.json`). This is acceptable for a pre-1.0 app.
    55|




Implement the plan as specified, it is attached for your reference. Do NOT edit the plan file itself.

To-do's from the plan have already been created. Do not create them again. Mark them as in_progress as you work, starting with the first one. Don't stop until you have completed all the to-dos.

### **Assistant** `claude-4.6-opus-high-thinking`

[Thinking] Let me implement the rename plan step by step. I'll start with the simpler config files and work my way to the large lib.rs file.
Let me start implementing the rename. I'll begin with the config files in parallel.
[Tool: TodoWrite]
  todos: [{"id":"rename-package-json","content":"Rename in package.json (name field)","status":"in_progress"},{"id":"rename-index-html","content":"Rename in index.html (title tag)","status":"pending"},{"id":"rename-tauri-conf","content":"Rename in tauri.conf.json (productName, identifier, title)","status":"pending"},{"id":"rename-cargo-toml","content":"Rename in Cargo.toml (package name, lib name)","status":"pending"},{"id":"rename-main-rs","content":"Rename in main.rs (lib crate reference)","status":"pending"},{"id":"rename-lib-rs","content":"Rename all occurrences in lib.rs (config path, log prefixes, menu items)","status":"pending"}]
  merge: true

### **Assistant** `claude-4.6-opus-high-thinking`

[Tool: StrReplace]
  path: /Users/lnmunhoz/projects/cursor-juice/package.json
  old_string: "name": "cursor-juice",
  new_string: "name": "token-juice",
[Tool: StrReplace]
  path: /Users/lnmunhoz/projects/cursor-juice/index.html
  old_string: <title>Cursor Juice</title>
  new_string: <title>Token Juice</title>
[Tool: StrReplace]
  path: /Users/lnmunhoz/projects/cursor-juice/src-tauri/tauri.conf.json
  old_string: "productName": "Cursor Juice",
  new_string: "productName": "Token Juice",
[Tool: StrReplace]
  path: /Users/lnmunhoz/projects/cursor-juice/src-tauri/src/main.rs
  old_string: cursor_juice_lib::run()
  new_string: token_juice_lib::run()

### **Assistant** `claude-4.6-opus-high-thinking`

Now the remaining tauri.conf.json changes and Cargo.toml.
[Tool: TodoWrite]
  todos: [{"id":"rename-package-json","content":"Rename in package.json (name field)","status":"completed"},{"id":"rename-index-html","content":"Rename in index.html (title tag)","status":"completed"},{"id":"rename-main-rs","content":"Rename in main.rs (lib crate reference)","status":"completed"},{"id":"rename-tauri-conf","content":"Rename in tauri.conf.json (productName, identifier, title)","status":"in_progress"}]
  merge: true
[Tool: StrReplace]
  path: /Users/lnmunhoz/projects/cursor-juice/src-tauri/tauri.conf.json
  old_string: "identifier": "com.cursorjuice.app",
  new_string: "identifier": "com.tokenjuice.app",
[Tool: StrReplace]
  path: /Users/lnmunhoz/projects/cursor-juice/src-tauri/tauri.conf.json
  old_string: "title": "Cursor Juice",
  new_string: "title": "Token Juice",
[Tool: StrReplace]
  path: /Users/lnmunhoz/projects/cursor-juice/src-tauri/Cargo.toml
  old_string: name = "cursor-juice"
  new_string: name = "token-juice"
[Tool: StrReplace]
  path: /Users/lnmunhoz/projects/cursor-juice/src-tauri/Cargo.toml
  old_string: name = "cursor_juice_lib"
  new_string: name = "token_juice_lib"

### **Assistant** `claude-4.6-opus-high-thinking`

Good, now the big one -- `lib.rs`. I'll use `replace_all` for the log prefix and then handle the unique strings individually.
[Tool: TodoWrite]
  todos: [{"id":"rename-tauri-conf","content":"Rename in tauri.conf.json (productName, identifier, title)","status":"completed"},{"id":"rename-cargo-toml","content":"Rename in Cargo.toml (package name, lib name)","status":"completed"},{"id":"rename-lib-rs","content":"Rename all occurrences in lib.rs (config path, log prefixes, menu items)","status":"in_progress"}]
  merge: true
[Tool: StrReplace]
  path: /Users/lnmunhoz/projects/cursor-juice/src-tauri/src/lib.rs
  old_string: [cursor-juice]
  new_string: [token-juice]
  replace_all: true

### **Assistant** `claude-4.6-opus-high-thinking`

Now let me handle the remaining unique occurrences in lib.rs: the config directory path and the menu items.
[Tool: StrReplace]
  path: /Users/lnmunhoz/projects/cursor-juice/src-tauri/src/lib.rs
  old_string: .join("cursor-juice");
  new_string: .join("token-juice");
[Tool: StrReplace]
  path: /Users/lnmunhoz/projects/cursor-juice/src-tauri/src/lib.rs
  old_string: "About Cursor Juice"
  new_string: "About Token Juice"
[Tool: StrReplace]
  path: /Users/lnmunhoz/projects/cursor-juice/src-tauri/src/lib.rs
  old_string: "Quit Cursor Juice"
  new_string: "Quit Token Juice"
[Tool: StrReplace]
  path: /Users/lnmunhoz/projects/cursor-juice/src-tauri/src/lib.rs
  old_string: SubmenuBuilder::new(app, "Cursor Juice")
  new_string: SubmenuBuilder::new(app, "Token Juice")

### **Assistant** `claude-4.6-opus-high-thinking`

Now let me verify there are no remaining references to `cursor-juice` or `Cursor Juice` anywhere in the source files.
[Tool: Grep]
  pattern: cursor.juice|Cursor Juice|cursor_juice|CursorJuice|cursorjuice
  -i: false
  path: /Users/lnmunhoz/projects/cursor-juice/src-tauri/src
[Tool: Grep]
  pattern: cursor.juice|Cursor Juice|cursor_juice|CursorJuice|cursorjuice
  -i: false
  path: /Users/lnmunhoz/projects/cursor-juice/package.json
[Tool: Grep]
  pattern: cursor.juice|Cursor Juice|cursor_juice|CursorJuice|cursorjuice
  -i: false
  path: /Users/lnmunhoz/projects/cursor-juice/index.html
[Tool: Grep]
  pattern: cursor.juice|Cursor Juice|cursor_juice|CursorJuice|cursorjuice
  -i: false
  path: /Users/lnmunhoz/projects/cursor-juice/src-tauri/tauri.conf.json
[Tool: Grep]
  pattern: cursor.juice|Cursor Juice|cursor_juice|CursorJuice|cursorjuice
  -i: false
  path: /Users/lnmunhoz/projects/cursor-juice/src-tauri/Cargo.toml

### **Assistant** `claude-4.6-opus-high-thinking`

All clean -- zero remaining references. Let me mark the final todo as complete.
[Tool: TodoWrite]
  todos: [{"id":"rename-lib-rs","content":"Rename all occurrences in lib.rs (config path, log prefixes, menu items)","status":"completed"}]
  merge: true

### **Assistant** `claude-4.6-opus-high-thinking`

All done. Here's a summary of everything that was renamed:

**Config files:**
- `package.json` -- name field changed to `token-juice`
- `index.html` -- page title changed to "Token Juice"
- `src-tauri/tauri.conf.json` -- product name, window title, and app identifier all updated (`com.tokenjuice.app`)
- `src-tauri/Cargo.toml` -- crate name to `token-juice`, lib name to `token_juice_lib`

**Rust source:**
- `src-tauri/src/main.rs` -- updated crate reference to `token_juice_lib::run()`
- `src-tauri/src/lib.rs` -- all 57 occurrences updated: log prefixes (`[token-juice]`), config directory path (`.join("token-juice")`), and menu items ("About Token Juice", "Quit Token Juice", submenu label)

The `Cargo.lock` will regenerate automatically on the next build. Note that the actual workspace directory is still named `cursor-juice` on disk -- you can rename that at the filesystem level whenever you're ready.

