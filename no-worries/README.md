# no-worries

A custom panic handler crate for Rust applications that provides enhanced error reporting and graceful failure handling.

## Features

- 🎨 **Colored Output**: Beautiful, readable panic messages with color coding
- ⏰ **Timestamps**: Optional timestamp inclusion in panic logs
- 📋 **JSON Format**: Machine-readable JSON output for logging systems
- 🔧 **Configurable**: Flexible configuration options via builder pattern
- 🚀 **Lightweight**: Minimal dependencies with optional features

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
no-worries = { path = "../no-worries" }
```

Basic usage:

```rust
use no_worries::set_panic_handler;

fn main() {
    set_panic_handler();

    // Your application code here
    panic!("Something went wrong!");
}
```

## Advanced Configuration

Use the builder pattern for custom configuration:

```rust
use no_worries::PanicHandlerBuilder;

fn main() {
    PanicHandlerBuilder::new()
        .with_app_name("my-awesome-app")
        .with_colors(true)
        .with_timestamp(true)
        .with_backtrace_hint(true)
        .install();

    // Your application code here
}
```

## Features

### Default Features
- `std`: Standard library support
- `colored-output`: Colored panic messages

### Optional Features
- `json-output`: JSON-formatted output (requires `serde` and `serde_json`)
- `timestamp`: Timestamp support (requires `chrono`)

Enable features in your `Cargo.toml`:

```toml
[dependencies]
no-worries = { path = "../no-worries", features = ["json-output", "timestamp"] }
```

## Output Examples

### Text Format (default)
```
💥 my-app encountered a panic!
🕐 2025-05-26 10:30:45 UTC
Message: Something went wrong!
📍 src/main.rs:15:5
💡 Run with `RUST_BACKTRACE=1` for a backtrace
```

### JSON Format
```json
{
  "type": "panic",
  "message": "Something went wrong!",
  "location": {
    "file": "src/main.rs",
    "line": 15,
    "column": 5
  },
  "timestamp": "2025-05-26T10:30:45.123Z",
  "app_name": "my-app"
}
```

## License

This project follows the same license as the parent workspace.
