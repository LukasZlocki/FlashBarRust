# Plan: Frameless App Search Window

## Current State

The project is still a skeleton:

- `src/main.rs` prints "Hello, world!".
- The layer modules are empty.
- There is no `.slint` file and no `build.rs`.

Nothing in this plan has been implemented yet.

## 1. Build Setup

- `Cargo.toml`: add `slint-build` as a build-dependency.
- Add `build.rs` to compile the Slint UI file.
- Add `ui/app.slint` for the window and its components.

## 2. Window and UI (infrastructure layer)

File: `src/infrastructure/ui/mod.rs`. Wires the Slint window to the service layer.

- Window: `no-frame: true`, transparent background, fixed size (about 600 x 220 px). The fixed height leaves room for 5 result rows.
- Root rectangle: black fill with a `border-radius` of about 24 px.
- Search field: a `TextInput` with a "search" placeholder overlay, focused on startup.
- Results list: up to 5 rows, each with an icon box and a name field.
- ESC: a key handler that calls `hide()`. The plan assumes ESC exits the process, since there is no tray or hotkey. Verify that ESC still works while the text field has focus.
- Centering: compute the position from the screen size and set it on startup. Slint 1.x has no stable monitor-size API, so this may require the winit backend.

## 3. Models (no dependencies)

File: `src/models/mod.rs`.

- `AppEntry { name, launch_command, icon_path: Option<PathBuf> }`

## 4. Repository (traits only)

File: `src/repository/mod.rs`.

- `AppRepository` trait with `all_apps() -> Result<Vec<AppEntry>, RepositoryError>`.
- Launching is a platform concern and can get its own trait later. It is not needed for this request.

## 5. Service (search logic, no UI)

File: `src/service/mod.rs`.

- `search(apps, query) -> Vec<AppEntry>`
- Rules:
  - Trim the query and compare case-insensitively.
  - An empty query returns no results.
  - Prefix matches rank above substring matches. Ties sort alphabetically.
  - Results are capped at `MAX_RESULTS = 5`.
- Unit tests with a fake repository. These run on both platforms.

## 6. Platform Discovery (infrastructure)

- `src/infrastructure/linux/mod.rs`
  - Read `.desktop` files from `/usr/share/applications` and `~/.local/share/applications`.
  - Use `Name`, `Icon`, `Exec`, and `Type=Application`. Skip entries with `NoDisplay=true`.
  - Resolve icons through the hicolor theme and `/usr/share/pixmaps`.
- `src/infrastructure/windows/mod.rs`
  - Scan the Start Menu folders for `.lnk` shortcuts.
  - Icon extraction needs the Win32 API via the `windows` crate. This is the riskiest part.
- Both implement `AppRepository`, so the service and UI do not change per platform.

## 7. Wiring

File: `src/main.rs`.

- Load apps once at startup.
- Create the platform repository and build the window.
- Connect the text-changed callback to `service::search`.
- Pass results to a Slint model.

## Dependencies to Approve

- `slint-build` (required).
- Linux: `freedesktop-desktop-entry` or `freedesktop-icons`, or a hand-written parser. Icon theme lookup is the hardest part to write from scratch.
- Windows: `windows` crate, and possibly `lnk`.

## Phases

1. Frameless, black, rounded, centered window. ESC closes it.
2. Search field, service search with tests, result list with names and fallback icons.
3. Linux app discovery and real icons.
4. Windows app discovery and icons.

## Verification

- Run `cargo build` and `cargo test` after each phase.
- Visual check with `cargo run` only when requested.
- Windows code cannot be built or run on Linux. Mark it as unverified.

## Risks

- Rounded corners need a transparent window. On Linux this depends on a compositor. Without one, the corners show as a black square.
- Ubuntu uses Wayland by default. Wayland ignores client-set window positions, so "always centered" may only hold on X11 or depend on the compositor.

## Open Questions

1. ESC: exit the process, or only hide the window?
2. Should Enter or click launch the selected app now, or in a later step?
3. Empty search field: show nothing, or show recent apps?
4. Windows icons: include them in the first version, or start with a fallback icon?
5. Is compositor-dependent centering on Wayland acceptable?
