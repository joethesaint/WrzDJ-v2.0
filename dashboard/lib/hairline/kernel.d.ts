declare const HL: {
  inject(root: Document | ShadowRoot): void;
  mk(tag: string, attrs: Record<string, string>, parent: Element): SVGSVGElement;
};
export default HL;
