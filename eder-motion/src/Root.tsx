import {
  AbsoluteFill,
  Composition,
  Folder,
  Sequence,
  useVideoConfig,
  staticFile,
} from "remotion";
import { Audio } from "@remotion/media";
import { Opening } from "./Opening";
import { Showcase } from "./Showcase";
import { Closing } from "./Closing";

const EderPromo = () => {
  const { fps } = useVideoConfig();
  return (
    <AbsoluteFill>
      <Sequence
        name="Your personal bookshelf"
        durationInFrames={2 * fps}
        premountFor={fps}
      >
        <Opening />
      </Sequence>
      <Sequence
        name="Desktop showcase"
        from={2 * fps}
        durationInFrames={5 * fps}
        premountFor={fps}
      >
        <Showcase />
      </Sequence>
      <Sequence
        name="A quieter way to read"
        from={7 * fps}
        durationInFrames={3 * fps}
        premountFor={fps}
      >
        <Closing />
      </Sequence>
      <Audio src={staticFile("eder-score.wav")} premountFor={fps} />
    </AbsoluteFill>
  );
};

export const RemotionRoot = () => (
  <>
    <Composition
      id="EderPromo"
      component={EderPromo}
      durationInFrames={300}
      fps={30}
      width={1920}
      height={1080}
    />
    <Folder name="Scenes">
      <Composition
        id="Opening"
        component={Opening}
        durationInFrames={60}
        fps={30}
        width={1920}
        height={1080}
      />
      <Composition
        id="Showcase"
        component={Showcase}
        durationInFrames={150}
        fps={30}
        width={1920}
        height={1080}
      />
      <Composition
        id="Closing"
        component={Closing}
        durationInFrames={90}
        fps={30}
        width={1920}
        height={1080}
      />
    </Folder>
  </>
);
