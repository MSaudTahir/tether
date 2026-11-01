package io.github.msaudtahir.tether.buddy;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.database.Cursor;
import android.net.Uri;
import android.os.Binder;
import android.os.Bundle;
import android.os.Process;
import android.service.notification.StatusBarNotification;

/**
 * Tether's way in, called by its on-device helper (running as adb's shell):
 *   "intent" (arg: notification key) → { "intent": the notification's content PendingIntent }
 */
public final class Provider extends ContentProvider {
    private static final int SHELL_UID = 2000;

    @Override
    public Bundle call(String method, String arg, Bundle extras) {
        int caller = Binder.getCallingUid();
        if (caller != SHELL_UID && caller != Process.ROOT_UID) {
            throw new SecurityException("Only adb can use Tether Buddy");
        }
        Listener listener = Listener.connected;
        Bundle result = new Bundle();
        result.putBoolean("listening", listener != null);
        if (listener == null) {
            return result;
        }
        if ("intent".equals(method)) {
            for (StatusBarNotification sbn : listener.getActiveNotifications()) {
                if (sbn.getKey().equals(arg)) {
                    result.putParcelable("intent", sbn.getNotification().contentIntent);
                }
            }
        }
        return result;
    }

    @Override public boolean onCreate() { return true; }
    @Override public Cursor query(Uri uri, String[] p, String s, String[] a, String o) { return null; }
    @Override public String getType(Uri uri) { return null; }
    @Override public Uri insert(Uri uri, ContentValues values) { return null; }
    @Override public int delete(Uri uri, String s, String[] a) { return 0; }
    @Override public int update(Uri uri, ContentValues v, String s, String[] a) { return 0; }
}
