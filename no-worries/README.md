# no-worries

A custom panic handler for Rust applications that provides enhanced error reporting and graceful failure handling.

## Quick Start

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
