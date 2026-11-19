// The integrated Windows BLE transport. Standard output is a private binary pipe to
// the compiler/game's SDL adapter; standard input carries rumble and LED requests.
using System.Buffers.Binary;
using System.Collections.Concurrent;
using System.Globalization;
using System.Runtime.InteropServices.WindowsRuntime;
using System.Threading.Channels;
using NxToWin.Dsp;
using NxToWin.Hardware;
using NxToWin.Models;
using Windows.Devices.Bluetooth;
using Windows.Devices.Bluetooth.Advertisement;
using Windows.Devices.Enumeration;

internal static class Program
{
    private static readonly CancellationTokenSource Life = new();
    private static readonly Channel<byte[]> Frames = Channel.CreateBounded<byte[]>(new BoundedChannelOptions(64)
        { FullMode = BoundedChannelFullMode.DropOldest, SingleReader = true });
    private static readonly ConcurrentDictionary<ulong, long> Retry = new();
    private static readonly SemaphoreSlim Connecting = new(1, 1);
    private static readonly object Gate = new();
    private static Switch2BleController? Controller;
    private static long Reports;
    private static uint Sequence;
    private static bool Probe;
    private static int Rumble;

    private static void Log(string text) => Console.Error.WriteLine("[Nintendo Bluetooth] " +
        System.Text.RegularExpressions.Regex.Replace(text, @"\b[0-9A-F]{12}\b", "[device]"));

    public static async Task<int> Main(string[] args)
    {
        if (args is ["--self-test"]) { SelfTest(); return 0; }
        if (args is ["--protocol-test"]) { try { await ProtocolTest(); } catch (IOException) { } return 0; }
        Probe = args is ["--probe", _];
        if (Probe && int.TryParse(args[1], out int seconds) && seconds is > 0 and <= 300)
            Life.CancelAfter(TimeSpan.FromSeconds(seconds));
        else if (args.Length != 0) return 2;
        Console.CancelKeyPress += (_, e) => { e.Cancel = true; Life.Cancel(); };
        var output = Probe ? Task.CompletedTask : WriteFrames();
        var input = Probe ? Task.CompletedTask : ReadCommands();
        _ = RumbleLoop();
        using var watcher = new Watcher();
        watcher.Start();
        Log("Press Sync on your Pro Controller 2. Close other controller bridge apps during connection.");
        try
        {
            while (!Life.IsCancellationRequested)
            {
                await ScanKnown();
                await Task.Delay(4000, Life.Token);
            }
        }
        catch (OperationCanceledException) { }
        finally
        {
            Switch2BleController? controller;
            lock (Gate) { controller = Controller; Controller = null; }
            if (controller != null)
            {
                try { await controller.SendHdRumbleAsync(StereoRumble.Silence).WaitAsync(TimeSpan.FromSeconds(1)); }
                catch (Exception) { }
                controller.Dispose();
            }
            Frames.Writer.TryComplete();
            try { await output.WaitAsync(TimeSpan.FromSeconds(1)); } catch (Exception) { }
        }
        if (Probe) Console.WriteLine(System.Text.Json.JsonSerializer.Serialize(new { reports = Reports }));
        return 0;
    }

    private sealed class Watcher : IDisposable
    {
        private readonly BluetoothLEAdvertisementWatcher watcher = new() { ScanningMode = BluetoothLEScanningMode.Active };
        public void Start()
        {
            watcher.Received += Received;
            try { watcher.Start(); }
            catch (Exception ex) { Log("Bluetooth scan unavailable: " + ex.Message); }
        }
        private static void Received(BluetoothLEAdvertisementWatcher sender, BluetoothLEAdvertisementReceivedEventArgs args)
        {
            foreach (var maker in args.Advertisement.ManufacturerData)
            {
                if (maker.CompanyId != NintendoIds.BluetoothManufacturerId) continue;
                byte[] data = maker.Data.ToArray();
                if (data.Length >= 7 && BinaryPrimitives.ReadUInt16LittleEndian(data.AsSpan(3)) == NintendoIds.VendorId &&
                    BinaryPrimitives.ReadUInt16LittleEndian(data.AsSpan(5)) == NintendoIds.ProController2Pid)
                    _ = Connect(args.BluetoothAddress);
            }
            // Nintendo's service UUID also appears in its Sync advertisement.
            if (args.Advertisement.ServiceUuids.Contains(NintendoIds.Sw2ServiceUuid)) _ = Connect(args.BluetoothAddress);
        }
        public void Dispose() { watcher.Stop(); watcher.Received -= Received; }
    }

    private static async Task ScanKnown()
    {
        try
        {
            const string addressKey = "System.Devices.Aep.DeviceAddress";
            var devices = await DeviceInformation.FindAllAsync(BluetoothLEDevice.GetDeviceSelectorFromPairingState(true), [addressKey])
                .AsTask(Life.Token);
            foreach (var info in devices)
                if (info.Name.Contains("Pro Controller", StringComparison.OrdinalIgnoreCase) &&
                    info.Properties.TryGetValue(addressKey, out var address) && address is string text &&
                    ulong.TryParse(text.Replace(":", "").Replace("-", ""), NumberStyles.HexNumber,
                        CultureInfo.InvariantCulture, out ulong value)) _ = Connect(value);
        }
        catch (OperationCanceledException) { }
        catch (Exception ex) { Log("Paired-device scan unavailable: " + ex.Message); }
    }

    private static async Task Connect(ulong address)
    {
        if (address == 0 || Life.IsCancellationRequested || !await Connecting.WaitAsync(0)) return;
        Switch2BleController? next = null;
        try
        {
            lock (Gate) if (Controller?.IsConnected == true) return;
            long now = Environment.TickCount64;
            if (Retry.TryGetValue(address, out long due) && now < due) return;
            Retry[address] = now + 8000;
            next = new Switch2BleController(address, NintendoIds.ProController2Pid, new BridgeConfiguration(), Log);
            if (!await next.ConnectAndInitializeAsync(Life.Token) || Life.IsCancellationRequested) return;
            // Half Joy-Cons require explicit pairing, so never expose one as a full pad.
            if (next.ProductId != NintendoIds.ProController2Pid) return;
            next.StateUpdated += OnState;
            next.Disconnected += OnDisconnected;
            lock (Gate) Controller = next;
            Log("Pro Controller 2 connected directly to Age of Decomp.");
            await next.SetPlayerLedsAsync(1);
            next = null;
        }
        catch (Exception ex) { if (!Life.IsCancellationRequested) Log("Connection failed: " + ex.Message); }
        finally { next?.Dispose(); Connecting.Release(); }
    }

    private static void OnState(IPhysicalController sender, ControllerStateSnapshot state)
    {
        Interlocked.Increment(ref Reports);
        if (!Probe) Frames.Writer.TryWrite(Packet(state, true));
    }
    private static void OnDisconnected(IPhysicalController sender)
    {
        lock (Gate) if (Controller == sender) Controller = null;
        if (!Probe) Frames.Writer.TryWrite(Packet(new(), false));
        Log("Pro Controller 2 disconnected. Press Sync to reconnect.");
        sender.Dispose();
    }

    // 64-byte AODPAD01 v1 packet, little endian. Slot 0, PID 2069; physical Nintendo
    // button bits; sticks in [-1,1], positive Y up; gyro rad/s, acceleration m/s².
    internal static byte[] Packet(ControllerStateSnapshot state, bool connected)
    {
        byte[] b = new byte[64];
        "AODPAD01"u8.CopyTo(b);
        b[8] = 0; b[9] = connected ? (byte)1 : (byte)0;
        BinaryPrimitives.WriteUInt16LittleEndian(b.AsSpan(10), NintendoIds.ProController2Pid);
        BinaryPrimitives.WriteUInt32LittleEndian(b.AsSpan(12), unchecked(++Sequence));
        BinaryPrimitives.WriteUInt32LittleEndian(b.AsSpan(16), (uint)state.Buttons);
        float[] values = [state.LeftStickX, state.LeftStickY, state.RightStickX, state.RightStickY,
            state.GyroDps_X * MathF.PI / 180, state.GyroDps_Z * MathF.PI / 180, -state.GyroDps_Y * MathF.PI / 180,
            state.AccelG_X * 9.80665f, state.AccelG_Z * 9.80665f, -state.AccelG_Y * 9.80665f];
        for (int i = 0; i < values.Length; i++) BinaryPrimitives.WriteSingleLittleEndian(b.AsSpan(20 + i * 4), values[i]);
        BinaryPrimitives.WriteInt32LittleEndian(b.AsSpan(60), state.Battery.Percent ?? -1);
        return b;
    }

    private static async Task WriteFrames()
    {
        try
        {
            using var output = Console.OpenStandardOutput();
            await foreach (byte[] frame in Frames.Reader.ReadAllAsync()) await output.WriteAsync(frame);
        }
        catch (IOException) { Life.Cancel(); }
    }
    private static async Task ReadCommands()
    {
        try
        {
            using var input = Console.OpenStandardInput();
            byte[] b = new byte[16];
            while (!Life.IsCancellationRequested)
            {
                await input.ReadExactlyAsync(b, Life.Token);
                if (!b.AsSpan(0, 8).SequenceEqual("AODRUM01"u8) || b[8] != 0) break;
                Volatile.Write(ref Rumble, BinaryPrimitives.ReadInt32LittleEndian(b.AsSpan(10)));
            }
        }
        catch (Exception ex) when (ex is IOException or OperationCanceledException) { }
        finally { Life.Cancel(); }
    }

    private static async Task RumbleLoop()
    {
        int previous = 0;
        try
        {
            while (!Life.IsCancellationRequested)
            {
                int value = Volatile.Read(ref Rumble);
                Switch2BleController? controller; lock (Gate) controller = Controller;
                if (controller != null && (value != 0 || previous != 0))
                {
                    int low = (value & 65535) * 1000 / 65535;
                    int high = (int)((uint)value >> 16) * 1000 / 65535;
                    try { await controller.SendHdRumbleAsync(new(new VibrationFrame(160, low, 320, 0), new VibrationFrame(160, 0, 320, high))); }
                    catch (Exception ex) { if (!Life.IsCancellationRequested) Log("Rumble unavailable: " + ex.Message); }
                }
                previous = value;
                await Task.Delay(15, Life.Token);
            }
        }
        catch (OperationCanceledException) { }
    }

    private static void SelfTest()
    {
        byte[] report = new byte[63];
        BinaryPrimitives.WriteUInt32LittleEndian(report.AsSpan(4), (uint)(SwitchButtons.A | SwitchButtons.Down));
        StickCalibration.Pack12Bit(report.AsSpan(10), 2048, 2048);
        StickCalibration.Pack12Bit(report.AsSpan(13), 2048, 2048);
        var state = Switch2InputReport.Parse(5, report, NintendoIds.ProController2Pid, out _) ?? throw new Exception("report rejected");
        if (state.Buttons != (SwitchButtons.A | SwitchButtons.Down) || state.RawLeftStickX != 2048 ||
            Switch2InputReport.Parse(5, report.AsSpan(0, 62), NintendoIds.ProController2Pid, out _) != null)
            throw new Exception("physical report decoding failed");
        state.LeftStickX = .5f; state.GyroDps_X = 180; state.AccelG_Z = 1;
        byte[] packet = Packet(state, true);
        if (packet.Length != 64 || packet[9] != 1 || BinaryPrimitives.ReadUInt32LittleEndian(packet.AsSpan(16)) != 0x10008 ||
            Math.Abs(BinaryPrimitives.ReadSingleLittleEndian(packet.AsSpan(36)) - MathF.PI) > .0001f ||
            Math.Abs(BinaryPrimitives.ReadSingleLittleEndian(packet.AsSpan(52)) - 9.80665f) > .0001f)
            throw new Exception("wire units or button bits failed");
        Console.WriteLine("Nintendo BLE physical-report and wire-format checks passed.");
    }

    private static async Task ProtocolTest()
    {
        using var output = Console.OpenStandardOutput();
        for (int i = 0; i < 150; i++)
        {
            var state = new ControllerStateSnapshot { Buttons = SwitchButtons.A | SwitchButtons.Down,
                LeftStickX = .5f, LeftStickY = -.5f, GyroDps_X = 180, AccelG_Y = -1 };
            await output.WriteAsync(Packet(state, true));
            await Task.Delay(10);
        }
        await output.WriteAsync(Packet(new(), false));
    }
}
