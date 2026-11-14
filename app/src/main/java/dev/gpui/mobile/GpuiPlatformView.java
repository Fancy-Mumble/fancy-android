package dev.gpui.mobile;

import android.app.Activity;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.webkit.JavascriptInterface;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.webkit.WebChromeClient;
import android.widget.FrameLayout;
import android.widget.PopupWindow;
import java.util.HashMap;
import java.util.Map;

/**
 * Helper class for managing native platform views embedded in the GPUI render tree.
 *
 * Platform views are native Android Views positioned absolutely over the
 * NativeActivity's content area. GPUI controls their position and visibility
 * via JNI calls from Rust.
 *
 * Supports view types:
 * - "container": Generic empty FrameLayout (for custom content)
 * - Additional types can be registered via registerViewType()
 */
public class GpuiPlatformView {
    private static final String TAG = "GpuiPlatformView";
    private static final Handler mainHandler = new Handler(Looper.getMainLooper());

    /** Map of view ID -> native View */
    private static final Map<Long, View> views = new HashMap<>();

    /** Map of view ID -> FrameLayout container */
    private static final Map<Long, FrameLayout> containers = new HashMap<>();

    /** The root FrameLayout that hosts all platform views */
    private static FrameLayout rootContainer;

    /**
     * Ensure the root container exists in the activity's view hierarchy.
     * Platform views are added as children of this container.
     */
    private static void ensureRootContainer(Activity activity) {
        if (rootContainer != null) {
            return;
        }

        mainHandler.post(() -> {
            if (rootContainer != null) return;

            rootContainer = new FrameLayout(activity);
            rootContainer.setLayoutParams(new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            ));

            // Add to the activity's content view on top of the NativeActivity surface.
            // NativeActivity uses an internal SurfaceView for native rendering.
            // addContentView adds to the end of the window's DecorView, which
            // renders ON TOP of the NativeActivity's SurfaceView.
            activity.addContentView(rootContainer, new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            ));

            // Log the view hierarchy for debugging
            try {
                View decorView = activity.getWindow().getDecorView();
                logViewHierarchy(decorView, 0);
            } catch (Exception e) {
                Log.w(TAG, "Could not log view hierarchy: " + e.getMessage());
            }

            Log.i(TAG, "Root container created and added to activity");
        });
    }

    /**
     * Create a new platform view and add it to the view hierarchy.
     *
     * @param activity       The hosting Activity
     * @param viewType       The type of view to create (e.g., "container", "video_player", "webview")
     * @param viewId         Unique ID for this view instance
     * @param x              Left position in logical pixels
     * @param y              Top position in logical pixels
     * @param width          Width in logical pixels
     * @param height         Height in logical pixels
     * @param creationParams Pipe-delimited key=value pairs (e.g., "player_id=1|url=https://...")
     * @return true if the view was created successfully
     */
    public static boolean createView(
            Activity activity,
            String viewType,
            long viewId,
            float x, float y,
            float width, float height,
            String creationParams) {

        Log.i(TAG, "createView: type=" + viewType + " id=" + viewId
                + " bounds=(" + x + ", " + y + ", " + width + ", " + height + ")"
                + " params=" + creationParams);

        // Parse creation params
        Map<String, String> params = parseCreationParams(creationParams);
        params.put("view_id", String.valueOf(viewId));

        mainHandler.post(() -> {
            try {
                float density = activity.getResources().getDisplayMetrics().density;

                // Create a container FrameLayout for this platform view
                FrameLayout container = new FrameLayout(activity);
                FrameLayout.LayoutParams layoutParams = new FrameLayout.LayoutParams(
                    (int)(width * density),
                    (int)(height * density)
                );
                layoutParams.leftMargin = (int)(x * density);
                layoutParams.topMargin = (int)(y * density);
                container.setLayoutParams(layoutParams);

                // Create the actual view based on type
                View view = createViewForType(activity, viewType, params);
                if (view != null) {
                    container.addView(view, new FrameLayout.LayoutParams(
                        ViewGroup.LayoutParams.MATCH_PARENT,
                        ViewGroup.LayoutParams.MATCH_PARENT
                    ));
                }

                // Store references
                views.put(viewId, view != null ? view : container);
                containers.put(viewId, container);

                // A NativeActivity's own window surface belongs to the native
                // code (takeSurface): views added to its content are laid out
                // but never drawn. Each platform view gets a sub-window of its
                // own at its rectangle instead, drawn above the GPUI surface;
                // touches inside it go to it, the rest to GPUI.
                container.setLayoutParams(new FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
                PopupWindow window = new PopupWindow(container,
                    (int)(width * density), (int)(height * density), false);
                window.setClippingEnabled(false);
                window.setTouchable(true);
                windows.put(viewId, window);
                anchors.put(viewId, activity.getWindow().getDecorView());
                rects.put(viewId, new int[] {(int)(x * density), (int)(y * density),
                    (int)(width * density), (int)(height * density)});
                showWindow(viewId);

                Log.i(TAG, "View created: id=" + viewId + " type=" + viewType);
            } catch (Exception e) {
                Log.e(TAG, "Failed to create view: " + e.getMessage(), e);
            }
        });

        return true;
    }

    /**
     * Parse a pipe-delimited creation params string into a map.
     * Format: "key1=value1|key2=value2"
     */
    private static Map<String, String> parseCreationParams(String params) {
        Map<String, String> map = new HashMap<>();
        if (params == null || params.isEmpty()) return map;
        for (String pair : params.split("\\|")) {
            int eq = pair.indexOf('=');
            if (eq > 0) {
                map.put(pair.substring(0, eq), pair.substring(eq + 1));
            }
        }
        return map;
    }

    /**
     * Create a View instance based on the view type string and creation params.
     */
    private static View createViewForType(Activity activity, String viewType, Map<String, String> params) {
        switch (viewType) {
            case "container":
                FrameLayout frame = new FrameLayout(activity);
                frame.setBackgroundColor(0x00000000);
                return frame;

            case "video_player":
                return createVideoPlayerView(activity, params);

            case "webview":
                return createWebViewView(activity, params);

            case "camera_preview":
                return createCameraPreviewView(activity, params);

            case "map":
                // Placeholder — MapView requires Google Play Services SDK
                FrameLayout mapContainer = new FrameLayout(activity);
                mapContainer.setBackgroundColor(0xFFE0E0E0);
                return mapContainer;

            default:
                Log.w(TAG, "Unknown view type: " + viewType + ", creating empty container");
                return new FrameLayout(activity);
        }
    }

    /**
     * Create a TextureView for video playback and wire it to the MediaPlayer.
     */
    private static View createVideoPlayerView(Activity activity, Map<String, String> params) {
        int playerId = 0;
        try {
            playerId = Integer.parseInt(params.getOrDefault("player_id", "0"));
        } catch (NumberFormatException e) {
            Log.w(TAG, "Invalid player_id in creation params");
        }

        return GpuiVideoPlayer.createVideoSurface(activity, playerId);
    }

    /**
     * Create a WebView with settings from creation params.
     */
    private static View createWebViewView(Activity activity, Map<String, String> params) {
        boolean jsEnabled = Boolean.parseBoolean(params.getOrDefault("javascript_enabled", "true"));
        boolean domStorage = Boolean.parseBoolean(params.getOrDefault("dom_storage_enabled", "true"));
        boolean zoom = Boolean.parseBoolean(params.getOrDefault("zoom_enabled", "true"));
        String url = params.getOrDefault("url", "");
        String html = params.getOrDefault("html", "");
        String baseUrl = params.getOrDefault("base_url", null);
        long viewId = Long.parseLong(params.getOrDefault("view_id", "0"));

        WebView wv = new WebView(activity);
        WebSettings settings = wv.getSettings();
        settings.setJavaScriptEnabled(jsEnabled);
        settings.setDomStorageEnabled(domStorage);
        settings.setBuiltInZoomControls(zoom);
        if (zoom) settings.setDisplayZoomControls(false);
        settings.setLoadWithOverviewMode(true);
        settings.setUseWideViewPort(true);

        wv.setWebViewClient(new WebViewClient());
        wv.setWebChromeClient(new WebChromeClient());
        // Media starts without a tap only where the app asks (an embedded player).
        settings.setMediaPlaybackRequiresUserGesture(
                !Boolean.parseBoolean(params.getOrDefault("autoplay", "false")));
        // The page talks back through window.fancyNative.postMessage(string).
        wv.addJavascriptInterface(new Bridge(viewId), "fancyNative");

        if (!html.isEmpty()) {
            wv.loadDataWithBaseURL(baseUrl, html, "text/html", "UTF-8", null);
        } else if (!url.isEmpty()) {
            wv.loadUrl(url);
        }

        return wv;
    }

    /**
     * Create a TextureView for camera preview and wire it to the camera session.
     */
    private static View createCameraPreviewView(Activity activity, Map<String, String> params) {
        int sessionId = 0;
        try {
            sessionId = Integer.parseInt(params.getOrDefault("session_id", "0"));
        } catch (NumberFormatException e) {
            Log.w(TAG, "Invalid session_id in creation params");
        }

        return GpuiCamera.createPreviewSurface(activity, sessionId);
    }

    /**
     * Update a view's position and size.
     */
    public static void setBounds(long viewId, float x, float y, float width, float height) {
        mainHandler.post(() -> {
            FrameLayout container = containers.get(viewId);
            if (container == null) {
                Log.w(TAG, "setBounds: no container for id=" + viewId);
                return;
            }

            Activity activity = (Activity) container.getContext();
            float density = activity.getResources().getDisplayMetrics().density;
            int[] rect = {(int)(x * density), (int)(y * density),
                (int)(width * density), (int)(height * density)};
            rects.put(viewId, rect);
            PopupWindow window = windows.get(viewId);
            if (window != null && window.isShowing()) {
                window.update(rect[0], rect[1], rect[2], rect[3]);
            }
        });
    }

    /**
     * Show or hide a platform view.
     */
    public static void setVisible(long viewId, boolean visible) {
        mainHandler.post(() -> {
            FrameLayout container = containers.get(viewId);
            if (container == null) return;
            container.setVisibility(visible ? View.VISIBLE : View.GONE);
            if (visible) {
                hidden.remove(viewId);
                showWindow(viewId);
            } else {
                hidden.add(viewId);
                PopupWindow window = windows.get(viewId);
                if (window != null) window.dismiss();
            }
        });
    }

    /** Puts view {@code viewId}'s sub-window on screen at its rectangle. */
    private static void showWindow(long viewId) {
        PopupWindow window = windows.get(viewId);
        View anchor = anchors.get(viewId);
        int[] rect = rects.get(viewId);
        if (window == null || anchor == null || rect == null || hidden.contains(viewId)) return;
        if (window.isShowing()) {
            window.update(rect[0], rect[1], rect[2], rect[3]);
        } else if (anchor.getWindowToken() != null) {
            window.setWidth(rect[2]);
            window.setHeight(rect[3]);
            window.showAtLocation(anchor, Gravity.NO_GRAVITY, rect[0], rect[1]);
        } else {
            // The activity's window is not attached yet: try again shortly.
            mainHandler.postDelayed(() -> showWindow(viewId), 50);
        }
    }

    /** Takes view {@code viewId}'s sub-window off screen and forgets it. */
    private static void removeWindow(long viewId) {
        PopupWindow window = windows.remove(viewId);
        if (window != null) window.dismiss();
        anchors.remove(viewId);
        rects.remove(viewId);
        hidden.remove(viewId);
    }

    /** Each view's sub-window, the view it is shown over, and its rectangle in pixels. */
    private static final Map<Long, PopupWindow> windows = new HashMap<>();
    private static final Map<Long, View> anchors = new HashMap<>();
    private static final Map<Long, int[]> rects = new HashMap<>();
    /** Views the app hid; pausing and resuming leaves them hidden. */
    private static final java.util.Set<Long> hidden = new java.util.HashSet<>();

    /**
     * Set the z-order of a platform view.
     * Uses View.setZ() (API 21+) for elevation-based ordering.
     */
    public static void setZIndex(long viewId, int zIndex) {
        mainHandler.post(() -> {
            FrameLayout container = containers.get(viewId);
            if (container == null) return;
            container.setZ(zIndex);
        });
    }

    /**
     * Remove and dispose a platform view.
     */
    public static void disposeView(long viewId) {
        mainHandler.post(() -> {
            removeWindow(viewId);
            FrameLayout container = containers.get(viewId);
            if (container != null) {
                container.removeAllViews();
                containers.remove(viewId);
            }
            View view = views.remove(viewId);
            if (view instanceof WebView) ((WebView) view).destroy();
            Log.i(TAG, "View disposed: id=" + viewId);
        });
    }

    /**
     * Loads {@code html} into web view {@code viewId} as if it came from
     * {@code baseUrl} (an https origin, so an embedded player accepts it).
     */
    public static void loadHtml(long viewId, String html, String baseUrl) {
        mainHandler.post(() -> {
            View view = views.get(viewId);
            if (view instanceof WebView) {
                ((WebView) view).loadDataWithBaseURL(baseUrl, html, "text/html", "UTF-8", null);
            }
        });
    }

    /** Runs {@code script} in web view {@code viewId}'s page. */
    public static void evaluate(long viewId, String script) {
        mainHandler.post(() -> {
            View view = views.get(viewId);
            if (view instanceof WebView) ((WebView) view).evaluateJavascript(script, null);
        });
    }

    /** What a page sends the app: one string at a time, to Rust. */
    private static final class Bridge {
        private final long viewId;

        Bridge(long viewId) {
            this.viewId = viewId;
        }

        @JavascriptInterface public void postMessage(String message) {
            nativeWebMessage(viewId, message == null ? "" : message);
        }
    }

    private static native void nativeWebMessage(long viewId, String message);

    /**
     * Get the native View for a given platform view ID.
     * Useful for packages that need direct access to the Android View.
     */
    public static View getView(long viewId) {
        return views.get(viewId);
    }

    /**
     * Get the container FrameLayout for a given platform view ID.
     */
    public static FrameLayout getContainer(long viewId) {
        return containers.get(viewId);
    }

    /**
     * Pause all platform views. Called when the app goes to background.
     * Views are hidden to release rendering resources.
     */
    public static void pauseAll() {
        mainHandler.post(() -> {
            for (PopupWindow window : windows.values()) {
                window.dismiss();
            }
            Log.i(TAG, "All platform views paused");
        });
    }

    /**
     * Resume all platform views. Called when the app returns to foreground.
     * Views are made visible again.
     */
    public static void resumeAll() {
        mainHandler.post(() -> {
            for (Long viewId : windows.keySet()) {
                showWindow(viewId);
            }
            Log.i(TAG, "All platform views resumed");
        });
    }

    /**
     * Dispose all platform views. Called during activity cleanup.
     */
    public static void disposeAll() {
        mainHandler.post(() -> {
            for (Long viewId : new java.util.ArrayList<>(windows.keySet())) {
                removeWindow(viewId);
            }
            for (FrameLayout container : containers.values()) {
                container.removeAllViews();
            }
            for (View view : views.values()) {
                if (view instanceof WebView) ((WebView) view).destroy();
            }
            views.clear();
            containers.clear();
            Log.i(TAG, "All platform views disposed");
        });
    }

    /**
     * Check if a touch point hits any visible platform view.
     *
     * Coordinates are in physical pixels relative to the window.
     * This can be called from any thread (reads only from containers map
     * on the calling thread, but note that container positions are set
     * asynchronously on the UI thread, so there is a small race window).
     *
     * For the primary hit-test path, the Rust side uses its own
     * PlatformViewRegistry.hit_test() which is synchronous and lock-free
     * on the native thread. This Java method is provided as a convenience
     * for Java-side callers.
     *
     * @param x Physical pixel x coordinate
     * @param y Physical pixel y coordinate
     * @return true if the point falls within a visible platform view
     */
    public static boolean hitTest(float x, float y) {
        for (Map.Entry<Long, FrameLayout> entry : containers.entrySet()) {
            FrameLayout container = entry.getValue();
            if (container.getVisibility() != View.VISIBLE) continue;

            int[] location = new int[2];
            container.getLocationOnScreen(location);
            int left = location[0];
            int top = location[1];
            int right = left + container.getWidth();
            int bottom = top + container.getHeight();

            if (x >= left && x <= right && y >= top && y <= bottom) {
                return true;
            }
        }
        return false;
    }

    /**
     * Dispatch a touch event to the platform view hierarchy.
     *
     * Used to forward NativeActivity input events to platform views
     * when the Rust-side hit-test determines the touch lands on a
     * platform view. The event is posted to the UI thread for dispatch.
     *
     * @param x      Physical pixel x coordinate
     * @param y      Physical pixel y coordinate
     * @param action MotionEvent action constant (0=DOWN, 1=UP, 2=MOVE)
     */
    public static void dispatchTouch(float x, float y, int action) {
        mainHandler.post(() -> {
            if (rootContainer == null) return;

            long now = android.os.SystemClock.uptimeMillis();
            android.view.MotionEvent event = android.view.MotionEvent.obtain(
                now, now, action, x, y, 0
            );
            rootContainer.dispatchTouchEvent(event);
            event.recycle();
        });
    }

    /**
     * Log the view hierarchy from a root view for debugging.
     */
    private static void logViewHierarchy(View view, int depth) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < depth; i++) sb.append("  ");
        sb.append(view.getClass().getSimpleName());
        sb.append(" [").append(view.getWidth()).append("x").append(view.getHeight()).append("]");
        sb.append(" vis=").append(view.getVisibility() == View.VISIBLE ? "V" : view.getVisibility() == View.GONE ? "G" : "I");
        if (view instanceof android.view.SurfaceView) {
            sb.append(" (SurfaceView)");
        }
        Log.i(TAG, sb.toString());
        if (view instanceof ViewGroup) {
            ViewGroup vg = (ViewGroup) view;
            for (int i = 0; i < vg.getChildCount(); i++) {
                logViewHierarchy(vg.getChildAt(i), depth + 1);
            }
        }
    }
}
