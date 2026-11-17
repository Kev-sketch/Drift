# Building Drift (native Rust shell)

Pure Rust. System WebKit. No Tauri, no wry, no tao, no bundled engine, no JS runtime.

The whole point, from `../Drift/docs/ARCHITECTURE.md`: the page engine is macOS's own WKWebView
in every candidate stack, so the only thing a stack still controls is what it wraps around it.
Drift wraps it in one process and an FFI call. The last section shows what that costs on disk.

## Requirements

- macOS 13 or later, x86_64 or arm64.
- Rust stable (built and measured with `rustc 1.99.1`).
- Command line tools only. **Xcode is not needed and was not installed** on the machine this was
  built on (`/Library/Developer/CommandLineTools`, SDK `MacOSX13.3.sdk`). No `SDKROOT` override,
  no `xcrun` shim, no `xcodebuild`.

Frameworks are linked by the `objc2-*` crates through the standard SDK search paths, so the only
toolchain setting that matters is that `rustc` can see the CLT SDK, which it does by default.

## Build

```bash
cd DriftRust
cargo build            # debug
cargo build --release  # optimised, stripped, LTO, opt-level=s
```

First build compiles the `objc2` family and takes a while (see timings below). Later builds only
recompile this crate.

## Run

```bash
./target/debug/drift
# or
cargo run --release
```

A window opens with an address bar and `https://example.com` loading. Type a host and press
Enter (`example.com`, `https://example.com`, `localhost:8080` all work); the bar normalises a
bare host to `https`.

Expected stdout, this is the render proof (captured from the run below):

```
drift: window up, loading https://example.com
drift: loaded title=Example Domain
drift: url=https://example.com/
```

The window title follows the page title through `WKNavigationDelegate`, and the address bar is
rewritten from the page's own URL after each finished navigation.

## Screenshot proof

```bash
./target/debug/drift &          # leave it running
sleep 8
screencapture -x /tmp/drift.png # full screen; the app window is centred
```

Taken on this machine while writing this file: the window shows
`https://example.com/` in the address bar, the page body of Example Domain rendered by WebKit,
and `Example Domain` in the title bar, the title having come through
`webView:didFinishNavigation:`. Menu bar says `drift`. Without a bundle there is no app icon or
menu, which is expected for a bare executable.

## One real macOS 13 gotcha, found by running it

`NSApplication::activate()` (no arguments) is bound in `objc2-app-kit` and compiles fine, but on
macOS 13 it does not exist and the process dies at startup:

```
thread 'main' panicked at objc2-app-kit-0.3.2/src/generated/NSApplication.rs:485:
invalid message send to -[NSApplication activate]: method not found
```

It is a macOS 14+ selector. The shell uses the deprecated but present
`activateIgnoringOtherApps:` instead. This is exactly the trap in
`../Drift/docs/ARCHITECTURE.md` section 6: the bindings compile for every version, so a binding
that links is not proof the class or selector exists on the target OS.

## What is in here

| File | What it does |
|---|---|
| `Cargo.toml` | five crates, all `objc2`; release profile tuned for size (`lto`, `opt-level="s"`, `codegen-units=1`, `strip`, `panic="abort"`) |
| `src/main.rs` | `NSApplication` (activation policy Regular so a bare binary gets a window and focus), `NSWindow` with a title bar, `NSTextField` address bar, one `WKWebView`, one `define_class!` delegate implementing `WKNavigationDelegate` plus the address bar's action |

Deliberately absent: `tao` (its windowing abstraction is what `objc2-app-kit` already gives us),
`wry` (its macOS layer returns its own `WryWebView` subclass that overrides key handling, drag
and mouse events a browser must own, and exposes no content-rule-list API at all), and `tauri`
(an app shell for web apps; a browser needs the opposite of an injected JS bridge).

## Timings and size (measured on this machine)

Hardware: 13" MacBook Pro, Intel i7-1068NG7, 8 threads, 16 GB, fanless, power-capped. Another
cargo build was competing for the same 8 threads during part of the measurement, so treat the
cold number as an upper bound.

| Step | Result |
|---|---|
| cold `cargo build`, all 20 packages from scratch | 57 min wall, including one interrupted-and-resumed run. `objc2-app-kit` alone was ~20 min of it; two other cargo builds shared the same 8 threads |
| incremental `cargo build` (this crate only) | 0.9 s to 1.3 s |
| `cargo build --release` | **not run**, deliberately: it recompiles every `objc2` dependency in the release profile, roughly another 40+ minutes on this machine. The profile is configured in `Cargo.toml` (`lto`, `opt-level="s"`, `codegen-units=1`, `strip`, `panic="abort"`) so a number is one command away, but no number is quoted that was not measured |
| debug binary | 1,482,456 bytes (1.4 MB), 347 dependency artifacts, `target/` 810 MB |
| original Swift app, for scale | 5.4 MB per architecture, 10.9 MB universal (`../Drift/build.sh`) |

| Runtime, this page | Result |
|---|---|
| app process, debug build, example.com loaded | 51.6 MB RSS |
| its WebContent process | 29.1 MB RSS |
| its Networking process | 14.3 MB RSS |

## Verified on

macOS 13.7.8 (build 22H730), Darwin 22.6.0, x86_64, Apple clang 14.0.3, SDK 13.3, `rustc 1.99.1`,
CommandLineTools only, no Xcode. The page rendered in WebKit 18615.3.12, the Safari 16.6 engine
this OS ships. `cargo build` exits 0 and the binary renders `https://example.com` with a working
address bar; screenshot and stdout above are from that run.

## What this shell does not do yet

Everything below is app code, not a toolchain problem, and each one is a separate task: tabs as a
model with one webview per visible tab, `WKContentRuleList` blocking, session restore without
instantiating webviews, downloads, find in page, permissions prompts, and a menu bar. They are
listed so nobody mistakes "compiles and renders" for "browser".
