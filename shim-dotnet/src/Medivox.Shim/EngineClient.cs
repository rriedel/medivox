namespace Medivox.Shim;

/// <summary>
/// HTTP-Client zur Engine (Python-Original: client.py).
/// </summary>
internal sealed class EngineClient : IDisposable
{
    private readonly HttpClient _http;

    public EngineClient()
    {
        _http = new HttpClient
        {
            BaseAddress = new Uri($"http://{ShimConfig.EngineHost}:{ShimConfig.EnginePort}/"),
            Timeout = TimeSpan.FromSeconds(ShimConfig.RequestTimeoutSeconds),
        };
    }

    public async Task<string> TranscribeAsync(float[] audio, CancellationToken cancellationToken = default)
    {
        var bytes = new byte[audio.Length * sizeof(float)];
        Buffer.BlockCopy(audio, 0, bytes, 0, bytes.Length);

        using var content = new ByteArrayContent(bytes);
        content.Headers.ContentType = new System.Net.Http.Headers.MediaTypeHeaderValue("application/octet-stream");

        using var response = await _http.PostAsync("transcribe", content, cancellationToken);
        response.EnsureSuccessStatusCode();
        return await response.Content.ReadAsStringAsync(cancellationToken);
    }

    public void Dispose() => _http.Dispose();
}
