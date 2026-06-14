import { registerInnerEditor } from '@leafer-in/editor';
import { TextEditor } from '@leafer-in/text-editor';

const textCaseMap = {
  none: 'none',
  title: 'capitalize',
  upper: 'uppercase',
  lower: 'lowercase',
  'small-caps': 'small-caps',
} as const;

const verticalAlignMap = {
  top: 'flex-start',
  middle: 'center',
  bottom: 'flex-end',
} as const;

const textDecorationMap = {
  none: 'none',
  under: 'underline',
  delete: 'line-through',
  'under-delete': 'underline line-through',
} as const;

type TooltipTextDecorationType = keyof typeof textDecorationMap;

type TooltipTextDecoration =
  | TooltipTextDecorationType
  | {
      type: TooltipTextDecorationType;
      color?: unknown;
    }
  | null
  | undefined;

type TooltipFillStop = string | { color?: string | null };

type TooltipSolidFill = { type: 'solid'; color?: string | null };
type TooltipGradientFill = { type: 'linear' | 'radial' | 'angular'; stops: TooltipFillStop[] };
type TooltipRgbFill = { r?: number; g?: number; b?: number; a?: number };
type TooltipObjectFill = { type: string } & Partial<TooltipRgbFill> & Record<string, unknown>;
type TooltipFill = string | TooltipObjectFill | Array<string | TooltipObjectFill> | null | undefined;

type TooltipTextLike = {
  fill?: unknown;
  padding?: number | number[];
  textWrap?: 'none' | 'break' | string;
  textOverflow?: 'show' | 'hide' | string;
  textDecoration?: TooltipTextDecoration;
  fontFamily?: string;
  fontSize: number;
  italic?: boolean;
  fontWeight?: string | number;
  textCase: keyof typeof textCaseMap;
  verticalAlign: keyof typeof verticalAlignMap;
  textAlign: 'both' | 'left' | 'center' | 'right' | string;
  paraIndent?: number;
  __: {
    __lineHeight?: number;
    __letterSpacing?: number;
    __autoWidth?: boolean;
  };
};

export class TooltipTextEditor extends TextEditor {
  public get tag() {
    return 'TooltipTextEditor';
  }

  public onLoad(): void {
    super.onLoad();
    if (!this.editDom) return;
    if (this.editDom.parentElement !== document.body) {
      document.body.appendChild(this.editDom);
    }
    this.inBody = true;
    this.editDom.focus();
    this.restoreSelection();
  }

  public onUpdate(): void {
    const { editTarget: text, editDom } = this;
    if (!text || !editDom) return;

    let textScale = 1;
    if (!this.isHTMLText) {
      const { scaleX, scaleY } = text.worldTransform;
      textScale = Math.max(Math.abs(scaleX), Math.abs(scaleY));

      const fontSize = text.fontSize * textScale;
      if (fontSize < 12) textScale *= 12 / text.fontSize;
    }

    this.textScale = textScale;

    let { width, height } = text;
    let offsetX = 0;
    let offsetY = 0;
    width *= textScale;
    height *= textScale;

    const data = text.__;
    if (data.__autoWidth) {
      width += 20;
      switch (data.textAlign) {
        case 'center':
          offsetX = data.autoSizeAlign ? -width / 2 : -10;
          break;
        case 'right':
          offsetX = data.autoSizeAlign ? -width : -20;
          break;
      }
    }

    if (data.__autoHeight) {
      height += 20;
      switch (data.verticalAlign) {
        case 'middle':
          offsetY = data.autoSizeAlign ? -height / 2 : -10;
          break;
        case 'bottom':
          offsetY = data.autoSizeAlign ? -height : -20;
          break;
      }
    }

    const appLike = text.app as { clientBounds?: { x?: number; y?: number }; view?: HTMLElement | null | undefined };
    const view = appLike.view;
    const viewBounds = view && typeof view.getBoundingClientRect === 'function' ? view.getBoundingClientRect() : null;
    const clientBounds = appLike.clientBounds;
    const x = clientBounds?.x ?? viewBounds?.left ?? 0;
    const y = clientBounds?.y ?? viewBounds?.top ?? 0;
    const world = text.worldTransform as { a?: number; b?: number; c?: number; d?: number; e?: number; f?: number };
    const worldA = world.a ?? 1;
    const worldB = world.b ?? 0;
    const worldC = world.c ?? 0;
    const worldD = world.d ?? 1;
    const worldE = world.e ?? 0;
    const worldF = world.f ?? 0;

    // Match Leafer Matrix exactly:
    // new Matrix(worldTransform).scale(1 / textScale).translateInner(offsetX, offsetY)
    const a = worldA / textScale;
    const b = worldB / textScale;
    const c = worldC / textScale;
    const d = worldD / textScale;
    const e = worldE + a * offsetX + c * offsetY;
    const f = worldF + b * offsetX + d * offsetY;

    const { style } = editDom;
    style.transform = `matrix(${a},${b},${c},${d},${e},${f})`;
    style.left = `${x}px`;
    style.top = `${y}px`;
    style.width = `${width}px`;
    style.height = `${height}px`;

    if (!this.isHTMLText) {
      updateTooltipTextEditorStyle(editDom, text as unknown as TooltipTextLike, textScale);
    }
  }

  private restoreSelection(): void {
    const { editDom } = this;
    if (!editDom) return;
    const selection = window.getSelection?.();
    if (!selection) return;
    const range = document.createRange();
    if (this.config.selectAll) {
      range.selectNodeContents(editDom);
    } else {
      const node = editDom.childNodes[0];
      if (node) {
        range.setStartAfter(node);
        range.setEndAfter(node);
      } else {
        range.selectNodeContents(editDom);
      }
      range.collapse(true);
    }
    selection.removeAllRanges();
    selection.addRange(range);
  }
}

registerInnerEditor()(TooltipTextEditor as never);

function updateTooltipTextEditorStyle(textDom: HTMLDivElement, text: TooltipTextLike, textScale: number): void {
  const { style } = textDom;
  const { fill, padding, textWrap, textOverflow } = text;
  const textDecoration = text.textDecoration as TooltipTextDecoration;

  style.fontFamily = text.fontFamily;
  style.fontSize = `${text.fontSize * textScale}px`;
  setFill(style, fill);

  style.fontStyle = text.italic ? 'italic' : 'normal';
  style.fontWeight = text.fontWeight as string;

  let decorationType: TooltipTextDecorationType;
  if (textDecoration && typeof textDecoration === 'object' && !Array.isArray(textDecoration)) {
    decorationType = textDecoration.type;
    if (textDecoration.color) style.textDecorationColor = String(textDecoration.color);
  } else {
    decorationType = (typeof textDecoration === 'string' ? textDecoration : 'none') as TooltipTextDecorationType;
  }
  style.textDecoration = textDecorationMap[decorationType];

  style.textTransform = textCaseMap[text.textCase];
  style.display = 'flex';
  style.flexDirection = 'column';
  style.justifyContent = verticalAlignMap[text.verticalAlign];
  style.textAlign = text.textAlign === 'both' ? 'justify' : text.textAlign;
  style.lineHeight = `${(text.__.__lineHeight || 0) * textScale}px`;
  style.letterSpacing = `${(text.__.__letterSpacing || 0) * textScale}px`;
  style.whiteSpace = textWrap === 'none' || text.__.__autoWidth ? 'nowrap' : 'normal';
  style.wordBreak = textWrap === 'break' ? 'break-all' : 'normal';
  style.textIndent = `${(text.paraIndent || 0) * textScale}px`;
  style.padding = Array.isArray(padding)
    ? padding.map((item) => `${item * textScale}px`).join(' ')
    : `${(padding || 0) * textScale}px`;
  style.textOverflow = textOverflow === 'show' ? '' : textOverflow === 'hide' ? 'clip' : textOverflow;
}

function setFill(style: CSSStyleDeclaration, fill: unknown): void {
  let color = 'black';

  let resolvedFill = fill as TooltipFill;

  if (Array.isArray(resolvedFill)) resolvedFill = resolvedFill[0];

  if (resolvedFill && typeof resolvedFill === 'object') {
    const objectFill = resolvedFill as TooltipObjectFill;
    switch (objectFill.type) {
      case 'solid':
        color = String((objectFill as TooltipSolidFill).color);
        break;
      case 'linear':
      case 'radial':
      case 'angular': {
        const stop = (objectFill as TooltipGradientFill).stops[0];
        color = String(typeof stop === 'string' ? stop : stop.color);
        break;
      }
      default:
        if ((objectFill as TooltipRgbFill).r !== undefined)
          color = `rgba(${(objectFill as TooltipRgbFill).r}, ${(objectFill as TooltipRgbFill).g}, ${(objectFill as TooltipRgbFill).b}, ${((objectFill as TooltipRgbFill).a ?? 1) / 255})`;
        break;
    }
  } else {
    color = typeof resolvedFill === 'string' ? resolvedFill : color;
  }

  style.color = color;
}
