package tether;

import android.app.ActivityOptions;
import android.app.Application;
import android.app.PendingIntent;
import android.content.ClipData;
import android.content.Context;
import android.content.ContextWrapper;
import android.content.Intent;
import android.content.pm.ApplicationInfo;
import android.content.pm.PackageManager;
import android.content.pm.ResolveInfo;
import android.content.res.Resources;
import android.os.Binder;
import android.os.Build;
import android.os.Bundle;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Matrix;
import android.media.ExifInterface;
import android.media.ThumbnailUtils;
import android.provider.MediaStore;
import android.graphics.Canvas;
import android.graphics.drawable.Drawable;
import android.os.IBinder;
import android.os.Looper;
import android.os.Process;
import android.util.Base64;
import android.util.DisplayMetrics;

import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.HashSet;
import java.util.Set;

/**
 * Runs on the device via app_process (as the shell user).
 *
 * With no arguments, prints one line per launchable app: package \t system (0/1) \t label \t base64 PNG icon
 * With "packages p1 p2 ...", prints the same for those packages (e.g. system apps that post
 * notifications but can't be launched); unknown packages are skipped.
 * With "clipboard", prints the clipboard's text as base64 (empty if there's none).
 * With "clipboard BASE64", sets the clipboard to that text.
 * With "thumbs PATH...", prints "path \t base64 JPEG" thumbnails of those photos and videos.
 * With "notification KEY", runs the notification's action (from Tether Buddy) as the shell;
 * prints "ok", "no buddy" (not installed or not enabled) or "gone".
 */
public final class Helper {
    private static final int ICON_SIZE = 96;

    public static void main(String[] args) throws Exception {
        if (args.length > 0 && args[0].equals("thumbs")) {
            systemContext(); // image decoding looks up the current app (at least on MIUI)
            PrintStream out = new PrintStream(System.out, false, "UTF-8");
            for (int i = 1; i < args.length; i++) {
                out.println(args[i] + "\t" + thumbnail(args[i]));
            }
            out.flush();
            return;
        }
        if (args.length > 1 && args[0].equals("notification")) {
            openNotification(args[1]);
            return;
        }
        if (args.length > 0 && args[0].equals("clipboard")) {
            clipboard(args.length > 1 ? args[1] : null);
            return;
        }
        PackageManager pm = systemContext().getPackageManager();
        PrintStream out = new PrintStream(System.out, false, "UTF-8");
        if (args.length > 0 && args[0].equals("packages")) {
            for (int i = 1; i < args.length; i++) {
                try {
                    ApplicationInfo app = pm.getApplicationInfo(args[i], 0);
                    print(out, app, app.loadLabel(pm).toString(), app.icon, pm);
                } catch (PackageManager.NameNotFoundException e) {
                    System.err.println(args[i] + ": not installed");
                }
            }
            out.flush();
            return;
        }
        Intent launcher = new Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER);
        Set<String> seen = new HashSet<>();
        for (ResolveInfo info : pm.queryIntentActivities(launcher, 0)) {
            ApplicationInfo app = info.activityInfo.applicationInfo;
            if (seen.add(app.packageName)) {
                print(out, app, info.loadLabel(pm).toString(), info.getIconResource(), pm);
            }
        }
        out.flush();
    }

    private static void print(PrintStream out, ApplicationInfo app, String label, int iconResource, PackageManager pm) {
        boolean system = (app.flags & ApplicationInfo.FLAG_SYSTEM) != 0;
        label = label.replaceAll("\\s+", " ").trim();
        out.println(app.packageName + "\t" + (system ? 1 : 0) + "\t" + label + "\t" + icon(app, iconResource, pm));
    }

    private static String icon(ApplicationInfo app, int iconResource, PackageManager pm) {
        try {
            // Read the icon straight from the app's resources: ResolveInfo.loadIcon() goes through OEM
            // theming hooks (e.g. MIUI's) that make permission checks the shell user fails.
            Resources resources = pm.getResourcesForApplication(app);
            Drawable drawable = iconResource != 0
                    ? resources.getDrawableForDensity(iconResource, DisplayMetrics.DENSITY_XXHIGH, null)
                    : pm.getDefaultActivityIcon();
            Bitmap bitmap = Bitmap.createBitmap(ICON_SIZE, ICON_SIZE, Bitmap.Config.ARGB_8888);
            drawable.setBounds(0, 0, ICON_SIZE, ICON_SIZE);
            drawable.draw(new Canvas(bitmap));
            ByteArrayOutputStream png = new ByteArrayOutputStream();
            bitmap.compress(Bitmap.CompressFormat.PNG, 100, png);
            return Base64.encodeToString(png.toByteArray(), Base64.NO_WRAP);
        } catch (Exception e) {
            System.err.println(app.packageName + ": " + e);
            return ""; // keep the app, just without an icon
        }
    }

    private static final int THUMB_SIZE = 320;

    /** A JPEG thumbnail (longest side THUMB_SIZE, turned upright) as base64, or "" if it can't be made. */
    private static String thumbnail(String path) {
        try {
            Bitmap bitmap;
            String lower = path.toLowerCase();
            if (lower.endsWith(".mp4") || lower.endsWith(".mkv") || lower.endsWith(".webm") || lower.endsWith(".3gp") || lower.endsWith(".mov")) {
                bitmap = ThumbnailUtils.createVideoThumbnail(path, MediaStore.Images.Thumbnails.MINI_KIND);
            } else {
                BitmapFactory.Options bounds = new BitmapFactory.Options();
                bounds.inJustDecodeBounds = true;
                BitmapFactory.decodeFile(path, bounds);
                BitmapFactory.Options options = new BitmapFactory.Options();
                options.inSampleSize = 1;
                while (Math.max(bounds.outWidth, bounds.outHeight) / (options.inSampleSize * 2) >= THUMB_SIZE) {
                    options.inSampleSize *= 2;
                }
                bitmap = BitmapFactory.decodeFile(path, options);
                if (bitmap != null) {
                    int rotation = new ExifInterface(path).getAttributeInt(ExifInterface.TAG_ORIENTATION, ExifInterface.ORIENTATION_NORMAL);
                    int degrees = rotation == ExifInterface.ORIENTATION_ROTATE_90 ? 90
                            : rotation == ExifInterface.ORIENTATION_ROTATE_180 ? 180
                            : rotation == ExifInterface.ORIENTATION_ROTATE_270 ? 270 : 0;
                    if (degrees != 0) {
                        Matrix matrix = new Matrix();
                        matrix.postRotate(degrees);
                        bitmap = Bitmap.createBitmap(bitmap, 0, 0, bitmap.getWidth(), bitmap.getHeight(), matrix, true);
                    }
                }
            }
            if (bitmap == null) {
                return "";
            }
            float scale = Math.min(1f, THUMB_SIZE / (float) Math.max(bitmap.getWidth(), bitmap.getHeight()));
            if (scale < 1f) {
                bitmap = Bitmap.createScaledBitmap(bitmap, Math.round(bitmap.getWidth() * scale), Math.round(bitmap.getHeight() * scale), true);
            }
            ByteArrayOutputStream jpeg = new ByteArrayOutputStream();
            bitmap.compress(Bitmap.CompressFormat.JPEG, 80, jpeg);
            return Base64.encodeToString(jpeg.toByteArray(), Base64.NO_WRAP);
        } catch (Exception e) {
            System.err.println(path + ": " + e);
            return "";
        }
    }

    private static final String SHELL = "com.android.shell";
    private static final String BUDDY = "io.github.msaudtahir.tether.buddy";

    /**
     * Calls Tether Buddy's provider the way the `content` shell command does: this process isn't a
     * registered app, so a ContentResolver would get it killed. The binder methods vary by version.
     */
    private static Bundle callBuddy(String method, String arg) throws Exception {
        Object am = Class.forName("android.app.ActivityManager").getMethod("getService").invoke(null);
        IBinder token = new Binder();
        Object holder;
        try {
            holder = am.getClass().getMethod("getContentProviderExternal", String.class, int.class, IBinder.class, String.class)
                    .invoke(am, BUDDY, 0, token, "tether");
        } catch (NoSuchMethodException e) {
            holder = am.getClass().getMethod("getContentProviderExternal", String.class, int.class, IBinder.class)
                    .invoke(am, BUDDY, 0, token);
        }
        if (holder == null) {
            return null; // not installed
        }
        Object provider = holder.getClass().getField("provider").get(holder);
        try {
            if (Build.VERSION.SDK_INT >= 31) {
                Class<?> source = Class.forName("android.content.AttributionSource");
                Object attribution = source.getConstructor(int.class, String.class, String.class).newInstance(Process.myUid(), SHELL, null);
                return (Bundle) provider.getClass().getMethod("call", source, String.class, String.class, String.class, Bundle.class)
                        .invoke(provider, attribution, BUDDY, method, arg, null);
            } else if (Build.VERSION.SDK_INT == 30) {
                return (Bundle) provider.getClass().getMethod("call", String.class, String.class, String.class, String.class, String.class, Bundle.class)
                        .invoke(provider, SHELL, null, BUDDY, method, arg, null);
            } else if (Build.VERSION.SDK_INT == 29) {
                return (Bundle) provider.getClass().getMethod("call", String.class, String.class, String.class, String.class, Bundle.class)
                        .invoke(provider, SHELL, BUDDY, method, arg, null);
            }
            return (Bundle) provider.getClass().getMethod("call", String.class, String.class, String.class, Bundle.class)
                    .invoke(provider, SHELL, method, arg, null);
        } finally {
            am.getClass().getMethod("removeContentProviderExternal", String.class, IBinder.class).invoke(am, BUDDY, token);
        }
    }

    /**
     * Gets the notification's PendingIntent from Tether Buddy and sends it from here, as the shell,
     * which may start activities from the background (Buddy, running in the background, may not).
     */
    private static void openNotification(String key) throws Exception {
        Context shell = systemContext();
        Bundle result = callBuddy("intent", key);
        if (result == null || !result.getBoolean("listening")) {
            System.out.println("no buddy");
            return;
        }
        PendingIntent intent = result.getParcelable("intent");
        if (intent == null) {
            System.out.println("gone");
            return;
        }
        ActivityOptions options = ActivityOptions.makeBasic();
        try {
            // Android 14+: the sender has to opt in to lending its background-start privilege.
            ActivityOptions.class.getMethod("setPendingIntentBackgroundActivityStartMode", int.class).invoke(options, 1);
        } catch (NoSuchMethodException ignored) {
        }
        intent.send(shell, 0, null, null, null, null, options.toBundle());
        System.out.println("ok");
    }

    /**
     * Reads or sets the clipboard through the clipboard service as the shell, which may read it in
     * the background. The service's methods changed over Android versions, so try each form
     * (newest last), as scrcpy does.
     */
    private static void clipboard(String setBase64) throws Exception {
        Class<?> serviceManager = Class.forName("android.os.ServiceManager");
        IBinder binder = (IBinder) serviceManager.getMethod("getService", String.class).invoke(null, "clipboard");
        Object service = Class.forName("android.content.IClipboard$Stub").getMethod("asInterface", IBinder.class).invoke(null, binder);
        if (setBase64 == null) {
            ClipData clip = (ClipData) call(service, "getPrimaryClip");
            CharSequence text = clip == null || clip.getItemCount() == 0 ? null : clip.getItemAt(0).getText();
            byte[] bytes = text == null ? new byte[0] : text.toString().getBytes("UTF-8");
            System.out.println(Base64.encodeToString(bytes, Base64.NO_WRAP));
        } else {
            String text = new String(Base64.decode(setBase64, Base64.DEFAULT), "UTF-8");
            call(service, "setPrimaryClip", ClipData.newPlainText(null, text));
            System.out.println("ok");
        }
    }

    /** Calls `name` with the clip (if any), then the shell package, then whatever else this Android version wants. */
    private static Object call(Object service, String name, Object... clip) throws Exception {
        for (Method method : service.getClass().getMethods()) {
            if (!method.getName().equals(name)) {
                continue;
            }
            Class<?>[] types = method.getParameterTypes();
            Object[] args = new Object[types.length];
            int i = 0;
            for (Object c : clip) {
                args[i++] = c;
            }
            if (i < types.length && types[i] == String.class) {
                args[i++] = SHELL; // calling package
            }
            for (; i < types.length; i++) {
                if (types[i] == String.class) {
                    args[i] = null; // attribution tag
                } else if (types[i] == int.class) {
                    args[i] = 0; // user id, then device id
                } else if (types[i] == boolean.class) {
                    args[i] = false;
                } else {
                    args[i] = null;
                }
            }
            return method.invoke(service, args);
        }
        throw new NoSuchMethodException(name);
    }

    /**
     * app_process doesn't give us a Context, so borrow the system one (the same trick scrcpy uses).
     * Adaptive icons also need a current Application, so attach a stub one.
     */
    private static Context systemContext() throws Exception {
        Looper.prepareMainLooper();
        Class<?> cls = Class.forName("android.app.ActivityThread");
        Constructor<?> constructor = cls.getDeclaredConstructor();
        constructor.setAccessible(true);
        Object thread = constructor.newInstance();
        Field current = cls.getDeclaredField("sCurrentActivityThread");
        current.setAccessible(true);
        current.set(null, thread);
        Context system = (Context) cls.getDeclaredMethod("getSystemContext").invoke(thread);
        // We run as the shell user, so permission checks must see the shell package, not "android".
        Context context = new ContextWrapper(system) {
            @Override
            public String getPackageName() {
                return "com.android.shell";
            }

            @Override
            public String getOpPackageName() {
                return "com.android.shell";
            }
        };

        Application application = new Application();
        Method attach = ContextWrapper.class.getDeclaredMethod("attachBaseContext", Context.class);
        attach.setAccessible(true);
        attach.invoke(application, context);
        Field initial = cls.getDeclaredField("mInitialApplication");
        initial.setAccessible(true);
        initial.set(thread, application);
        return context;
    }
}
