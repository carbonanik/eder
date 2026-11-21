import {
  AbsoluteFill,
  CanvasImage,
  interpolate,
  staticFile,
  useCurrentFrame,
  useVideoConfig,
} from "remotion";
import { base, colors, ease } from "./design";

export const Showcase = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  return (
    <AbsoluteFill style={{ ...base, overflow: "hidden" }}>
      <div
        style={{
          position: "absolute",
          left: 1090,
          top: -100,
          width: 1000,
          height: 1300,
          background: "#e0e9e3",
          rotate: "-12deg",
        }}
      />
      <div
        style={{
          position: "absolute",
          left: 110,
          top: 115,
          display: "flex",
          alignItems: "center",
          gap: 14,
        }}
      >
        <CanvasImage
          src={staticFile("eder-mark.svg")}
          style={{ width: 54, height: 54 }}
        />
        <span style={{ fontSize: 37, fontWeight: 800, letterSpacing: -1 }}>
          Eder<span style={{ color: colors.orange }}>.</span>
        </span>
      </div>
      <div
        style={{
          position: "absolute",
          left: 115,
          top: 300,
          width: 600,
          opacity: interpolate(frame, [0, 0.4 * fps], [0, 1], ease),
          translate: `0 ${interpolate(frame, [0, 0.7 * fps], [35, 0], ease)}px`,
        }}
      >
        <div
          style={{
            fontSize: 94,
            lineHeight: 1.08,
            fontWeight: 650,
            letterSpacing: -5,
          }}
        >
          On your
          <br />
          <span style={{ color: colors.teal }}>desktop.</span>
        </div>
        <div
          style={{
            width: 72,
            height: 5,
            background: colors.orange,
            marginTop: 46,
          }}
        />
        <div
          style={{
            position: "relative",
            marginTop: 34,
            height: 210,
            fontFamily: "DM Sans",
            fontSize: 43,
            lineHeight: 1.28,
            letterSpacing: -1,
          }}
        >
          <div
            style={{
              position: "absolute",
              opacity: interpolate(
                frame,
                [0, 0.45 * fps, 2.1 * fps, 2.45 * fps],
                [0, 1, 1, 0],
                ease,
              ),
            }}
          >
            Your books.
            <br />
            Ready to read.
          </div>
          <div
            style={{
              position: "absolute",
              opacity: interpolate(
                frame,
                [2.4 * fps, 2.65 * fps, 3.45 * fps, 3.7 * fps],
                [0, 1, 1, 0],
                ease,
              ),
            }}
          >
            Familiar folders.
          </div>
          <div
            style={{
              position: "absolute",
              opacity: interpolate(frame, [3.6 * fps, 3.9 * fps], [0, 1], ease),
            }}
          >
            Pick up where
            <br />
            you left off.
          </div>
        </div>
      </div>
      <div
        style={{
          position: "absolute",
          left: 770,
          top: 155,
          width: 1135,
          borderRadius: 12,
          overflow: "hidden",
          boxShadow: "0 38px 85px #09212b30",
          border: "1px solid #09212b24",
          translate: `${interpolate(frame, [0, 0.8 * fps], [200, 0], ease)}px ${interpolate(frame, [0, 5 * fps], [30, -20], ease)}px`,
          scale: interpolate(frame, [0, 5 * fps], [0.96, 1.04], ease),
          transformOrigin: "left center",
          opacity: interpolate(frame, [0, 0.4 * fps], [0, 1], ease),
        }}
      >
        <CanvasImage
          src={staticFile("eder-app.png")}
          style={{ width: "100%", display: "block" }}
        />
        <div
          style={{
            position: "absolute",
            left: 19,
            top: "43.8%",
            width: 234,
            height: 60,
            border: "3px solid #159d99",
            borderRadius: 4,
            opacity: interpolate(
              frame,
              [3.65 * fps, 3.85 * fps],
              [0, 0.85],
              ease,
            ),
            boxShadow: "0 0 0 5000px #09212b08",
          }}
        />
      </div>
      <div
        style={{
          position: "absolute",
          left: 115,
          bottom: 115,
          color: colors.muted,
          fontFamily: "DM Sans",
          fontSize: 23,
          letterSpacing: 3,
        }}
      >
        DESKTOP PDF READER
      </div>
    </AbsoluteFill>
  );
};
