// Original SVG button artwork. Positions and logical actions are deliberately separate.
export type PadFamily = 'xbox' | 'ps4' | 'ps5' | 'playstation' | 'nintendo' | 'keyboard';
const images = import.meta.glob('../assets/buttons/*/*.svg', { eager: true, query: '?url', import: 'default' }) as Record<string, string>;

export function padFamily(type: string): PadFamily {
  return type === 'ps5' ? 'ps5' : type.startsWith('ps') ? 'ps4' :
    type.startsWith('switch') || type.startsWith('joycon') || type === 'gamecube' ? 'nintendo' : 'xbox';
}
export function buttonImage(family: PadFamily, button: string): string | undefined {
  return images[`../assets/buttons/${family === 'playstation' ? 'ps4' : family}/${button}.svg`];
}
export function buttonLabel(family: PadFamily, button: string): string {
  const face = ['ps4', 'ps5', 'playstation'].includes(family) ? { a: '✕', b: '○', x: '□', y: '△' } :
    { a: 'A', b: 'B', x: 'X', y: 'Y' };
  return (face as Record<string, string>)[button] ?? ({ up: '↑', down: '↓', left: '←', right: '→' } as Record<string, string>)[button] ?? button.toUpperCase();
}
export function physicalButton(family: PadFamily, button: string): string {
  const face = family === 'nintendo' ? { south: 'b', east: 'a', west: 'y', north: 'x' } :
    { south: 'a', east: 'b', west: 'x', north: 'y' };
  return (face as Record<string, string>)[button] ?? ({ left_shoulder: 'lb', right_shoulder: 'rb', back: 'back',
    start: 'start', left_stick: 'ls', right_stick: 'rs', guide: 'guide', misc1: 'back',
    dpad_up: 'up', dpad_down: 'down', dpad_left: 'left', dpad_right: 'right' } as Record<string, string>)[button] ?? button;
}
