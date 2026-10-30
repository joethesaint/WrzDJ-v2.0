export interface HairlineRead { textContent: string }
export interface HairlineHandle { set(value: number): void; destroy(): void }
export interface HairlineFigure {
  name: string;
  means: string;
  range: [number, number, number];
  mount(host: { stage: HTMLElement; svg: SVGSVGElement; read: HairlineRead }, value: number): HairlineHandle;
}
