# FlashBarRust

A lightweight, always-at-hand search bar for quickly finding and launching installed applications. Built with Rust and the Slint UI framework.

## Features

- Simple floating search window
- Fast search across installed applications
- Cross-platform: Windows 11 and Ubuntu Linux

## Tech Stack

- **Language:** Rust
- **UI:** [Slint](https://slint.dev/)

## Architecture

The project follows clean architecture, separated into layers:

- **models**: core data types (e.g. application entries)
- **repository**: access contracts for application data
- **service**: search and launch logic
- **infrastructure**: platform-specific implementations (Windows and Linux) and the Slint UI integration

## Project Structure

```
FlashBarRust/
├── src/        # Application source code
└── README.md
```

## Getting Started

### Prerequisites

- [Rust toolchain](https://www.rust-lang.org/tools/install) (stable)
- Windows 11 or Ubuntu Linux

### Build and Run

```bash
cargo build
cargo run
```

## Status

This project is in early development.

## License

To be determined.
