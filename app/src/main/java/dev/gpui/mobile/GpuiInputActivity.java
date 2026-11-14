package dev.gpui.mobile;

import android.app.NativeActivity;
import android.content.pm.PackageManager;
import android.graphics.Insets;
import android.os.Build;
import android.os.Bundle;
import android.text.Editable;
import android.text.InputType;
import android.text.Selection;
import android.text.TextWatcher;
import android.view.KeyEvent;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowInsets;
import android.view.WindowInsetsAnimation;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputConnectionWrapper;
import android.view.inputmethod.InputMethodManager;
import android.widget.EditText;

import java.util.List;

/** NativeActivity with a UI-thread InputConnection for multistage IMEs. */
public class GpuiInputActivity extends NativeActivity {
    private InputProxy input;

    @Override protected void onCreate(Bundle state) {
        // NativeActivity's dlopen alone does not register JNI native methods.
        try {
            String library = getPackageManager().getActivityInfo(getComponentName(),
                    PackageManager.GET_META_DATA).metaData.getString("android.app.lib_name");
            if (library != null) System.loadLibrary(library);
        } catch (PackageManager.NameNotFoundException error) {
            throw new IllegalStateException(error);
        }
        super.onCreate(state);
        watchInsets();
    }

    /**
     * Reports the system bars, the display cutout and the soft keyboard to
     * the native side, at each change and at every step of the keyboard's
     * animation (Android 11 and later).
     */
    private void watchInsets() {
        View decor = getWindow().getDecorView();
        decor.setOnApplyWindowInsetsListener((view, insets) -> {
            // While the keyboard animates, the end state arrives here first;
            // the animation's own steps are reported instead.
            if (insetAnimations == 0) reportInsets(insets);
            return view.onApplyWindowInsets(insets);
        });
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            decor.setWindowInsetsAnimationCallback(new WindowInsetsAnimation.Callback(
                    WindowInsetsAnimation.Callback.DISPATCH_MODE_CONTINUE_ON_SUBTREE) {
                @Override public void onPrepare(WindowInsetsAnimation animation) {
                    insetAnimations++;
                }
                @Override public WindowInsets onProgress(WindowInsets insets,
                        List<WindowInsetsAnimation> running) {
                    reportInsets(insets);
                    return insets;
                }
                @Override public void onEnd(WindowInsetsAnimation animation) {
                    insetAnimations = Math.max(0, insetAnimations - 1);
                    WindowInsets settled = decor.getRootWindowInsets();
                    if (insetAnimations == 0 && settled != null) reportInsets(settled);
                }
            });
        }
    }

    /** Inset animations running now (UI thread only). */
    private int insetAnimations;

    private void reportInsets(WindowInsets insets) {
        float density = getResources().getDisplayMetrics().density;
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            Insets safe = insets.getInsets(
                    WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
            Insets ime = insets.getInsets(WindowInsets.Type.ime());
            nativeInsets(density, safe.left, safe.top, safe.right, safe.bottom,
                    ime.left, ime.top, ime.right, ime.bottom);
        } else {
            // Before Android 11 the system window insets include the keyboard;
            // the stable insets are the bars alone.
            @SuppressWarnings("deprecation") int keyboard = Math.max(0,
                    insets.getSystemWindowInsetBottom() - insets.getStableInsetBottom());
            @SuppressWarnings("deprecation") int left = insets.getStableInsetLeft();
            @SuppressWarnings("deprecation") int top = insets.getStableInsetTop();
            @SuppressWarnings("deprecation") int right = insets.getStableInsetRight();
            @SuppressWarnings("deprecation") int bottom = insets.getStableInsetBottom();
            nativeInsets(density, left, top, right, bottom, 0, 0, 0,
                    keyboard > 0 ? keyboard + bottom : 0);
        }
    }

    /** Memory pressure reaches GPUI as a memory warning. */
    @Override public void onTrimMemory(int level) {
        super.onTrimMemory(level);
        nativeTrimMemory(level);
    }

    private static native void nativeTrimMemory(int level);

    private static native void nativeInsets(float density, int safeLeft, int safeTop,
            int safeRight, int safeBottom, int imeLeft, int imeTop, int imeRight, int imeBottom);

    public void gpuiShowKeyboard(int keyboardType, long session) {
        runOnUiThread(() -> {
            if (input == null) {
                input = new InputProxy();
                input.setAlpha(0f);
                input.setPadding(0, 0, 0, 0);
                addContentView(input, new ViewGroup.LayoutParams(1, 1));
            }
            input.reset(session);
            shownKeyboardType = keyboardType;
            applyKeyboardConfiguration();
            input.requestFocus();
            InputMethodManager imm = (InputMethodManager) getSystemService(INPUT_METHOD_SERVICE);
            imm.restartInput(input);
            imm.showSoftInput(input, InputMethodManager.SHOW_IMPLICIT);
        });
    }

    /** The focused field's keyboard type, kept for a reconfiguration. */
    private int shownKeyboardType;
    /** {@code EditorInfo.IME_ACTION_*} the focused field asked for; 0 for none. */
    private int imeAction;
    /** {@code InputType.TYPE_TEXT_FLAG_CAP_*} the focused field asked for. */
    private int capitalize;
    private boolean autocorrect;
    private boolean suggestions;

    /**
     * The focused field's text assistance and action key (GPUI's
     * TextInputConfiguration); applied at once if the keyboard is up.
     */
    public void gpuiConfigureKeyboard(int action, int capitalizeFlags, boolean correct,
            boolean suggest) {
        runOnUiThread(() -> {
            imeAction = action;
            capitalize = capitalizeFlags;
            autocorrect = correct;
            suggestions = suggest;
            if (input != null && input.hasFocus()) {
                applyKeyboardConfiguration();
                ((InputMethodManager) getSystemService(INPUT_METHOD_SERVICE)).restartInput(input);
            }
        });
    }

    private void applyKeyboardConfiguration() {
        int type;
        switch (shownKeyboardType) {
            case 1: type = InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS; break;
            case 2: type = InputType.TYPE_CLASS_PHONE; break;
            case 3: type = InputType.TYPE_CLASS_NUMBER; break;
            case 4: type = InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI; break;
            case 5: type = InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_FLAG_DECIMAL; break;
            default:
                type = InputType.TYPE_CLASS_TEXT | capitalize;
                // A field with an action key gets that key; without one the
                // enter key types a line break.
                if (imeAction == 0) type |= InputType.TYPE_TEXT_FLAG_MULTI_LINE;
                if (autocorrect) type |= InputType.TYPE_TEXT_FLAG_AUTO_CORRECT;
                if (!suggestions && !autocorrect) type |= InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS;
        }
        input.setInputType(type);
        input.setImeOptions(EditorInfo.IME_FLAG_NO_EXTRACT_UI | imeAction);
        android.util.Log.d("GpuiInputActivity", "keyboard: inputType=0x"
                + Integer.toHexString(type) + " action=" + imeAction);
    }

    public void gpuiHideKeyboard(long session) {
        runOnUiThread(() -> {
            if (input == null) return;
            input.reset(session);
            InputMethodManager imm = (InputMethodManager) getSystemService(INPUT_METHOD_SERVICE);
            imm.hideSoftInputFromWindow(input.getWindowToken(), 0);
            input.clearFocus();
        });
    }

    public void gpuiResetComposition(long session) {
        runOnUiThread(() -> {
            if (input == null) return;
            input.reset(session);
            ((InputMethodManager) getSystemService(INPUT_METHOD_SERVICE)).restartInput(input);
        });
    }

    private final class InputProxy extends EditText {
        private long session;
        private int depth;
        private boolean marked;

        InputProxy() {
            super(GpuiInputActivity.this);
            addTextChangedListener(new TextWatcher() {
                public void beforeTextChanged(CharSequence s, int start, int count, int after) {}
                public void onTextChanged(CharSequence s, int start, int before, int count) {}
                public void afterTextChanged(Editable text) {
                    // Hardware keyboards edit the widget directly, outside its
                    // InputConnection. IME mutations are batched by depth below.
                    if (depth == 0) { depth++; endEdit(); }
                }
            });
        }

        @Override public boolean onKeyDown(int code, KeyEvent event) {
            if (code == KeyEvent.KEYCODE_DEL && getText().length() == 0 && !marked) {
                nativeIme(session, 3, "", 1, 0);
                return true;
            }
            return super.onKeyDown(code, event);
        }

        void reset(long nextSession) {
            depth++;
            getText().clear();
            marked = false;
            session = nextSession;
            depth = 0;
        }

        private void endEdit() {
            if (--depth != 0) return;
            Editable text = getText();
            boolean composing = BaseInputConnection.getComposingSpanStart(text) >= 0;
            if (composing || marked || text.length() > 0) {
                nativeIme(session, composing ? 0 : 1, text.toString(),
                        Math.max(0, Selection.getSelectionStart(text)),
                        Math.max(0, Selection.getSelectionEnd(text)));
                marked = composing;
                if (!composing) {
                    depth++;
                    text.clear();
                    depth--;
                }
            }
        }

        @Override public boolean onKeyPreIme(int code, KeyEvent event) {
            if (code == KeyEvent.KEYCODE_BACK && event.getAction() == KeyEvent.ACTION_UP) {
                nativeIme(session, 5, "", 0, 0);
            }
            return super.onKeyPreIme(code, event);
        }

        @Override public InputConnection onCreateInputConnection(EditorInfo info) {
            InputConnection connection = super.onCreateInputConnection(info);
            if (connection == null) return null;
            final long connectionSession = session;
            return new InputConnectionWrapper(connection, false) {
                @Override public boolean beginBatchEdit() {
                    if (connectionSession != session) return false;
                    depth++;
                    return super.beginBatchEdit();
                }
                @Override public boolean endBatchEdit() {
                    if (connectionSession != session) return false;
                    boolean result = super.endBatchEdit();
                    if (depth > 0) endEdit();
                    return result;
                }
                @Override public boolean setComposingText(CharSequence text, int cursor) {
                    if (connectionSession != session) return false;
                    depth++;
                    try { return super.setComposingText(text, cursor); }
                    finally { endEdit(); }
                }
                @Override public boolean setComposingRegion(int start, int end) {
                    if (connectionSession != session) return false;
                    depth++;
                    try { return super.setComposingRegion(start, end); }
                    finally { endEdit(); }
                }
                @Override public boolean finishComposingText() {
                    if (connectionSession != session) return false;
                    depth++;
                    try { return super.finishComposingText(); }
                    finally { endEdit(); }
                }
                @Override public boolean commitText(CharSequence text, int cursor) {
                    if (connectionSession != session) return false;
                    depth++;
                    try { return super.commitText(text, cursor); }
                    finally { endEdit(); }
                }
                @Override public boolean setSelection(int start, int end) {
                    if (connectionSession != session) return false;
                    depth++;
                    try { return super.setSelection(start, end); }
                    finally { endEdit(); }
                }
                @Override public boolean deleteSurroundingText(int before, int after) {
                    if (connectionSession != session) return false;
                    if (getText().length() == 0 && !marked) {
                        nativeIme(session, 2, "", before, after);
                        return true;
                    }
                    depth++;
                    try { return super.deleteSurroundingText(before, after); }
                    finally { endEdit(); }
                }
                @Override public boolean deleteSurroundingTextInCodePoints(int before, int after) {
                    if (connectionSession != session) return false;
                    if (getText().length() == 0 && !marked) {
                        nativeIme(session, 3, "", before, after);
                        return true;
                    }
                    depth++;
                    try { return super.deleteSurroundingTextInCodePoints(before, after); }
                    finally { endEdit(); }
                }
                @Override public boolean sendKeyEvent(KeyEvent event) {
                    if (connectionSession != session) return false;
                    if (event.getKeyCode() == KeyEvent.KEYCODE_DEL) {
                        if (event.getAction() == KeyEvent.ACTION_DOWN) deleteSurroundingText(1, 0);
                        return true;
                    }
                    if (event.getKeyCode() == KeyEvent.KEYCODE_ENTER) {
                        if (event.getAction() == KeyEvent.ACTION_DOWN) commitText("\n", 1);
                        return true;
                    }
                    return super.sendKeyEvent(event);
                }
                @Override public boolean performEditorAction(int action) {
                    if (connectionSession != session) return false;
                    if (imeAction != 0) {
                        // The action key the field asked for: GPUI gets enter.
                        finishComposingText();
                        nativeIme(session, 6, "", action, 0);
                        return true;
                    }
                    if (action == EditorInfo.IME_ACTION_DONE) {
                        finishComposingText();
                        nativeIme(session, 4, "", 0, 0);
                        return true;
                    }
                    return commitText("\n", 1);
                }
            };
        }
    }

    private static native void nativeIme(long session, int kind, String text, int start, int end);
}
