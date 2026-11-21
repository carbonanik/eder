import {
  AbsoluteFill,
  CanvasImage,
  interpolate,
  staticFile,
  useCurrentFrame,
  useVideoConfig,
} from "remotion";
import { base, colors, ease } from "./design";

export const Closing = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  return (
    <AbsoluteFill
      style={{
        ...base,
        background: colors.deep,
        color: colors.paper,
        alignItems: "center",
        justifyContent: "center",
        overflow: "hidden",
      }}
    >
      <div
        style={{
          position: "absolute",
          inset: 95,
          border: "1px solid #9de1d725",
        }}
      />
      <div
        style={{
          position: "absolute",
          width: 900,
          height: 900,
          border: "1px solid #9de1d712",
          borderRadius: "50%",
          right: -450,
          top: 90,
          scale: interpolate(frame, [0, 3 * fps], [0.9, 1.1], ease),
        }}
      />
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 28,
          opacity: interpolate(frame, [0, 0.5 * fps], [0, 1], ease),
          translate: `0 ${interpolate(frame, [0, 0.65 * fps], [38, 0], ease)}px`,
        }}
      >
        <CanvasImage
          src={staticFile("eder-mark.svg")}
          style={{ width: 140, height: 140 }}
        />
        <div style={{ fontSize: 150, fontWeight: 750, letterSpacing: -8 }}>
          Eder<span style={{ color: colors.orange }}>.</span>
        </div>
      </div>
      <div
        style={{
          fontSize: 65,
          fontWeight: 500,
          letterSpacing: -2,
          marginTop: 35,
          opacity: interpolate(frame, [0.2 * fps, 0.7 * fps], [0, 1], ease),
        }}
      >
        A quieter way to read.
      </div>
      <div
        style={{
          marginTop: 65,
          background: colors.teal,
          padding: "19px 36px",
          fontFamily: "DM Sans",
          fontSize: 34,
          fontWeight: 600,
          color: "#fff",
          opacity: interpolate(frame, [0.4 * fps, 0.85 * fps], [0, 1], ease),
          translate: `0 ${interpolate(frame, [0.4 * fps, 0.85 * fps], [18, 0], ease)}px`,
        }}
      >
        Get Eder <span style={{ marginLeft: 28 }}>↗</span>
      </div>
      <div
        style={{
          position: "absolute",
          bottom: 136,
          fontFamily: "DM Sans",
          fontSize: 27,
          color: "#a4bfc1",
          opacity: interpolate(frame, [0.5 * fps, 0.9 * fps], [0, 1], ease),
        }}
      >
        github.com/carbonanik/eder
      </div>
    </AbsoluteFill>
  );
};
