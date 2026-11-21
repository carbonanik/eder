import { loadFont } from "@remotion/fonts";
import { staticFile, Easing } from "remotion";

export const colors = {
  paper: "#f4f1e9",
  ink: "#101f24",
  muted: "#68777a",
  teal: "#159d99",
  deep: "#09212b",
  orange: "#f0803c",
};
export const ease = {
  extrapolateLeft: "clamp",
  extrapolateRight: "clamp",
  easing: Easing.bezier(0.16, 1, 0.3, 1),
} as const;
export const base = {
  fontFamily: "Manrope",
  color: colors.ink,
  background: colors.paper,
};
await Promise.all([
  loadFont({
    family: "Manrope",
    url: staticFile("fonts/Manrope.ttf"),
    weight: "200 800",
  }),
  loadFont({
    family: "DM Sans",
    url: staticFile("fonts/DMSans.ttf"),
    weight: "100 1000",
  }),
]);
