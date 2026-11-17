# Review: Drift, research, fork and Rust shell

Reviewed 2026-11-17 by `backup` (t4). Strict pass over t1, t2, t3. Everything below was re-run
or re-read on this machine; where I could not verify something I say so.

Verdicts:

| Task | Deliverable | Verdict |
|---|---|---|
| t1 research | `Drift/docs/ARCHITECTURE.md` | **pass**, 2 minor fixes |
| t2 fork-rebrand | `Kev-sketch/Drift` at `ab803f7` | **pass**, 2 follow-ups (one is mine to own) |
| t3 scaffold | `DriftRust/` + `target/debug/drift` | **pass**, 1 structural follow-up |

Conflict of interest, stated plainly: I (backup) wrote t2. The captain verified it independently
before this review. t2's verdict below rests on commands anyone can re-run, not on my account of
what I did.

## t1: ARCHITECTURE.md - pass

Decision is unambiguous and correctly scoped: native Rust + `objc2-app-kit` + `objc2-web-kit`
driving WKWebView, one process, no Tauri/Wry/tao, no Swift rewrite (section 0, section 12). It
also refuses the flattering version of the user's ask: section 3 measures that 70-90% of the
footprint is the page's own WebContent process, and section 8 says the debug Rust floor is
"not settled: measure it before quoting it".

Citations are real, and the ones that carry the decision are verifiable:

- Tauri, quoted verbatim and confirmed by fetch: "Tauri uses WRY as the abstract layer
  responsible to determine which webview is used (and how interactions are made)"
  (https://v2.tauri.app/concept/architecture/, match).
- Tauri process model, quoted verbatim and confirmed by fetch: "The Core process doesn't render
  the actual user interface (UI) itself; it spins up WebView processes that leverage WebView
  libraries provided by the operating system" (https://v2.tauri.app/concept/process-model/, match).
- `wry` 0.57.2 claim, checked against the local registry source: `define_class!` at
  `src/wkwebview/class/wry_web_view.rs:37`, `WebViewExtMacOS::webview()` returns
  `Retained<WryWebView>` near `src/lib.rs:2493`/`2513`, and `grep -rn ContentRuleList wry-0.57.2/src`
  returns 0. The "no content rule list API" claim is true.
- `objc2-web-kit 0.3.2` ships exactly 16 `WKWebExtension*.rs` generated bindings, as claimed.
- SDK facts, counted: `MacOSX13.3.sdk` WebKit headers = 200, `WKWebExtension.h` = 0, AppKit
  headers = 283. `rustc 1.99.1 (58cf8f9c3 2026-10-13)`.
- Header line numbers, spot-checked: `WKWebView.h` 460 `customUserAgent`, 502 `pageZoom`, 665
  `isInspectable`, 61 the class; `WKUserContentController.h` 133 `addContentRuleList:`. All correct.

Minor fixes:

1. Section 8 quotes the binary as "806,888 bytes debug". That is the t1 probe, not the shell t3
   shipped, which is 1,482,456 bytes. Cite the shell number for the shell, both for honesty and
   because the first thing a reader compares is the delivered artifact.
2. The Apple WKProcessPool quote is the one external citation I could not machine-verify:
   developer.apple.com renders its docs with scripts, so a text fetch returns nothing. The URL is
   canonical and the wording matches Apple's documented process model, but the doc should either
   carry a captured quote beside it or point at a fetchable mirror.

Not a defect, worth naming because it reshapes the project: section 2 proves the original is
macOS 14 with a 15.4 extension engine, so it cannot run here at all. That is the strongest single
argument in the document and it is made with counted evidence (68+37 sites at 15.4, and so on).

## t2: fork and rebrand - pass

Confirmed by execution, not by assertion:

- `gh api repos/Kev-sketch/Drift`: `fork: true`, `parent: driceroland/Search`, `default_branch:
  main`, `pushed_at` 2026-11-17T17:33:56Z.
- Local HEAD and `git ls-remote origin refs/heads/main` are both
  `ab803f7bdd831dcbec62b7580393b6f96e1afeb0`. Upstream history at `5962b7f` is unrewritten.
- Remote `LICENSE` begins with the fork note and still carries "MIT License" and "Copyright (c)
  2026 Office Commun" with the full MIT text. Attribution is intact.
- Remote `Package.swift`: package `Drift`, target `Drift` at `Sources/Drift`, test target
  `DriftTests` at `Tests/DriftTests`.
- `git diff 5962b7f HEAD --name-only` contains no `CHANGELOG.md` or `ROADMAP.md`.
- Fresh greps: `officecommun.search` and `import Search` = 0 hits outside the two history files.

Follow-ups:

1. GitHub reports `license: NOASSERTION` on the repo. The MIT text is intact, but the attribution
   note was placed above the licence header, which breaks GitHub's licensee match. Move the note
   below the MIT paragraph, or move it to a `NOTICE` file, to get the MIT badge back. Severity is
   cosmetic, not legal.
2. Neither `Drift/docs/ARCHITECTURE.md` (t1) nor this file is committed. The fork as pushed
   contains the rebranded browser but none of the research. If the docs are meant to travel with
   the fork, they need a commit and a push.

Tree state: at `ab803f7` the tree was clean. It is now dirty only by `?? docs/`, which is t1's
file, not a regression in t2.

## t3: native Rust shell - pass

Verified by running it, not by reading about it:

- Binary: `DriftRust/target/debug/drift`, 1,482,456 bytes, Mach-O 64-bit executable x86_64.
- Dependencies: `Cargo.lock` resolves 21 packages and every one is an `objc2` family crate
  (`objc2`, `objc2-app-kit`, `objc2-web-kit`, `objc2-foundation`, `objc2-core-*`,
  `objc2-quartz-core`, `objc2-security`, `bitflags`, `block2`, `dispatch2`, `libc`, `drift`).
  Grep for `tauri|wry|tao|v8|javascriptcore|quickjs|boa|deno|node` finds only the explanatory
  comment in `Cargo.toml`. There is no Tauri, no Wry and no bundled JS runtime.
  One precision note: `objc2-javascript-core` appears transitively through `objc2-web-kit`. That
  is the binding to the macOS JavaScriptCore framework WebKit itself already uses, not an
  embedded engine. Say it that way in future docs so nobody reads "JS runtime" and thinks V8.
- Source: `src/main.rs`, 213 lines, `NSApplication` (activation policy `Regular`), `NSWindow`,
  one `NSTextField` address bar, one `WKWebView`, one `define_class!` delegate implementing
  `WKNavigationDelegate` plus the bar's action. Exactly what the task asked for and nothing more.
- Launch, this review, killed after 7 s (stdout captured verbatim):
  `drift: window up, loading https://example.com` / `drift: loaded title=Example Domain` /
  `drift: url=https://example.com/`, process alive at 7 s, exited on SIGTERM. This matches
  `docs/BUILD.md` line for line, so the recorded run is not embellished.
- `docs/BUILD.md` is 127 lines and is honest where it matters: `cargo build --release` is stated
  as **not run**, with the reason, and no release size, memory or timing number is invented. The
  cold-build figure carries its own caveat about competing cargo builds.

The `NSApplication::activate()` trap is real and correctly handled: the code uses
`activateIgnoringOtherApps:` with a comment naming the macOS 14 requirement and the panic text.
That is the same class of binding-compiles-selector-absent risk t1 predicted in section 5/6.

Follow-up, structural rather than code:

1. `DriftRust/` is not under version control at all (`git rev-parse` fails; there is no `.git`
   and no enclosing repo). The Rust rebuild is the actual product of this team and it exists only
   on this disk. `git init` plus a push into `Kev-sketch/Drift` (as a subtree or a second branch),
   keeping `Cargo.lock`, is the single highest-value follow-up in this review.

## Caveats to carry forward

1. The Swift fork cannot be built here: CLT-only Swift 5.8.1 against `swift-tools-version: 6.0`
   and `platforms: [.macOS(.v14)]` with SDK 13.3. `swift package dump-package` fails outright.
   t2's rebrand is therefore verified textually and structurally, never by a compile.
2. macOS 13.7.8 cannot do Chrome extensions (WebKit 15.4+), passkeys (Developer ID entitlement) or
   page translation (framework absent from the SDK). Three of the fork's headline features are
   unreachable on this hardware under any stack. t1 says so; the roadmap has to keep saying so.
3. The updater still points at `https://officecommun.com/search`, so the fork as pushed would
   offer to replace itself with an upstream Search build. Repoint or disable before anyone runs
   it as a daily browser. Documented in the fork's README, unresolved in code.
4. Memory comparisons are not apples-to-apples: the Rust numbers are debug builds, and the
   release build was deliberately skipped. Do not quote 11.0 MB against Swift's 12.6 MB as a
   settled result; t1 already labels it unsettled and this review agrees.

## Bottom line

All three tasks meet their acceptance criteria and none fabricated a result. The decision to go
native Rust is argued with evidence that survives re-checking, the fork is real and properly
attributed, and the shell genuinely launches and renders under WebKit. The open risks are not
quality problems, they are choices still owed: version-control the Rust tree, fix the licence
badge, repoint the updater, and accept or reject the three OS-gated features on the record.
