# Audio fixtures

These two-second, 440 Hz sine waves were generated for Tunerift's offline integration tests. They contain no third-party recordings.

```sh
ffmpeg -f lavfi -i 'sine=frequency=440:sample_rate=44100:duration=2' -ac 2 -c:a libmp3lame -b:a 64k tone.mp3
ffmpeg -f lavfi -i 'sine=frequency=440:sample_rate=44100:duration=2' -ac 2 -c:a aac -b:a 64k -f mpegts tone.ts
```

FFmpeg is only a fixture generation tool. Tunerift and its tests do not invoke it.
