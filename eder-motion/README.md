# Eder — A quieter way to read

Editable ten-second product promo. The finished deliverable is
[out/eder-10s-final.mp4](out/eder-10s-final.mp4): 1920 × 1080, 30 fps,
300 frames, H.264 video, stereo AAC audio, exactly 10.000 seconds.

## Preview and export

Run these commands from `eder-motion/`. Node.js 22 and FFmpeg are required.

```sh
npm ci
npm run dev -- --no-open
```

Select **EderPromo** in Studio, normally at http://localhost:3000/EderPromo.
Opening, Showcase, and Closing also have individual editable timelines.

```sh
npm run render
```

Export renders the composition, adjusts the quiet soundtrack by +10 dB,
trims AAC padding to exactly ten seconds, and enables MP4 fast start.
The intermediate `out/eder-10s.mp4` has the unadjusted soundtrack.

## Timing and editable assets

| Time | Scene | Source |
| --- | --- | --- |
| 0–2 seconds | Your personal bookshelf | `src/Opening.tsx` |
| 2–7 seconds | App showcase, folders, saved progress | `src/Showcase.tsx` |
| 7–10 seconds | Eder logo, tagline, CTA | `src/Closing.tsx` |

`src/Root.tsx` contains timing and audio; `src/design.tsx` contains palette,
fonts, and easing. The interface and logo were copied from the existing
landing-page assets. The highlight around Currently Reading is an editorial
overlay; the screenshot remains unchanged. No synthetic reader UI is shown.

The original ambient score contains two short accents at 2.04 and 4.5 seconds,
plus a three-note closing chime beginning at 7.25 seconds. Regenerate the WAV
with `python3 soundtrack.py`. No stock music, voiceover, or caption fonts are used.
Local Manrope and DM Sans fonts match the website; their SIL Open Font Licenses
are included in `public/fonts/`.

## Validation

- ESLint and TypeScript passed.
- Opening (frame 40), showcase (185), and closing (270) inspected as rendered images.
- Final MP4 fully decoded with FFmpeg without errors.
- FFprobe confirmed 300 video frames and exactly 10.000-second video/audio/container duration.
- Final measured audio: −21.4 LUFS integrated; −9.6 dBFS true peak; no clipping.
- Audio measured programmatically; real-time audiovisual playback was not auditioned.
- Style follows Eder's cream, teal, navy, and orange brand, with no font substitutions.

Remotion's [license terms](https://github.com/remotion-dev/remotion/blob/main/LICENSE.md)
apply to the editing/rendering tools.
