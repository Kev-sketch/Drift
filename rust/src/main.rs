// Drift: native macOS browser shell.
//
// One process. NSApplication + NSWindow from objc2-app-kit, the page in a system
// WKWebView from objc2-web-kit. No Tauri, no wry, no tao, no webview wrapper
// layer, no injected JS framework: the app talks to WebKit directly.
//
// Layout: address bar (NSTextField) across the top, web view beneath it.

use objc2::rc::Retained;
use objc2::runtime::{NSObject, ProtocolObject};
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSTextField,
    NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSURL, NSURLRequest};
use objc2_web_kit::{WKNavigation, WKNavigationDelegate, WKWebView, WKWebViewConfiguration};

const HOME: &str = "https://example.com";
const WINDOW_W: f64 = 1100.0;
const WINDOW_H: f64 = 760.0;
const BAR_H: f64 = 34.0;

/// Everything the delegate needs to touch. The window, the page and the address
/// bar are all main-thread objects, and the delegate itself is MainThreadOnly,
/// so plain owned fields are enough: no locks, no channels.
struct Ivars {
    window: Retained<NSWindow>,
    webview: Retained<WKWebView>,
    address: Retained<NSTextField>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "DriftDelegate"]
    #[ivars = Ivars]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl WKNavigationDelegate for Delegate {
        // Page finished: take its title for the window, its URL for the bar.
        #[unsafe(method(webView:didFinishNavigation:))]
        fn web_view_did_finish(&self, web_view: &WKWebView, _navigation: Option<&WKNavigation>) {
            let ivars = self.ivars();
            unsafe {
                if let Some(title) = web_view.title() {
                    ivars.window.setTitle(&title);
                    // Not decoration: this is how the shell proves it rendered
                    // something real when there is no screenshot to take.
                    println!("drift: loaded title={}", title.to_string());
                }
                if let Some(url) = web_view.URL() {
                    if let Some(text) = url.absoluteString() {
                        ivars.address.setStringValue(&text);
                        println!("drift: url={}", text.to_string());
                    }
                }
            }
        }

        // A failed provisional load is normal (bad host, refused connection);
        // show it instead of silently keeping the old page.
        #[unsafe(method(webView:didFailProvisionalNavigation:withError:))]
        fn web_view_did_fail(
            &self,
            _web_view: &WKWebView,
            _navigation: Option<&WKNavigation>,
            error: &objc2_foundation::NSError,
        ) {
            let text = unsafe { error.localizedDescription() };
            self.ivars().window.setTitle(&text);
        }
    }

    impl Delegate {
        // Enter in the address bar lands here: NSTextField sends its action when
        // editing ends. This is the whole "browser chrome" logic so far.
        #[unsafe(method(driftGo:))]
        fn go(&self, sender: &NSTextField) {
            let text = sender.stringValue();
            let Some(url) = normalize(&text) else { return };
            let request = NSURLRequest::requestWithURL(&url);
            unsafe { self.ivars().webview.loadRequest(&request) };
        }
    }
);

impl Delegate {
    fn new(mtm: MainThreadMarker, ivars: Ivars) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
    }
}

/// Accept "example.com", "https://example.com", "localhost:8080".
fn normalize(text: &NSString) -> Option<Retained<NSURL>> {
    let raw = text.to_string();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let candidate = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    NSURL::URLWithString(&NSString::from_str(&candidate))
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");

    let app = NSApplication::sharedApplication(mtm);
    // Regular, not the default Prohibited: without this a bare binary has no
    // Dock icon and the window will not take focus.
    unsafe {
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    }

    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WINDOW_W, WINDOW_H));
    let style = NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Miniaturizable
        | NSWindowStyleMask::Resizable;
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            style,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    window.setTitle(&NSString::from_str("Drift"));
    window.center();

    let Some(content) = window.contentView() else {
        eprintln!("no content view");
        std::process::exit(1);
    };
    let (cw, ch) = {
        let f = content.frame();
        (f.size.width, f.size.height)
    };

    // Address bar: top strip, full width, stays at the top when the window grows.
    let bar_rect = NSRect::new(
        NSPoint::new(0.0, ch - BAR_H),
        NSSize::new(cw, BAR_H),
    );
    let address = NSTextField::initWithFrame(NSTextField::alloc(mtm), bar_rect);
    address.setStringValue(&NSString::from_str(HOME));
    address.setAutoresizingMask(
        objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable
            | objc2_app_kit::NSAutoresizingMaskOptions::ViewMinYMargin,
    );

    // Page: everything below the bar, grows both ways.
    let web_rect = NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(cw, ch - BAR_H),
    );
    let config = unsafe { WKWebViewConfiguration::new(mtm) };
    let webview =
        unsafe { WKWebView::initWithFrame_configuration(WKWebView::alloc(mtm), web_rect, &config) };
    webview.setAutoresizingMask(
        objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable
            | objc2_app_kit::NSAutoresizingMaskOptions::ViewHeightSizable,
    );

    let delegate = Delegate::new(
        mtm,
        Ivars {
            window: window.clone(),
            webview: webview.clone(),
            address: address.clone(),
        },
    );

    let view: &NSView = &webview;
    content.addSubview(view);
    let bar: &NSView = &address;
    content.addSubview(bar);

    unsafe {
        webview.setNavigationDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        // The bar's action lives on the delegate. setTarget: takes AnyObject, and
        // a Rust class reference goes across the bridge as a plain object
        // pointer, so send it rather than cast it.
        let _: () = msg_send![&*address, setTarget: &*delegate];
        let _: () = msg_send![&*address, setAction: sel!(driftGo:)];
    }

    window.makeKeyAndOrderFront(None);
    // `activate` is macOS 14+; on 13 it panics with "method not found".
    // activateIgnoringOtherApps is deprecated but present on every version here.
    #[allow(deprecated)]
    unsafe {
        app.activateIgnoringOtherApps(true)
    };

    let request = NSURLRequest::requestWithURL(&NSURL::URLWithString(&NSString::from_str(HOME)).unwrap());
    unsafe { webview.loadRequest(&request) };

    // The delegate is owned by the web view's weak delegate slot and by nothing
    // else; keep it alive for the life of the process.
    std::mem::forget(delegate);

    println!("drift: window up, loading {HOME}");
    app.run();
}
