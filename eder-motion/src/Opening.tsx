import {
  AbsoluteFill,
  CanvasImage,
  interpolate,
  staticFile,
  useCurrentFrame,
  useVideoConfig,
} from "remotion";
import { base, colors, ease } from "./design";

export const Opening = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  return (
    <AbsoluteFill style={{ ...base, overflow: "hidden" }}>
      <div
        style={{
          position: "absolute",
          inset: 100,
          border: "1px solid #101f2418",
        }}
      />
      <div
        style={{
          position: "absolute",
          left: 154,
          top: 143,
          display: "flex",
          alignItems: "center",
          gap: 20,
        }}
      >
        <CanvasImage
          src={staticFile("eder-mark.svg")}
          style={{ width: 64, height: 64 }}
        />
        <div style={{ fontSize: 48, fontWeight: 800, letterSpacing: -2 }}>
          Eder<span style={{ color: colors.orange }}>.</span>
        </div>
      </div>
      <div
        style={{
          position: "absolute",
          left: 160,
          top: 332,
          opacity: interpolate(frame, [0, 0.4 * fps], [0, 1], ease),
          translate: `0 ${interpolate(frame, [0, 0.7 * fps], [52, 0], ease)}px`,
        }}
      >
        <div
          style={{
            fontSize: 142,
            lineHeight: 1.08,
            fontWeight: 600,
            letterSpacing: -9,
          }}
        >
          Your personal
        </div>
        <div
          style={{
            position: "relative",
            fontSize: 172,
            lineHeight: 1.15,
            fontWeight: 700,
            letterSpacing: -10,
            width: "fit-content",
          }}
        >
          bookshelf<span style={{ color: colors.teal }}>.</span>
          <div
            style={{
              position: "absolute",
              bottom: -15,
              left: 5,
              width: interpolate(frame, [0.4 * fps, 1.2 * fps], [0, 852], ease),
              height: 9,
              background: colors.teal,
            }}
          />
        </div>
      </div>
      <div
        style={{
          position: "absolute",
          right: 160,
          bottom: 153,
          fontFamily: "DM Sans",
          fontSize: 26,
          letterSpacing: 5,
          color: colors.muted,
          opacity: interpolate(frame, [0.7 * fps, 1.2 * fps], [0, 1], ease),
        }}
      >
        YOUR BOOKS, YOUR SPACE
      </div>
      <div
        style={{
          position: "absolute",
          right: 100,
          top: 100,
          width: 16,
          height: 16,
          background: colors.orange,
        }}
      />
    </AbsoluteFill>
  );
};
