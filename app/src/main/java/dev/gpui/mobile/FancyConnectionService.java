package dev.gpui.mobile;

import android.Manifest;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.content.pm.ServiceInfo;
import android.graphics.drawable.Icon;
import android.os.Build;
import android.os.IBinder;
import android.os.PowerManager;

/**
 * Keeps the process alive while the app holds a voice connection: a
 * foreground service with an ongoing notification (title and text from the
 * app, a Disconnect action) and a partial wake lock. Without it Android
 * suspends a backgrounded app within minutes and the connection drops.
 *
 * <p>From Android 14 a service may claim the microphone type only while
 * RECORD_AUDIO is granted (claiming it without throws), so the type is
 * chosen when the service starts and again on {@link #refresh}, after the
 * app was granted the microphone.</p>
 */
public final class FancyConnectionService extends Service {
    private static final String CHANNEL_ID = "connection";
    private static final int NOTIFICATION_ID = 1001;
    private static final String EXTRA_TITLE = "title";
    private static final String EXTRA_TEXT = "text";
    private static final String ACTION_DISCONNECT = "dev.gpui.mobile.DISCONNECT";

    private static volatile boolean running;
    private PowerManager.WakeLock wakeLock;
    private String title = "";
    private String text = "";

    /** Starts the service, or updates its notification if it runs. */
    public static void start(Context context, String title, String text) {
        Intent intent = new Intent(context, FancyConnectionService.class)
                .putExtra(EXTRA_TITLE, title).putExtra(EXTRA_TEXT, text);
        context.startForegroundService(intent);
    }

    /** Claims the foreground again with the types now allowed. */
    public static void refresh(Context context) {
        if (running) start(context, null, null);
    }

    public static void stop(Context context) {
        context.stopService(new Intent(context, FancyConnectionService.class));
    }

    public static boolean isRunning() {
        return running;
    }

    @Override public IBinder onBind(Intent intent) {
        return null;
    }

    @Override public void onCreate() {
        super.onCreate();
        running = true;
        {
            NotificationChannel channel = new NotificationChannel(CHANNEL_ID,
                    "Connection", NotificationManager.IMPORTANCE_LOW);
            channel.setDescription("Shown while connected to a server");
            channel.setShowBadge(false);
            getSystemService(NotificationManager.class).createNotificationChannel(channel);
        }
        PowerManager power = (PowerManager) getSystemService(Context.POWER_SERVICE);
        wakeLock = power.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "FancyMumble::Connection");
        wakeLock.acquire();
    }

    @Override public void onDestroy() {
        running = false;
        if (wakeLock != null && wakeLock.isHeld()) wakeLock.release();
        super.onDestroy();
    }

    @Override public void onTaskRemoved(Intent rootIntent) {
        super.onTaskRemoved(rootIntent);
        stopSelf();
    }

    @Override public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent != null && ACTION_DISCONNECT.equals(intent.getAction())) {
            nativeDisconnectRequested();
            return START_NOT_STICKY;
        }
        if (intent != null && intent.getStringExtra(EXTRA_TITLE) != null) {
            title = intent.getStringExtra(EXTRA_TITLE);
            text = intent.getStringExtra(EXTRA_TEXT);
        }
        Notification notification = notification();
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            int microphone = checkSelfPermission(Manifest.permission.RECORD_AUDIO)
                    == PackageManager.PERMISSION_GRANTED
                    ? ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE : 0;
            startForeground(NOTIFICATION_ID, notification,
                    ServiceInfo.FOREGROUND_SERVICE_TYPE_REMOTE_MESSAGING | microphone);
        } else {
            startForeground(NOTIFICATION_ID, notification);
        }
        return START_NOT_STICKY;
    }

    private Notification notification() {
        Intent open = getPackageManager().getLaunchIntentForPackage(getPackageName());
        PendingIntent content = PendingIntent.getActivity(this, 0, open,
                PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        PendingIntent disconnect = PendingIntent.getService(this, 1,
                new Intent(this, FancyConnectionService.class).setAction(ACTION_DISCONNECT),
                PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        return new Notification.Builder(this, CHANNEL_ID)
                .setContentTitle(title)
                .setContentText(text)
                .setSmallIcon(android.R.drawable.ic_lock_silent_mode_off)
                .setContentIntent(content)
                .addAction(new Notification.Action.Builder(
                        Icon.createWithResource(this, android.R.drawable.ic_menu_close_clear_cancel),
                        "Disconnect", disconnect).build())
                .setOngoing(true)
                .build();
    }

    private static native void nativeDisconnectRequested();
}
