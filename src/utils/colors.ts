/**
 * Color palette generator — creates a family of colors from a single primary color.
 * Useful for dynamic theming (UI components, gradients, hover states).
 */

export interface HSL {
  h: number;
  s: number;
  l: number;
}

export interface Palette {
  /** The original input color */
  primary: string;
  /** Lightest tint (~15% lightness boost) */
  lightest: string;
  /** Light tint (~8% brightness) */
  light: string;
  /** Dark shade (~10% darker) */
  dark: string;
  /** Darkest shade (~20% darker) */
  darkest: string;
  /** Accent — hue-shifted by 30° */
  accent: string;
  /** Muted/desaturated variant */
  muted: string;
  /** Full gradient stops (6 stops) for SVG/UI gradients */
  gradientStops: string[];
  /** Background shade (very dark, low saturation) */
  bg: string;
  /** Surface shade (dark, slight saturation) */
  surface: string;
  /** Text-on-primary color (white or black based on contrast) */
  textOnPrimary: string;
}

// ── Helpers ──────────────────────────────────────────────

function hexToHsl(hex: string): HSL {
  let r = 0, g = 0, b = 0;
  hex = hex.replace("#", "");
  if (hex.length === 3) {
    r = parseInt(hex[0] + hex[0], 16) / 255;
    g = parseInt(hex[1] + hex[1], 16) / 255;
    b = parseInt(hex[2] + hex[2], 16) / 255;
  } else {
    r = parseInt(hex.substring(0, 2), 16) / 255;
    g = parseInt(hex.substring(2, 4), 16) / 255;
    b = parseInt(hex.substring(4, 6), 16) / 255;
  }

  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;

  let h = 0, s = 0;
  if (max !== min) {
    const d = max - min;
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    switch (max) {
      case r:
        h = ((g - b) / d + (g < b ? 6 : 0)) / 6;
        break;
      case g:
        h = ((b - r) / d + 2) / 6;
        break;
      case b:
        h = ((r - g) / d + 4) / 6;
        break;
    }
  }

  return { h: Math.round(h * 360), s: Math.round(s * 100), l: Math.round(l * 100) };
}

function hslToHex({ h, s, l }: HSL): string {
  s /= 100;
  l /= 100;

  const c = (1 - Math.abs(2 * l - 1)) * s;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = l - c / 2;

  let r = 0, g = 0, b = 0;
  if (h < 60) { r = c; g = x; }
  else if (h < 120) { r = x; g = c; }
  else if (h < 180) { g = c; b = x; }
  else if (h < 240) { g = x; b = c; }
  else if (h < 300) { r = x; b = c; }
  else { r = c; b = x; }

  const toHex = (v: number) =>
    Math.round((v + m) * 255)
      .toString(16)
      .padStart(2, "0");

  return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
}

function clampHsl({ h, s, l }: HSL): HSL {
  return {
    h: ((h % 360) + 360) % 360,
    s: Math.max(0, Math.min(100, s)),
    l: Math.max(0, Math.min(100, l)),
  };
}

function withLightness(base: HSL, delta: number): string {
  return hslToHex(clampHsl({ ...base, l: base.l + delta }));
}

function withSaturation(base: HSL, delta: number): string {
  return hslToHex(clampHsl({ ...base, s: base.s + delta }));
}

function withHue(base: HSL, delta: number): string {
  return hslToHex(clampHsl({ ...base, h: base.h + delta }));
}

// ── Main generator ───────────────────────────────────────

/**
 * Generates a full color palette from a single primary hex color.
 *
 * @param primary - The base color in hex format (e.g. "#32B5F1")
 * @returns A Palette object with all derived colors
 */
export function generatePalette(primary: string): Palette {
  const base = hexToHsl(primary);

  // For the gradient, replicate a VS Code-style multi-stop gradient
  // going from light → medium → dark
  const gradientStops = [
    hslToHex(clampHsl({ ...base, l: base.l + 18, s: base.s - 5 })), // lightest
    hslToHex(clampHsl({ ...base, l: base.l + 10 })),                 // lighter
    primary,                                                          // primary
    hslToHex(clampHsl({ ...base, l: base.l - 6, s: base.s + 3 })),  // mid-dark
    hslToHex(clampHsl({ ...base, l: base.l - 14 })),                 // darker
    hslToHex(clampHsl({ ...base, l: base.l - 22 })),                 // darkest
  ];

  return {
    primary,
    lightest: withLightness(base, 15),
    light: withLightness(base, 8),
    dark: withLightness(base, -10),
    darkest: withLightness(base, -20),
    accent: withHue(base, 30),
    muted: withSaturation(base, -40),
    gradientStops,
    bg: hslToHex(clampHsl({ ...base, l: 5, s: 15 })),
    surface: hslToHex(clampHsl({ ...base, l: 10, s: 10 })),
    textOnPrimary: base.l > 60 ? "#0d1117" : "#ffffff",
  };
}
