import subprocess

STREAMS = 8
RTP_ADDRESS = "127.0.0.1"
FIRST_PORT = 1181
GLOBAL_FILTER = "scale=480:320,eq=brightness=-0.2:contrast=0.6"
STREAM_FILTER = (
    """drawtext=text=CAMSTREAM_NUMBER %{gmtime\\\\:%X.%3N}:fontsize=32:fontcolor=red"""
)

filter_ = f"[0:v]{GLOBAL_FILTER},split={STREAMS}"
filter_ += "".join([f"[in{i}]" for i in range(STREAMS)]) + ";"
filter_ += ";".join(
    [
        f"[in{i}]{STREAM_FILTER.replace('STREAM_NUMBER', str(i))}[out{i}]"
        for i in range(STREAMS)
    ]
)

arguments = [
    "ffmpeg",
    "-loglevel",
    "debug",
    "-f",
    "dshow",
    "-i",
    "video=UHD 4K Camera",
    "-filter_complex",
    filter_,
]

for i in range(STREAMS):
    # fmt: off
    arguments.extend([
        "-map", f"[out{i}]",
        "-vcodec", "libvpx",
        "-cpu-used", "5",
        "-deadline", "1",
        "-g", "10",
        "-error-resilient", "1",
        "-auto-alt-ref", "1",
        "-f", "rtp",
        "-b:v", "512k",
        "-maxrate", "524k",
        "-v", "0",
        f"rtp://{RTP_ADDRESS}:{FIRST_PORT + i}?pkt_size=1200",
    ])
    # fmt: on

subprocess.run(arguments)
