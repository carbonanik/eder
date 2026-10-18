# Reusable UI Design Specification — Minimal Editorial

Design a premium, responsive product landing page using a **Minimal Editorial / Swiss-Inspired** design language.

The website should feel like a carefully designed independent software brand, not a generic SaaS template.

## Visual Style

- Warm off-white backgrounds and deep navy text.
- Restrained teal accents for emphasis, section numbers, and interactions.
- Oversized, bold typography with tight letter spacing.
- Asymmetrical two-column layouts.
- Generous whitespace and clear visual hierarchy.
- Thin 1px dividers, minimal shadows, and restrained corner rounding.
- Alternate between light editorial sections and dark, high-contrast showcase sections.
- Avoid gradients, excessive glassmorphism, colorful icons, and unnecessary decorations.

## Color Tokens

- Background: `#F4F1E9`
- Primary text: `#101F24`
- Dark sections: `#09212B`
- Accent: `#159D99`
- Muted text: `#68777A`
- Secondary surface: `#DFE9E4`
- Light text: `#FAF9F5`
- Border: `rgba(13,39,43,0.2)`

## Typography

Use **Manrope** for headings and **DM Sans** for body content.

- Hero headlines: 74–128px, 800 weight, -0.095em tracking.
- Section headlines: 53–104px, 800 weight, -0.085em tracking.
- Feature titles: 24–37px, bold.
- Body: 15–17px, line-height 1.7–1.85.
- Small labels: 11–12px, bold, uppercase, wide letter spacing.

Use short, meaningful headlines, intentional line breaks, and an occasional accent-colored word.

## Page Architecture

1. **Header:** Minimal brand identity, numbered uppercase navigation, understated CTA.
2. **Hero:** Asymmetric 50/50 layout with oversized statement on the left and real product imagery on the right.
3. **Value strip:** Compact horizontal list of core product principles.
4. **Features:** Editorial heading followed by numbered horizontal feature rows instead of traditional feature cards.
5. **Product showcase:** Full-width dark section featuring a large authentic application screenshot.
6. **Brand manifesto:** Bold multiline statement with a structured list of core product values.
7. **Technology or capabilities:** Border-based four-column information grid.
8. **Final CTA:** Dark background, oversized headline, simple text links, and relevant getting-started information.
9. **Footer:** Minimal logo, navigation, copyright, and essential notes.

## Layout Rules

- Maximum content width: 1360px.
- Desktop horizontal padding: approximately 48px.
- Major section spacing: 120–130px.
- Use CSS Grid for large structural layouts.
- Feature rows should be separated by thin horizontal lines.
- Keep imagery large and purposeful.
- Avoid filling empty space with unnecessary UI elements.

## Interactions

- Underlined text CTAs with diagonal arrows.
- Subtle 200ms hover transitions.
- Feature row hover background and horizontal movement.
- IntersectionObserver-powered fade-up animations around 700ms.
- Smooth anchor navigation.
- Responsive mobile menu.
- Respect `prefers-reduced-motion`.

## Responsive Behavior

- Desktop: Wide, asymmetrical editorial compositions.
- Tablet: Reduce spacing and stack major sections as needed.
- Mobile: Single-column layout, oversized but scaled typography, collapsible navigation, and full-width product imagery.
- Preserve generous whitespace and readable typography on every screen.

## Design Philosophy

Prioritize storytelling over decoration.

Each section should communicate one clear idea. Use typography and whitespace to create interest rather than gradients, illustrations, animations, or excessive cards.

The finished design should feel **minimal, premium, distinctive, technical, confident, and timeless**.

Adapt all copy, visuals, brand colors, and product details to the actual product. Do not invent functionality or technical claims.