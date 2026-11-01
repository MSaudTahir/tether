package io.github.msaudtahir.tether.buddy;

import android.service.notification.NotificationListenerService;

/** Receives the phone's notifications once Tether enables it (cmd notification allow_listener). */
public final class Listener extends NotificationListenerService {
    static volatile Listener connected;

    @Override
    public void onListenerConnected() {
        connected = this;
    }

    @Override
    public void onListenerDisconnected() {
        connected = null;
    }
}
