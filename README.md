# How to Setup and Run (Linux/WSL)

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

# How to Setup and Run (Windows)

Install **Visual Studio C++ Build Tools**. If you do not already have Visual Studio installed, go to the official [Visual Studio downloads page](https://visualstudio.microsoft.com/downloads/) and install the latest version of **Build Tools for Visual Studio**. If you already have Visual Studio installed, modify your current installation and ensure the latest MSVC version is installed:

<img width="1886" height="1042" alt="image" src="https://github.com/user-attachments/assets/1d0e55ac-12af-4c08-8538-3d0039e68f0b" />

Install `rustup` using either `winget` or by running the [official installer](https://rust-lang.org/tools/install/). You will likely get a "Bad Image" error but restarting your computer should fix this.
```
winget install Rustlang.Rustup
```

Check your version of `rustc` to confirm that rust has been successfully installed:
```
rustc --version
```

Build and run the application:
```
cargo run
```

