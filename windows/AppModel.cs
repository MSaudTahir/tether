using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using uniffi.tether_core;

namespace Tether;

/// App state. Core calls block (adb, scrcpy, downloads), so they run on the thread pool via Task.Run;
/// everything else happens on the UI thread. Views listen to Changed and redraw.
sealed class AppModel
{
    public static readonly AppModel Shared = new();

    public readonly Core Core;
    public Config Config;
    public bool NeedsSetup;

    public Device[] Devices = [];
    public string? Selection;
    public readonly Dictionary<string, DeviceInfo> Info = [];
    public readonly Dictionary<string, DeviceApp[]> Apps = [];
    public readonly HashSet<string> LoadingApps = [];
    public readonly Dictionary<string, string> AppErrors = [];
    public readonly Dictionary<string, PhoneNotification[]> Notifications = [];
    public Release? Update;
    public bool InstallingUpdate;

    public event Action? Changed;
    /// An error to show the user.
    public event Action<string>? Alert;

    bool started;

    AppModel()
    {
        var dir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Tether");
        Core = new Core(dir);
        Config = Core.Config();
        NeedsSetup = Core.NeedsSetup();
    }

    public Device? SelectedDevice => Devices.FirstOrDefault(d => d.serial == Selection);

    void Notify() => Changed?.Invoke();

    /// Starts watching for devices and checks for a scrcpy update.
    public async void Start()
    {
        if (started) return;
        started = true;
        CheckForUpdate();
        for (var tick = 1; ; tick++)
        {
            await RefreshDevices();
            // Keep notifications, battery, memory etc. current for the device on screen.
            if (tick % 3 == 0 && Selection is { } selected) await LoadNotifications(selected);
            if (tick % 15 == 0 && Selection is { } serial) await LoadInfo(serial);
            await Task.Delay(2000);
        }
    }

    public async Task RefreshDevices()
    {
        Device[] devices;
        try { devices = await Task.Run(Core.Devices); }
        catch { return; }
        var changed = !devices.SequenceEqual(Devices);
        Devices = devices;
        if (Selection == null || !devices.Any(d => d.serial == Selection))
        {
            Selection = devices.FirstOrDefault()?.serial;
            changed = true;
        }
        if (changed) Notify();
        LoadSelection();
    }

    public void Select(string? serial)
    {
        if (serial == Selection) return;
        Selection = serial;
        Notify();
        LoadSelection();
    }

    /// The selected serial and state last loaded: a device is (re)loaded when selected or once it's authorized.
    string? loaded;

    void LoadSelection()
    {
        var key = SelectedDevice is { } device ? device.serial + device.state : null;
        if (key == loaded) return;
        loaded = key;
        if (Selection is { } serial) _ = Load(serial);
    }

    // --- Selected device ---------------------------------------------------

    /// Loads info and the (cached) app list for a device.
    public async Task Load(string serial)
    {
        if (Devices.FirstOrDefault(d => d.serial == serial)?.state != "device") return;
        await Task.WhenAll(LoadInfo(serial), LoadApps(serial, force: false));
        await LoadNotifications(serial); // after the apps, which give them names and icons
    }

    async Task LoadNotifications(string serial)
    {
        if (Devices.FirstOrDefault(d => d.serial == serial)?.state != "device") return;
        try
        {
            var list = await Task.Run(() => Core.Notifications(serial));
            if (Notifications.TryGetValue(serial, out var shown) && shown.SequenceEqual(list)) return;
            Notifications[serial] = list;
            Notify();
        }
        catch { }
    }

    /// Mirrors the phone with its notification shade pulled down, so the notification can be tapped:
    /// only the phone can run a notification's action.
    public async void Show(PhoneNotification notification, Device device)
    {
        try { await Task.Run(() => Core.ShowNotificationShade(device.serial)); }
        catch { }
        Mirror(device);
    }

    /// Opens the notification's app: in its own window if the phone supports that, else mirrored.
    public void OpenApp(PhoneNotification notification, Device device)
    {
        var supported = Info.TryGetValue(device.serial, out var info) && info.sdk >= TetherCoreMethods.VirtualDisplayMinSdk();
        if (!supported) { Mirror(device); return; }
        var app = Apps.GetValueOrDefault(device.serial)?.FirstOrDefault(a => a.package == notification.package)
            ?? new DeviceApp(notification.appName, notification.package, false, null);
        Open(app, device);
    }

    /// Reloads info, and the app list from the device rather than the cache.
    public async void Refresh()
    {
        if (Selection is not { } serial) return;
        await Task.WhenAll(LoadInfo(serial), LoadApps(serial, force: true));
    }

    async Task LoadInfo(string serial)
    {
        try
        {
            Info[serial] = await Task.Run(() => Core.DeviceInfo(serial));
            Notify();
        }
        catch { }
    }

    async Task LoadApps(string serial, bool force)
    {
        if (LoadingApps.Contains(serial) || (!force && Apps.ContainsKey(serial))) return;
        LoadingApps.Add(serial);
        AppErrors.Remove(serial);
        Notify();
        try { Apps[serial] = await Task.Run(() => Core.Apps(serial, force)); }
        catch (Exception e) { AppErrors[serial] = e.Message; }
        LoadingApps.Remove(serial);
        Notify();
    }

    public string Name(Device device) => Info.TryGetValue(device.serial, out var info) ? info.name : device.label;

    // --- Actions -----------------------------------------------------------

    public void Mirror(Device device) => Launch(() => Core.Mirror(device.serial, Name(device)));

    /// The usable screen area app windows open on, in points (DIPs), and its scale (1.5 at 150%).
    public Screen Screen = new(1280, 800, 1);

    public void Open(DeviceApp app, Device device) =>
        Launch(() => Core.OpenApp(device.serial, app.package, $"{app.name} · {Name(device)}", Screen));

    /// Saves a screenshot to the Desktop and shows it in Explorer.
    public async void Screenshot(Device device)
    {
        var desktop = Environment.GetFolderPath(Environment.SpecialFolder.DesktopDirectory);
        var name = string.Concat($"Screenshot {Name(device)} {DateTime.Now:yyyy-MM-dd HHmmss}.png".Split(Path.GetInvalidFileNameChars()));
        var path = Path.Combine(desktop, name);
        try
        {
            await Task.Run(() => Core.Screenshot(device.serial, path));
            System.Diagnostics.Process.Start("explorer.exe", $"/select,\"{path}\"");
        }
        catch (Exception e) { Alert?.Invoke(e.Message); }
    }

    /// Serials with an APK install in progress.
    public readonly HashSet<string> Installing = [];

    public async void Install(IEnumerable<string> apks, Device device)
    {
        Installing.Add(device.serial);
        Notify();
        foreach (var apk in apks)
        {
            try { await Task.Run(() => Core.InstallApk(device.serial, apk)); }
            catch (Exception e) { Alert?.Invoke($"Couldn't install {Path.GetFileName(apk)}: {e.Message}"); }
        }
        Installing.Remove(device.serial);
        Notify();
        await LoadApps(device.serial, force: true);
    }

    public async void Press(DeviceKey key, Device device)
    {
        try { await Task.Run(() => Core.PressKey(device.serial, key)); }
        catch (Exception e) { Alert?.Invoke(e.Message); }
        if (key == DeviceKey.Power)
        {
            await Task.Delay(600);
            await LoadInfo(device.serial);
        }
    }

    public void SetDarkMode(bool on, Device device) => Update(device, () => Core.SetDarkMode(device.serial, on));

    public void SetStayAwake(bool on, Device device) => Update(device, () => Core.SetStayAwake(device.serial, on));

    public void Reboot(Device device, RebootMode mode) => Run(() => Core.Reboot(device.serial, mode));

    /// Changes a device setting, then reloads its info to show the result.
    async void Update(Device device, Action action)
    {
        try { await Task.Run(action); }
        catch (Exception e) { Alert?.Invoke(e.Message); }
        await LoadInfo(device.serial);
    }

    /// A short result of the last clipboard action, per phone.
    public readonly Dictionary<string, string> ClipboardStatus = [];

    /// Puts this PC's clipboard text on the phone's clipboard.
    public async void SendClipboard(Device device)
    {
        var content = Windows.ApplicationModel.DataTransfer.Clipboard.GetContent();
        var text = content.Contains(Windows.ApplicationModel.DataTransfer.StandardDataFormats.Text) ? await content.GetTextAsync() : "";
        if (text == "") { ClipboardStatus[device.serial] = "The clipboard has no text."; Notify(); return; }
        try
        {
            await Task.Run(() => Core.SetPhoneClipboard(device.serial, text));
            ClipboardStatus[device.serial] = $"Sent {text.Length} characters to the phone.";
        }
        catch (Exception e) { ClipboardStatus[device.serial] = e.Message; }
        Notify();
    }

    /// Copies the phone's clipboard text to this PC's clipboard.
    public async void CopyClipboard(Device device)
    {
        try
        {
            var text = await Task.Run(() => Core.PhoneClipboard(device.serial));
            if (text != null)
            {
                var package = new Windows.ApplicationModel.DataTransfer.DataPackage();
                package.SetText(text);
                Windows.ApplicationModel.DataTransfer.Clipboard.SetContent(package);
                ClipboardStatus[device.serial] = $"Copied {text.Length} characters from the phone.";
            }
            else
            {
                ClipboardStatus[device.serial] = "Nothing to copy: the phone's clipboard is empty, or the phone doesn't let it be read (Xiaomi phones don't).";
            }
        }
        catch (Exception e) { ClipboardStatus[device.serial] = e.Message; }
        Notify();
    }

    public void GoWireless(Device device) => Run(() => Core.GoWireless(device.serial));

    /// Pairs (if a code is given) and/or connects over Wi-Fi. Throws so the dialog can show the error.
    public async Task Pair(string address, string code, string connect)
    {
        await Task.Run(() =>
        {
            if (address != "" && code != "") Core.Pair(address, code);
            if (connect != "") Core.Connect(connect);
        });
        await RefreshDevices();
    }

    public WindowSize WindowSize => TetherCoreMethods.ParseWindowSize(Config.windowSize) ?? new WindowSize(0, 0, 0);

    public void SetWindowSize(WindowSize size) => Save(Config with { windowSize = TetherCoreMethods.FormatWindowSize(size) });

    public void SetCaptureMouse(bool on) => Save(Config with { captureMouse = on });

    public void Save(Config config)
    {
        try
        {
            Core.SaveConfig(config);
            Config = config;
            Notify();
        }
        catch (Exception e) { Alert?.Invoke(e.Message); }
    }

    public void FinishSetup()
    {
        Save(Core.Config() with { setupDone = true });
        NeedsSetup = Core.NeedsSetup();
        Apps.Clear(); // adb or scrcpy may have changed
        Notify();
        CheckForUpdate();
    }

    public void RunSetupAgain()
    {
        NeedsSetup = true;
        Notify();
    }

    // --- scrcpy updates ----------------------------------------------------

    public async void CheckForUpdate()
    {
        try { Update = await Task.Run(Core.AvailableUpdate); Notify(); }
        catch { }
    }

    public async void InstallUpdate()
    {
        if (Update is not { } release) return;
        InstallingUpdate = true;
        Notify();
        try
        {
            await Task.Run(() => Core.InstallScrcpy(release));
            Update = null;
        }
        catch (Exception e) { Alert?.Invoke($"Couldn't update scrcpy: {e.Message}"); }
        InstallingUpdate = false;
        Notify();
    }

    /// Starts a scrcpy window, brings it to the front and reports how it ended.
    async void Launch(Func<Session> start)
    {
        try
        {
            using var session = await Task.Run(start);
            _ = BringToFront(session.Pid());
            await Task.Run(session.Wait);
        }
        catch (Exception e) { Alert?.Invoke(e.Message); }
    }

    /// Windows only lets the foreground app hand over focus, so do that once scrcpy's window appears.
    static async Task BringToFront(uint pid)
    {
        for (var i = 0; i < 50; i++) // up to 10 s
        {
            var window = IntPtr.Zero;
            EnumWindows((hwnd, _) =>
            {
                GetWindowThreadProcessId(hwnd, out var owner);
                if (owner != pid || !IsWindowVisible(hwnd)) return true;
                window = hwnd;
                return false;
            }, IntPtr.Zero);
            if (window != IntPtr.Zero)
            {
                SetForegroundWindow(window);
                return;
            }
            await Task.Delay(200);
        }
    }

    /// Runs a core action in the background and shows its error, if any.
    async void Run(Action action)
    {
        try { await Task.Run(action); }
        catch (Exception e) { Alert?.Invoke(e.Message); }
    }

    delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr param);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumWindowsProc callback, IntPtr param);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr hwnd);
}
