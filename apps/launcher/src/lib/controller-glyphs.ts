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
