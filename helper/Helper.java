package tether;

import android.app.Application;
import android.content.Context;
import android.content.ContextWrapper;
import android.content.Intent;
import android.content.pm.ApplicationInfo;
import android.content.pm.PackageManager;
import android.content.pm.ResolveInfo;
import android.content.res.Resources;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.drawable.Drawable;
import android.os.Looper;
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
 * Runs on the device via app_process (as the shell user) and prints one line per launchable app:
 * package \t system (0/1) \t label \t base64 PNG icon
 */
public final class Helper {
    private static final int ICON_SIZE = 96;

    public static void main(String[] args) throws Exception {
        PackageManager pm = systemContext().getPackageManager();
        Intent launcher = new Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER);
        PrintStream out = new PrintStream(System.out, false, "UTF-8");
        Set<String> seen = new HashSet<>();
        for (ResolveInfo info : pm.queryIntentActivities(launcher, 0)) {
            ApplicationInfo app = info.activityInfo.applicationInfo;
            if (!seen.add(app.packageName)) {
                continue;
            }
            String label = info.loadLabel(pm).toString().replaceAll("\\s+", " ").trim();
            boolean system = (app.flags & ApplicationInfo.FLAG_SYSTEM) != 0;
            out.println(app.packageName + "\t" + (system ? 1 : 0) + "\t" + label + "\t" + icon(info, pm));
        }
        out.flush();
    }

    private static String icon(ResolveInfo info, PackageManager pm) {
        try {
            // Read the icon straight from the app's resources: ResolveInfo.loadIcon() goes through OEM
            // theming hooks (e.g. MIUI's) that make permission checks the shell user fails.
            Resources resources = pm.getResourcesForApplication(info.activityInfo.applicationInfo);
            Drawable drawable = resources.getDrawableForDensity(
                    info.getIconResource(), DisplayMetrics.DENSITY_XXHIGH, null);
            Bitmap bitmap = Bitmap.createBitmap(ICON_SIZE, ICON_SIZE, Bitmap.Config.ARGB_8888);
            drawable.setBounds(0, 0, ICON_SIZE, ICON_SIZE);
            drawable.draw(new Canvas(bitmap));
            ByteArrayOutputStream png = new ByteArrayOutputStream();
            bitmap.compress(Bitmap.CompressFormat.PNG, 100, png);
            return Base64.encodeToString(png.toByteArray(), Base64.NO_WRAP);
        } catch (Exception e) {
            System.err.println(info.activityInfo.packageName + ": " + e);
            return ""; // keep the app, just without an icon
        }
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
