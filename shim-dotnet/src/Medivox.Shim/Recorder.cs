using NAudio.CoreAudioApi;
using NAudio.Wave;

namespace Medivox.Shim;

/// <summary>
/// Nimmt Audio zwischen Start() und Stop() auf. Kein VAD -- Start/Stop ist explizit
/// (Python-Original: audio.py).
/// </summary>
internal sealed class Recorder
{
    private WasapiCapture? _capture;
    private WaveFormat? _captureFormat;
    private readonly List<byte> _rawBytes = new();
    private readonly object _lock = new();

    public void Start()
    {
        lock (_lock)
        {
            _rawBytes.Clear();
        }

        _capture = new WasapiCapture();
        _captureFormat = _capture.WaveFormat;
        _capture.DataAvailable += OnDataAvailable;
        _capture.StartRecording();
    }

    private void OnDataAvailable(object? sender, WaveInEventArgs e)
    {
        lock (_lock)
        {
            _rawBytes.AddRange(new ArraySegment<byte>(e.Buffer, 0, e.BytesRecorded));
        }
    }

    /// <summary>
    /// Stoppt die Aufnahme und liefert die Samples als 16 kHz mono float32 zurueck --
    /// unabhaengig vom nativen Mixformat des Aufnahmegeraets (WASAPI-Shared-Mode liefert i. d. R.
    /// Stereo bei Geraete-Samplerate, z. B. 48 kHz). Downmix + Resampling passiert hier einmalig
    /// per MediaFoundationResampler, analog zur automatischen Konvertierung, die PortAudio im
    /// Python-Original (sounddevice) uebernimmt.
    /// </summary>
    public float[] Stop()
    {
        if (_capture is null || _captureFormat is null)
        {
            return Array.Empty<float>();
        }

        using var stoppedSignal = new ManualResetEventSlim(false);
        _capture.RecordingStopped += (_, _) => stoppedSignal.Set();
        _capture.StopRecording();
        stoppedSignal.Wait(TimeSpan.FromSeconds(2));

        _capture.DataAvailable -= OnDataAvailable;
        _capture.Dispose();
        _capture = null;

        byte[] raw;
        lock (_lock)
        {
            raw = _rawBytes.ToArray();
        }
        if (raw.Length == 0)
        {
            return Array.Empty<float>();
        }

        var sourceStream = new RawSourceWaveStream(new MemoryStream(raw), _captureFormat);
        var targetFormat = WaveFormat.CreateIeeeFloatWaveFormat(ShimConfig.SampleRate, 1);
        using var resampler = new MediaFoundationResampler(sourceStream, targetFormat) { ResamplerQuality = 60 };

        var output = new List<float>();
        var buffer = new byte[targetFormat.AverageBytesPerSecond];
        int bytesRead;
        while ((bytesRead = resampler.Read(buffer, 0, buffer.Length)) > 0)
        {
            for (var i = 0; i + 4 <= bytesRead; i += 4)
            {
                output.Add(BitConverter.ToSingle(buffer, i));
            }
        }
        return output.ToArray();
    }
}
