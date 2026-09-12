# How to Setup and Run

After cloning the repository, install `rustup`. It will automatically handle downloading `rustc` (the actual rust compiler), `cargo` (the package manager and build tool), and `rust-std` (the rust standard library):
```
sudo apt install rustup
```

Install the latest stable version of rust:
```
rustup default stable
```

Build and run the application:
```
cargo run
```

> [!NOTE]
> If an "Error opening certificate file" is thrown, open [default_config.toml](https://github.com/MissouriMRDT/basestation_camera_server/blob/057fd4cb353d9ca170862ff8ef10d68185c44878/src/default_config.toml) and set `webssocket_address` to an empty string. You will then need to run `cargo clean` before calling `cargo run` again so rust rebuilds everything.
