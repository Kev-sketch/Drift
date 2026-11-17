# Drift architecture: native Rust + AppKit + WKWebView

Status: decision document for the Rust rebuild. Every claim below is either a command run on
this machine or a file read in this checkout; the reproduction commands are at the end.

## 0. Decision

**NATIVE. Rust + objc2-app-kit + objc2-web-kit (WKWebView), one process, no Tauri, no Wry, no
Electron-like runtime, and no Swift rewrite.**

Reason in one line: the page engine is the same WKWebView in all three candidates, so the only
thing a stack can still change is what it adds around it, and Tauri/Wry add a layer a browser
must then fight, while Swift cannot be built at all on this machine (section 5).

Verified end of turn: the Rust route now builds, links and runs here. `objc2-app-kit 0.3.2` and
`objc2-web-kit 0.3.2` both compile against the 13.3 CLT SDK, and a binary that constructs
`WKWebViewConfiguration` plus `WKWebView` inside the AppKit runtime prints
`rust ok: webview=0x7f8185704ca0 view=0x7f8185704ca0` and exits 0 on macOS 13.7.8. Nothing about
this stack is speculative any more.

Two things the user's frame forces me to say up front, because they contradict "no compromise
and strictly less memory":

1. On this OS the original's headline feature (Chrome extensions via `WKWebExtension`) is
   unreachable at the WebKit layer, not the language layer. macOS 15.4+ only. No Rust binding,
   no Tauri version, and no amount of rewriting fixes that here. See section 6.
2. Memory is **not** where the language is. Measured below: a Swift shell and a C shell running
   the same page land within 0.4 MB of each other, because 70 to 90 % of the footprint is the
   page's own `WebContent` process. A Rust port can win the shell floor (11.0 MB measured in a
   debug build with a live `WKWebView`, against Swift's 12.6 MB, with release expected lower)
   and the binary (0.8 MB debug against 10.9 MB universal), and it can win more by policy (tab
   suspension, one live webview per visible tab). It cannot win 2x by being Rust. Section 8
   states the number.

## 1. What this machine is (verified)

| Fact | Value | How |
|---|---|---|
| macOS | 13.7.8, build 22H730 | `sw_vers` |
| Kernel | Darwin 22.6.0, x86_64 | `uname -a` |
| CPU | Intel i7-1068NG7, 8 threads, fanless 13" MBP | `sysctl machdep.cpu.brand_string` |
| RAM | 16 GiB | `sysctl hw.memsize` |
| Toolchain | Apple clang 14.0.3, Swift 5.8.1, rustc 1.99.1 | `clang --version`, `swift --version`, `rustc --version` |
| Xcode | absent. `xcode-select -p` = `/Library/Developer/CommandLineTools` | `xcodebuild -version` fails |
| SDK | `MacOSX.sdk` -> `MacOSX13.3.sdk`, `SDKSettings.plist` Version 13.3. Also 12.1, 12.3, 13.1 | `plutil -p .../SDKSettings.plist` |
| WebKit headers | 200 headers, `WebKit.tbd`, no `WKWebExtension.h` | `ls .../WebKit.framework/Headers` |
| AppKit headers | 283 headers | `ls .../AppKit.framework/Headers` |
| Installed Safari | 16.6, bundle version 18615.3.12.11.2 | `defaults read /Applications/Safari.app/Contents/Info.plist` |
| WebKit that WKWebView actually uses | 18615.3.12.11.2 (same build) | probe: `evaluateJavaScript` UA + `navigator` dump |

Consequence: the engine available to any WKWebView app on this machine is **WebKit 18615.3.12,
the Safari 16.6 engine**. That is the ceiling. It is not upgradeable without an OS update, and
every candidate stack inherits it equally.

## 2. What the fork actually is (verified from this checkout)

- 159 Swift files, 89,807 lines, under `Sources/Drift`. Largest: `Browser.swift` (8,825),
  `ExtensionShims.swift` (5,752), `Bench.swift` (5,055), `Tab.swift` (3,459).
- `Package.swift`: `swift-tools-version: 6.0`, `platforms: [.macOS(.v14)]`.
- `README.md`: "macOS 14 or later", "about 10 MB", extensions "macOS 15.4 or later".
- `build.sh`: 5.4 MB of binary per architecture, 10.9 MB universal (Apple Silicon and Intel are
  built separately for exactly that reason).
- OS gates present in the source, counted:

| Gate | Sites |
|---|---|
| `#available(macOS 15.4, *)` | 68 (plus 37 `@available(macOS 15.4, *)`) |
| `#available(macOS 15.0, *)` / `(macOS 15, *)` | 6 + 4 |
| `#available(macOS 14, *)` | 4 |
| `#available(macOS 14.4, *)` | 3 |
| `#available(macOS 13.3, *)` | 3 |
| `#available(macOS 26.4, *)` / 26 / 27 | 3 / 2 / 1 |

So the original is not a macOS 13 app with a few newer bits. It is a macOS 14 app whose
extension engine needs 15.4, i.e. **it cannot run on this machine at all**, let alone be forked
here. That single fact reorganises the project: a Rust port targeting 13.7.8 is not a
nice-to-have, it is the only route to a running Drift on this hardware.

## 3. Where the memory actually goes (measured, on this machine)

All numbers are resident set size from `ps`, processes created by my own probes, page loaded
over the network, 10 to 12 s after load.

| Probe | App RSS | WebContent | Networking | Notes |
|---|---|---|---|---|
| ObjC `NSApplication` + WKWebView, no page | 9,572 KB | none | none | shell floor |
| ObjC shell + `example.com` | 31,392 KB | 21,500 KB | 12,720 KB | +1 webview |
| ObjC shell + `en.wikipedia.org/wiki/WebKit` | 31,444 KB | 56,692 KB | shared | page content dominates |
| Swift `NSApplication` + WKWebView, no page | 12,632 KB | none | none | Swift floor is 3.0 MB higher |
| Swift shell + same Wikipedia page | 31,268 KB | (same engine) | (same) | within 0.4 MB of ObjC |
| 3 webviews, same page | 31,176 KB | 3 processes, 19,592 / 19,676 / 19,756 KB | 12,340 KB | one process per webview |
| idle CPU, app process, 5 s window | `ps -o time` 1.94 s -> 1.94 s = **0.0 %** | | | engine idles cleanly |
| binary, ObjC (AppKit + WebKit linked) | 49,968 bytes | | | dynamic, no static libs |
| binary, Swift (same functionality) | 56,456 bytes | | | Swift runtime lives in the OS |

Three facts fall out, and they decide the whole comparison:

1. **One WKWebView is one `WebContent` process.** Measured directly: 3 views produced 3 new
   `com.apple.WebKit.WebContent.xpc` processes at about 19.6 MB each, plus one new
   `com.apple.WebKit.Networking.xpc` at 12.3 MB. This matches Apple's own wording: "WebKit
   renders the content of web views in separate processes, rather than in your app's process
   space. By default, WebKit gives each web view its own process space until it reaches an
   implementation-defined process limit" ([WKProcessPool](https://developer.apple.com/documentation/webkit/wkprocesspool);
   `WKProcessPool` itself is deprecated as of macOS 12 in that same document).
2. **Page weight, not shell weight, is the budget.** A Wikipedia article costs 56.7 MB in its
   content process. The whole difference between a Swift shell and a C/ObjC shell is 3.0 MB at
   idle and 0.4 MB with a page loaded. Anyone promising a "much lighter" rewrite by changing
   language is measuring the wrong process.
3. **The writable win is the shell floor and the binary.** 9.4 MB idle shell and a 49 KB binary
   are the numbers a Rust build has to match or beat, and Rust with `objc2` does beat the Swift
   floor (section 4.1), because it does not carry the Swift runtime's per-process allocations.

## 4. The three stacks

### 4.1 Native Rust + AppKit + objc2-web-kit

Shape: one process, `NSApplication` + `NSWindow` from `objc2-app-kit`, one `WKWebView` per
visible tab from `objc2-web-kit`, Rust owning all model state (tabs, history, rules, downloads),
WKWebView owning pages. No JS-side framework, no IPC bridge, no config file. Same WKWebView the
original uses, but the layer between the app and the engine is a direct FFI call instead of a
Swift class graph plus a SwiftUI/AppKit view tree.

Costs verified by probe on this machine. Be precise about what was proven by which tool: the
WebKit capability results below came from an Obj-C probe linked against the same SDK headers
that `objc2-web-kit` binds 1:1 (same classes, same selectors), and the Rust side is now proven
through compile, link and run: `objc2-app-kit 0.3.2` plus `objc2-web-kit 0.3.2` build against
SDK 13.3 and a `WKWebView` is constructed inside the AppKit runtime on macOS 13.7.8. Debug
binary 806,888 bytes; the release figure was still linking when this was written, so quote the
debug number or measure it, do not guess it.

- `WKContentRuleListStore compileContentRuleListForIdentifier:` returns a list, and
  `WKUserContentController addContentRuleList:` accepts it (`WKUserContentController.h:133`,
  macOS 10.13+). This is the ad blocker substrate the original uses in `Shield.swift:15-195`.
- A custom scheme handler serves `drift://newtab` and navigation succeeds
  (`WKURLSchemeHandler`, in `objc2-web-kit` as `WKURLSchemeHandler`), which is how internal
  pages are served without a bundled web server.
- `nonPersistentDataStore` gives private browsing.
- `findString:withConfiguration:` returns `matchFound = YES`.
- `takeSnapshotWithConfiguration:` returns a 1200x900 `NSImage`; `createPDFWithConfiguration:`
  returns 16,401 bytes.
- `WKUserScript` injection at document start lands (`window.__drift_injected === 1`), and
  `webkit.messageHandlers.drift.postMessage` reaches a native `WKScriptMessageHandler`.
- `inspectable` exists and works (macOS 13.3+, `WKWebView.h:665`), so Web Inspector is usable.

What it costs in build time: `objc2-app-kit` 0.3.2 is a single enormous crate (every AppKit
class in one compilation unit). On this fanless 7 W-capped i7 it is the slowest dependency in
the tree by a wide margin; plan for a cold build in the tens of minutes and keep `sccache` or a
warm `target/` in the loop. This is a developer-experience cost only; nothing about it reaches
the shipped binary.

### 4.2 Tauri / Wry

Tauri is a Rust application framework: `tauri` (config, IPC, plugins, updater), `wry` (a
platform-neutral webview wrapper), `tao` (windowing, a winit fork). Its own docs describe it
this way: "Tauri uses WRY as the abstract layer responsible to determine which webview is used
(and how interactions are made)" and TAO as the window/menu library
([Tauri Architecture](https://v2.tauri.app/concept/architecture/)). On macOS the webview is the
system WKWebView, so the engine, the process model, and the per-tab memory cost are identical to
4.1. Tauri adds nothing to the engine and takes nothing away from its cost.

What it does add, verified in `wry 0.57.2` source in the local registry:

- `WebViewExtMacOS` (`wry/src/lib.rs:2493`) is the escape hatch, and it hands back
  `Retained<WryWebView>` (`lib.rs:2513`), not a plain `WKWebView`. `WryWebView` is wry's own
  `define_class!` subclass (`wry/src/wkwebview/class/wry_web_view.rs:40`) that already overrides
  `performKeyEquivalent:` (line 45), `acceptFirstMouse:` (60), `draggingEntered:`/`Updated:`/
  `performDragOperation:`/`exited:` (80-104) and `otherMouseDown:`/`otherMouseUp:` (113-118).
  A browser has to own every one of those behaviours. You would be removing wry's overrides or
  fighting them, in a subclass you do not own, in a crate you cannot patch without vendoring.
- Wry exposes a lowest-common-denominator surface: `eval`, `load_url`, `load_html`, `reload`,
  `go_back`/`go_forward`, `clear_all_browsing_data`, `print`, `zoom`, cookies, data stores,
  downloads, custom protocol, drag and drop (`wry/src/wkwebview/mod.rs`). It has **no content
  rule list API at all** (`grep -rn "ContentRuleList" wry-0.57.2/src` returns nothing) and no
  macOS find-in-page binding (the only `findString` hit is in the iOS file). So the ad blocker,
  the single most load-bearing feature for a low-memory browser, has to bypass wry and reach the
  `WKUserContentController` through `manager()` anyway.
- `tauri` 2.12.6 is also present in the local registry, so this is not a download question. It
  is a "does it help" question, and for a browser it does not: Tauri's value is a web-app-to-
  desktop shell (asset protocol, invoke bridge, plugin permissions, updater). A browser already
  owns its window, its navigation, its downloads, and its update story, and it needs the
  opposite of an injected JS bridge, because every injected byte and every extension runtime is
  memory the user asked not to spend.

Verdict: Tauri/Wry is **strictly worse for this product and strictly larger at runtime**. Not
because Tauri is bad, but because it is a layer between the app and the API a browser exists to
drive, and a browser needs that API raw. Removing a layer beats optimizing inside one.

### 4.3 Original Swift

Same engine, same process model, no layer. It would be the natural baseline, and it loses for one
reason that is not about quality: **it does not build here**. See section 5. Beyond that, the
Swift floor measured 3.0 MB above the C/ObjC floor at idle (12,632 KB vs 9,572 KB), which is the
only place Rust has a structural advantage over it.

## 5. What is buildable on macOS 13.7.8 with CLT only

Proven by execution, not by inspection:

| Route | Result | Evidence |
|---|---|---|
| clang + Obj-C + AppKit + WebKit | **works** | `clang -fobjc-arc -framework AppKit -framework WebKit -o t_objc t.m` -> 49,968-byte binary, prints `WKWebViewConfiguration` address |
| swiftc + AppKit + WebKit | **works** | `swiftc -o t_swift t.swift` -> links, runs, `WKWebViewConfiguration` instantiated |
| Rust + objc2 chain against SDK 13.3 | **works, proven by run** | `cargo build` compiles and links `objc2 0.6`, `objc2-foundation 0.3.2`, `objc2-app-kit 0.3.2`, `objc2-web-kit 0.3.2`; the binary constructs `WKWebViewConfiguration` and `WKWebView` and exits 0. Debug binary 806,888 bytes |
| SwiftPM build of this repo | **impossible** | `swift package dump-package` -> `xcrun: error: unable to lookup item 'PlatformPath' from command line tools installation`. Even with Xcode, the manifest demands `swift-tools-version: 6.0` (toolchain here is 5.8.1) and `platforms: [.macOS(.v14)]` (max SDK here is 13.3) |
| `WKWebExtension` anything | **impossible** | no `WKWebExtension.h` in SDK 13.3 (0 headers); 0 `macos(14` availability annotations in the whole WebKit header set |
| passkeys | **impossible without Apple** | requires `com.apple.developer.web-browser.public-key-credential` plus a Developer ID provisioning profile (`Drift.passkeys.entitlements`), which needs Xcode plus an Apple grant |

The objc2 route is the only one of the four that is both buildable and native. Note the trap in
the last two rows: `objc2-web-kit 0.3.2` does ship 16 `WKWebExtension*.rs` binding files, so
those **compile** on this machine (bindings are pre-generated Rust, they do not need the SDK
header). They will resolve to nothing at runtime on macOS 13. Never enable those features; gate
every such call on a runtime currency check, not on the crate's compilation success.

## 6. OS-gated features, and whether Rust bindings can reach them

Measured on this machine in a live WKWebView (WebKit 18615.3.12), by evaluating one JSON probe:

```
{"sw":false,"wasm":"object","sab":false,"coi":false,"idb":true,"rtc":true,"push":false,
 "fsa":false,"clip":false,"share":false,"popover":false,"has":true,"container":true,
 "colorMix":true,"subgrid":true,"viewtrans":false,"webauthn":false,"injected":true,
 "uaData":false,"webgl2":true,"wasmThreads":true}
```

| Feature | Original path | First macOS | On 13.7.8 | Reachable from Rust? |
|---|---|---|---|---|
| Ad / tracker blocking | `WKContentRuleListStore` (`Shield.swift:15,179`) | 10.13 | **yes**, compiled and attached in probe | yes, `WKContentRuleList`/`WKContentRuleListStore` in `objc2-web-kit` |
| Internal pages (`drift://`) | `WKURLSchemeHandler` | 10.13 | **yes**, served and navigated in probe | yes |
| Private browsing | `nonPersistentDataStore` | 10.13 | **yes** | yes |
| Find in page | `findString:withConfiguration:` (`PageFind.swift`) | 11.0 | **yes**, `matchFound = YES` | yes |
| Reader / snapshot / PDF export | `takeSnapshot`, `createPDF` | 10.13 / 11.0 | **yes**, 1200x900 image, 16,401-byte PDF | yes |
| User scripts + native bridge | `WKUserScript`, `WKScriptMessageHandler` | 10.10 | **yes**, both directions verified | yes |
| Web Inspector | `isInspectable` (`WKWebView.h:665`) | 13.3 | **yes** | yes |
| Tab suspension / discard | app policy over `WKWebView` lifecycle | any | **yes**, this is app code | yes |
| Page zoom, magnification, swipe nav | `pageZoom`, `allowsMagnification` | 11.0 | **yes** | yes |
| Chrome extensions | `WKWebExtensionController` (`Extensions.swift:62`, 105 sites at 15.4) | **15.4** | **no** | **no binding on 13.3**; would compile via objc2 and be nil at runtime |
| Passkeys | `ASAuthorization` + restricted entitlement (`Passkeys.swift`) | 14.0, entitlement gated | **no** | API reachable, entitlement not obtainable here |
| Page translation | `import Translation` (`Translate.swift:4,58`) | 15.0 | **no** | framework absent from SDK |
| Keep site awake | `ProcessInfo` activity + assertions (`Sleep.swift`) | any | **yes** | yes, via `objc2-foundation` + IOKit |
| Service workers / offline | n/a | not in WKWebView here | **no** (`sw:false`) | not a binding problem, a WebKit-in-WKWebView gap |
| Web Push | n/a | no | **no** (`push:false`) | same |
| File System Access API | n/a | no | **no** (`fsa:false`) | same |
| WebAuthn from the page | n/a | no | **no** (`webauthn:false`, `PublicKeyCredential` absent) | same |
| `<div popover>` | n/a | Safari 17 | **no** (`popover:false`) | engine ceiling |
| View Transitions | n/a | Safari 18 | **no** (`viewtrans:false`) | engine ceiling |

So on this machine the honest feature ceiling is: **everything except extensions, passkeys,
translation, and the post-16.6 web platform additions.** The first three are exactly the three
things the README sells hardest. That is the real "no compromise" collision, and it is a
decision the user has to make, not a thing the port can engineer around.

## 7. Crate plan

All versions below are present and were resolved from the local registry on this machine.

| Crate | Version | Role |
|---|---|---|
| `objc2` | 0.6.5 | runtime, `Retained`, `MainThreadMarker`, `define_class!`, `sel!` |
| `objc2-foundation` | 0.3.2 | `NSString`, `NSURL`, `NSData`, `NSNotification`, `NSThread` |
| `objc2-app-kit` | 0.3.2 | `NSApplication`, `NSWindow`, `NSView`, `NSMenu`, `NSToolbar`, `NSSplitView` |
| `objc2-web-kit` | 0.3.2 | `WKWebView`, `WKWebViewConfiguration`, `WKUserContentController`, `WKContentRuleList`, `WKContentRuleListStore`, `WKWebsiteDataStore`, `WKNavigationDelegate`, `WKURLSchemeHandler`, `WKFindConfiguration` |
| `objc2-core-foundation` | 0.3.2 | `CFString`, `CFURL`, run loop and dispatch interop |
| `objc2-javascript-core` | 0.3.2 | only if `evaluateJavaScript` return values need typed access |
| `objc2-security` | 0.3.2 | `SecTrust` for certificate decisions (`WKWebView.h:200`) |
| `block2` | 0.6.2 | Objective-C blocks for the async WebKit completion handlers |
| `dispatch2` | 0.3.1 | `dispatch_async` to main queue |
| `serde` / `serde_json` | 1.0.229 / 1.0.154 | profile, session, rules persistence |
| `url` / `percent-encoding` | 2.5.8 / 2.3.2 | address parsing, omnibox |
| `thiserror` / `anyhow` | 2.0.21 / 1.0.104 | error types |
| `parking_lot` | 0.12.5 | cheap locks around the tab model |
| `ahash` | 0.8.12 | hash maps for history and rules |
| `global-hotkey` | 0.8.0 | optional, only if keyboard shortcuts need system-wide scope |
| `notify-rust` | 4.12.0 | optional, download-complete notifications |

Explicitly **not** in the tree: `tauri` (2.12.6 available, do not use), `wry`, `tao`, any
`webkit2gtk`/`webview2` path, any JS bundler, any embedded Chromium.

Storage: `rusqlite` is not in the local registry, and SQLite is already in the SDK
(`MacOSX.sdk/usr/lib/libsqlite3.tbd`, `usr/include/sqlite3.h`). Bind the 12 functions you need
(`open`, `close`, `prepare`, `step`, `column_*`, `bind_*`, `exec`) with `unsafe extern "C"`
rather than pull a dependency chain. History and bookmarks that way cost well under 1 MB of RSS.

Reachability rules for these crates, learned from the probes:

- `objc2-app-kit` and `objc2-web-kit` are `#![no_std]` shells with feature-gated modules; the
  features you enable are **inlining surface**, and every class you enable compiles into the
  binary whether or not you call it. Enable per class, not per framework. A first slice needs
  roughly `NSApplication`, `NSWindow`, `NSView`, `NSResponder`, plus the nine `WK*` types above.
- Feature-gated APIs are `#[cfg(target_os = "macos")]`, not availability-gated. The crate will
  happily compile a call to a macOS 14 class on a 13.3 SDK. Currency checks are your job.
- Every WebKit completion handler arrives on the main queue; `WKWebView`, `WKWebViewConfiguration`
  and `NSView` are main-thread-only and the crate enforces it with `MainThreadMarker`.

## 8. Memory budget, and the "strictly less" audit

What the port can honestly claim, measured:

| Line item | Original (Swift) | Rust native | Delta |
|---|---|---|---|
| Idle shell | 12.6 MB (Swift, app running) | measured 11.0 MB in a **debug** Rust build with a `WKWebView` constructed, AppKit up; the Obj-C floor with no webview is 9.6 MB. A release build of the same probe was still linking when this was written, so the Rust floor is **not settled**: measure it before quoting it | -1 MB class, pending a release number |
| Binary on disk | 5.4 MB per arch, 10.9 MB universal | 806,888 bytes debug; release pending, expect the low single-digit MB | -2 to -9 MB |
| Per visible tab | 1 `WebContent` (about 20 MB blank, 57 MB on a heavy article) | identical, same engine | 0 |
| Networking process | 1 shared, about 12 MB | identical | 0 |
| Injected JS (extensions, shims) | up to 5,752 lines of shim (`ExtensionShims.swift`) | none on macOS 13, feature unreachable | -unknown, real |
| Idle CPU | 0.0 % measured on a loaded WKWebView | identical | 0 |

The only honest statement of the user's requirement that survives this table is: **strictly less
on the shell and on disk, and structurally less on anything the port declines to load (extension
shims, a JS bridge, an updater runtime)**. Per-tab memory is a function of WKWebView and of how
many pages are alive, and it can only be improved by policy: one live webview per visible tab,
session restore that stores URL plus title without instantiating a webview, and discard under
pressure. Those are app decisions, and they are worth more than the language choice by an order
of magnitude.

## 9. Risks, and what would flip the decision

1. **Extensions, passkeys, translation are unreachable on 13.7.8.** If the user means "no
   compromise" to include those, then no stack satisfies the request on this machine and the
   answer is a macOS 15.4+ target, not a rewrite. Do not let an implementation plan quietly drop
   them; name them in the roadmap as OS-gated.
2. **No signing identity.** CLT only means no `identity` for `codesign` beyond ad-hoc, so TCC
   prompts (camera, mic, location, address book in `Drift.entitlements`) will behave like an
   unsigned app and Gatekeeper will not accept a distributed build. Development is fine;
   shipping is not. Passkeys need a Developer ID profile as well.
3. **`objc2-app-kit` cold build time** on this thermal envelope is the practical bottleneck for
   a subagent-driven port. Mitigate with a warm shared `target/` and `sccache`, and by keeping
   the enabled feature set small.
4. **The engine ceiling is fixed at Safari 16.6** while the fork stays on 13.7.8. Modern sites
   will degrade in ways a Chromium-based browser would not show. This is the price of the
   10 MB, 0-dependency design and it is the same price the original pays.
5. **Nothing flips this decision to Tauri** unless the product becomes a web-app wrapper instead
   of a browser. Nothing flips it to Swift unless an Xcode 16 plus macOS 14+ machine appears,
   and even then a Swift rebuild buys back the 3 MB shell floor and the extension engine but
   still cannot run on this OS.

## 10. Reproduce everything

```bash
# environment
sw_vers; uname -m; clang --version; swift --version; rustc --version
plutil -p /Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/SDKSettings.plist | grep Version
ls /Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/System/Library/Frameworks/WebKit.framework/Headers | wc -l

# what the engine is
defaults read /Applications/Safari.app/Contents/Info.plist CFBundleVersion

# Obj-C route builds and runs
clang -fobjc-arc -framework AppKit -framework WebKit -o /tmp/t_objc /tmp/t.m && /tmp/t_objc

# SwiftPM route cannot even parse the manifest here
swift package dump-package   # xcrun: error: unable to lookup item 'PlatformPath'

# Rust route, proven: /tmp/drift-probe/rust with
#   objc2 0.6, objc2-foundation 0.3.2, objc2-app-kit 0.3.2, objc2-web-kit 0.3.2
#   src/main.rs builds a WKWebView and prints both pointers
cd /tmp/drift-probe/rust && cargo build && ./target/debug/drift-probe
# -> rust ok: webview=0x... view=0x...   (exit 0)

# per-webview process model
ps -Ao pid=,rss=,command= | grep "WebKit.WebContent.xpc" | grep -v grep
```

## 11. Sources

Primary, read on this machine:

- `/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/System/Library/Frameworks/WebKit.framework/Headers/WKWebView.h` (class at line 61, `isInspectable` at 665, `customUserAgent` at 460, `pageZoom` at 502)
- same directory, `WKWebViewConfiguration.h` (line 94 class, 102 `processPool`, 110 `userContentController`, 191 `websiteDataStore`), `WKUserContentController.h` (133 `addContentRuleList:`), `WKPreferences.h` (57, 76, 81 availability), `WKContentRuleListStore.h`, `WKURLSchemeHandler.h`
- `.../AppKit.framework/Headers/` (283 headers)
- `Package.swift`, `build.sh`, `README.md`, `Drift.entitlements`, `Drift.passkeys.entitlements`, `Sources/Drift/{Shield,Extensions,Translate,Sleep,Passkeys,PageFind}.swift` in this checkout

External:

- Apple, [WKProcessPool](https://developer.apple.com/documentation/webkit/wkprocesspool): "WebKit renders the content of web views in separate processes, rather than in your app's process space. By default, WebKit gives each web view its own process space until it reaches an implementation-defined process limit"; deprecated macOS 12.
- Apple, [Safari Release Notes](https://developer.apple.com/documentation/safari-release-notes)
- Tauri, [Architecture](https://v2.tauri.app/concept/architecture/) and [Process Model](https://v2.tauri.app/concept/process-model/) ("The Core process doesn't render the actual user interface (UI) itself; it spins up WebView processes that leverage WebView libraries provided by the operating system").
- `wry` 0.57.2 source, local registry: `src/lib.rs:2493` (`WebViewExtMacOS`), `src/lib.rs:2513` (returns `Retained<WryWebView>`), `src/wkwebview/class/wry_web_view.rs:40-118` (subclass and its overrides), `src/wkwebview/mod.rs` (the exposed surface).
- [docs.rs/objc2-web-kit](https://docs.rs/objc2-web-kit) (WKWebView, WKWebViewConfiguration, WKContentRuleList, WKUserContentController, WKNavigationDelegate).
- `objc2-web-kit` 0.3.2 `Cargo.toml` feature table on this machine (per-class features), and its 16 `WKWebExtension*.rs` generated files, which compile on macOS 13 while the classes they name do not exist there.

## 12. One-line recommendation

Build Drift as a single-process Rust app on `objc2-app-kit` plus `objc2-web-kit` driving
WKWebView directly, spend the memory effort on tab policy rather than on language, and treat
extensions, passkeys and translation as macOS-version-gated features that this machine cannot
have under any stack.
