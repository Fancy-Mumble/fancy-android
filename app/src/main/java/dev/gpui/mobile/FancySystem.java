package dev.gpui.mobile;

import android.Manifest;
import android.app.Activity;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.ContentResolver;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.database.Cursor;
import android.net.Uri;
import android.os.Build;
import android.provider.OpenableColumns;
import android.util.Log;

import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.ArrayList;
import java.util.UUID;

/**
 * The system services a chat client asks of the host, called from Rust
 * (gpui_mobile's android modules) through GpuiInputActivity: runtime
 * permissions, message notifications, and files other apps share in.
 */
public final class FancySystem {
    private static final String TAG = "FancySystem";
    private static final String MESSAGES_CHANNEL = "messages";

    private FancySystem() {}

    // ── permissions ─────────────────────────────────────────────────────

    /** Whether {@code permission} is granted (POST_NOTIFICATIONS is implicit before 13). */
    public static boolean isGranted(Context context, String permission) {
        if (Manifest.permission.POST_NOTIFICATIONS.equals(permission)
                && Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
            return true;
        }
        return context.checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED;
    }

    /** Asks for {@code permission}; the answer reaches Rust with {@code code}. */
    public static void request(Activity activity, String permission, int code) {
        if (isGranted(activity, permission)) {
            nativePermissionResult(code, true);
            return;
        }
        activity.runOnUiThread(() -> activity.requestPermissions(new String[] {permission}, code));
    }

    /** The answer to {@link #request}, from the activity's callback. */
    static void onResult(Activity activity, int code, String[] permissions, int[] results) {
        boolean granted = results.length > 0 && results[0] == PackageManager.PERMISSION_GRANTED;
        nativePermissionResult(code, granted);
        if (granted && permissions.length > 0
                && Manifest.permission.RECORD_AUDIO.equals(permissions[0])) {
            // A running connection service may now claim the microphone.
            FancyConnectionService.refresh(activity);
        }
    }

    // ── notifications ───────────────────────────────────────────────────

    /** Posts (or replaces) message notification {@code id}; a tap opens the app. */
    public static void post(Context context, int id, String title, String body) {
        NotificationManager manager = context.getSystemService(NotificationManager.class);
        manager.createNotificationChannel(new NotificationChannel(MESSAGES_CHANNEL,
                "Messages", NotificationManager.IMPORTANCE_DEFAULT));
        Intent open = context.getPackageManager().getLaunchIntentForPackage(context.getPackageName());
        PendingIntent content = PendingIntent.getActivity(context, id, open,
                PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        Notification notification = new Notification.Builder(context, MESSAGES_CHANNEL)
                .setContentTitle(title)
                .setContentText(body)
                .setSmallIcon(android.R.drawable.stat_notify_chat)
                .setContentIntent(content)
                .setAutoCancel(true)
                .build();
        manager.notify(id, notification);
    }

    public static void cancel(Context context, int id) {
        context.getSystemService(NotificationManager.class).cancel(id);
    }

    // ── files shared in ─────────────────────────────────────────────────

    /**
     * Hands Rust what an ACTION_SEND / ACTION_SEND_MULTIPLE intent carries:
     * its text, and its streams copied into the cache (a content URI's
     * permission ends with the intent), on a worker thread.
     */
    static void receive(Context context, Intent intent) {
        if (intent == null) return;
        String action = intent.getAction();
        if (!Intent.ACTION_SEND.equals(action) && !Intent.ACTION_SEND_MULTIPLE.equals(action)) {
            return;
        }
        String text = intent.getStringExtra(Intent.EXTRA_TEXT);
        ArrayList<Uri> uris = new ArrayList<>();
        if (Intent.ACTION_SEND.equals(action)) {
            Uri uri = stream(intent);
            if (uri != null) uris.add(uri);
        } else {
            ArrayList<Uri> many = streams(intent);
            if (many != null) uris.addAll(many);
        }
        // Handled once: a recreated activity must not receive it again.
        intent.setAction(null);
        new Thread(() -> {
            ArrayList<String> paths = new ArrayList<>();
            File directory = new File(context.getCacheDir(), "shared-" + UUID.randomUUID());
            for (Uri uri : uris) {
                try {
                    paths.add(copy(context.getContentResolver(), uri, directory).getAbsolutePath());
                } catch (Exception error) {
                    Log.w(TAG, "could not copy shared " + uri, error);
                }
            }
            nativeShared(paths.toArray(new String[0]), text == null ? "" : text);
        }, "fancy-share-in").start();
    }

    private static Uri stream(Intent intent) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            return intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri.class);
        }
        @SuppressWarnings("deprecation") Uri uri = intent.getParcelableExtra(Intent.EXTRA_STREAM);
        return uri;
    }

    private static ArrayList<Uri> streams(Intent intent) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            return intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM, Uri.class);
        }
        @SuppressWarnings("deprecation") ArrayList<Uri> uris =
                intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM);
        return uris;
    }

    private static File copy(ContentResolver resolver, Uri uri, File directory) throws Exception {
        String name = "shared";
        try (Cursor cursor = resolver.query(uri,
                new String[] {OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (cursor != null && cursor.moveToFirst() && !cursor.isNull(0)) {
                name = new File(cursor.getString(0)).getName();
            }
        } catch (Exception ignored) {
            // Not every provider answers the query; the name stays generic.
        }
        if (!directory.isDirectory() && !directory.mkdirs()) {
            throw new IllegalStateException("cannot create " + directory);
        }
        File target = new File(directory, name);
        try (InputStream in = resolver.openInputStream(uri);
             OutputStream out = new FileOutputStream(target)) {
            if (in == null) throw new IllegalStateException("no stream for " + uri);
            byte[] buffer = new byte[64 * 1024];
            for (int read; (read = in.read(buffer)) > 0; ) out.write(buffer, 0, read);
        }
        return target;
    }

    private static native void nativePermissionResult(int code, boolean granted);
    private static native void nativeShared(String[] paths, String text);
}
