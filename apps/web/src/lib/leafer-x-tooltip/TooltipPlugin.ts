import {
  addStyle,
  allowNodeType,
  assert,
  ATTRS_NAME,
  createCssClass,
  denyNodeType,
  getTooltip,
  PLUGIN_NAME,
  randomStr,
} from './utils'

type IOffset = number | { x: number; y: number } | [number, number]

type ILeaf = any
type IEventListenerId = any
type LeaferLike = any
type EventEnums = {
  LeaferEvent: any
  PointerEvent: any
}

const DEFAULT_OFFSET = 6
const TOOLTIP_OPEN_DELAY = 500
const TOOLTIP_CLOSE_DELAY = 120
const INTERACTIVE_CLASS_NAME = 'leafer-x-tooltip--interactive'

export type UserConfig = {
  className?: string
  includeTypes?: Array<string>
  excludeTypes?: Array<string>
  triggerType?: 'hover' | 'click'
  offset?: IOffset
  closeDelay?: number
  interactive?: boolean | ((node: ILeaf | null) => boolean)
  resolveNode?: (node: ILeaf | null, event: any) => ILeaf | null
  shouldBegin?: (event: any, node?: ILeaf | null) => boolean
  shouldKeepOpen?: () => boolean
  getContent: (node: ILeaf) => string
  onOpen?: (container: HTMLElement, node: ILeaf) => void
  onUpdate?: (container: HTMLElement, node: ILeaf) => void
  onClose?: (container: HTMLElement, node: ILeaf | null) => void
  events: EventEnums
}

export class TooltipPlugin {
  private readonly app: LeaferLike
  private readonly domId: string
  private readonly config: UserConfig
  private readonly events: EventEnums
  private activeNode: ILeaf | null
  private openTimer: number | null
  private isHoveringTooltip: boolean
  private isHoveringNode: boolean
  private isFrozen: boolean
  private lastPosition: { x: number; y: number } | null
  private pendingEvent: MouseEvent | PointerEvent | null
  private lastEvent: MouseEvent | PointerEvent | null
  private closeTimer: number | null
  private readonly bindEventIds: Array<IEventListenerId>
  private readonly _moveTooltip: (event: MouseEvent) => void
  private readonly _hideTooltip: (event: MouseEvent) => void
  private hoverDomHost: HTMLElement | null
  private isOpen: boolean
  private openedNode: ILeaf | null

  public styleSheetElement: HTMLStyleElement

  private ensureTriggerType(): void {
    if (!this.config.triggerType) {
      this.config.triggerType = 'hover'
    }
  }

  private resetState(): void {
    this.openTimer = null
    this.activeNode = null
    this.isHoveringTooltip = false
    this.isHoveringNode = false
    this.isFrozen = false
    this.lastPosition = null
    this.pendingEvent = null
    this.lastEvent = null
    this.closeTimer = null
    this.isOpen = false
    this.openedNode = null
  }

  private bindContainerEvents(container: HTMLElement): void {
    container.addEventListener('mouseenter', () => {
      if (this.config.triggerType !== 'hover') return
      this.isHoveringTooltip = true
      this.isHoveringNode = false
      this.clearCloseTimer()
    })
    container.addEventListener('mouseleave', (event) => {
      if (this.config.triggerType !== 'hover') return
      if (this.isTooltipTarget(event.relatedTarget)) return
      this.isHoveringTooltip = false
      this.isFrozen = false
      if (!this.isHoveringNode) {
        this.scheduleClose()
      }
    })
    container.addEventListener('mousedown', () => {
      this.isFrozen = true
      this.clearCloseTimer()
    })
    container.addEventListener('mouseup', () => {
      const selection = window.getSelection?.()
      if (!selection || selection.type !== 'Range') {
        this.isFrozen = false
        if (!this.isHoveringTooltip && !this.isHoveringNode) {
          this.scheduleClose()
        }
      }
    })
  }

  private setTooltipContent(container: HTMLElement): boolean {
    const content = this.getTooltipContent()
    if (content === null) {
      this.hideTooltip()
      return false
    }
    container.innerHTML = content
    this.openedNode = this.activeNode
    return true
  }

  private refreshFrozenState(): void {
    if (!this.isFrozen) return
    const selection = window.getSelection?.()
    if (!selection || selection.type !== 'Range') {
      this.isFrozen = false
    }
  }

  private isInteractiveNode(node: ILeaf | null): boolean {
    const interactive = this.config.interactive
    if (typeof interactive === 'function') {
      return !!interactive(node)
    }
    return !!interactive
  }

  private syncInteractiveState(tooltipContainer: HTMLElement): boolean {
    const interactive = this.isInteractiveNode(this.activeNode)
    tooltipContainer.classList.toggle(INTERACTIVE_CLASS_NAME, interactive)
    return interactive
  }

  private applyTooltipPosition(tooltipContainer: HTMLElement, position: { x: number; y: number }): void {
    const interactive = this.syncInteractiveState(tooltipContainer)
    addStyle(tooltipContainer, {
      display: 'block',
      position: 'absolute',
      left: `${position.x}px`,
      top: `${position.y}px`,
      pointerEvents: interactive ? 'auto' : 'none',
    })
  }

  private updateTooltipContent(tooltipContainer: HTMLElement): boolean {
    const shouldRefreshContent = !this.isOpen || this.openedNode !== this.activeNode
    if (!shouldRefreshContent) {
      this.syncInteractiveState(tooltipContainer)
      return false
    }
    if (this.isOpen && this.openedNode !== this.activeNode) {
      this.closeTooltipContent(tooltipContainer)
    }
    return this.setTooltipContent(tooltipContainer)
  }

  private openTooltipIfNeeded(tooltipContainer: HTMLElement, shouldRefreshContent: boolean): void {
    if (!this.activeNode) return
    if (shouldRefreshContent) {
      this.config.onOpen?.(tooltipContainer, this.activeNode)
      this.isOpen = true
      return
    }
    if (this.isOpen) {
      this.config.onUpdate?.(tooltipContainer, this.activeNode)
    }
  }

  private isInvalidNode(node: ILeaf): boolean {
    return !node || node.isLeafer || node.isApp
  }

  constructor(app: LeaferLike, config: UserConfig) {
    this.app = app
    this.config = config
    this.events = config.events
    this.ensureTriggerType()
    this.domId = `lxt--${randomStr(8)}`
    this.bindEventIds = []
    this.resetState()
    this.initEvent()
    this.initCssClass()
    this.initCreateTooltip()
    this._moveTooltip = (event) => this.moveTooltip(event)
    this._hideTooltip = (event) => this.handleCanvasLeave(event)
    this.hoverDomHost = null
  }

  private initEvent() {
    const eventIds = []
    let event = this.events.PointerEvent.MOVE
    let eventFunc = this.leaferPointMove

    if (this.config.triggerType === 'click') {
      event = this.events.PointerEvent.CLICK
      eventFunc = this.leaferPointClick
    }

    eventIds.push(this.app.on_(event, eventFunc, this))
    eventIds.push(this.app.on_(this.events.LeaferEvent.VIEW_READY, this.viewReadyEvent, this))

    this.bindEventIds.push(...eventIds)
  }

  private shouldShowTooltip(node: ILeaf, event: any): boolean {
    const resolvedNode = this.config.resolveNode ? this.config.resolveNode(node ?? null, event) : node
    if (this.isInvalidNode(resolvedNode)) {
      this.isHoveringNode = false
      this.clearOpenTimer()
      if (!this.isHoveringTooltip) {
        this.scheduleClose()
      }
      return null as never
    }
    const isAllowType = allowNodeType(this.config.includeTypes, resolvedNode.tag)
    const isDenyType = denyNodeType(this.config.excludeTypes, resolvedNode.tag)
    const isShouldBegin = this.config.shouldBegin ? this.config.shouldBegin(event, resolvedNode) : true
    if (!isAllowType || isDenyType || !isShouldBegin) {
      this.isHoveringNode = false
      this.clearOpenTimer()
      if (!this.isHoveringTooltip) {
        this.scheduleClose()
      }
      return null as never
    }
    this.isHoveringNode = true
    return resolvedNode as never
  }

  private handleHoverNode(node: ILeaf, event: any): ILeaf | null {
    const resolvedNode = this.shouldShowTooltip(node, event)
    if (!resolvedNode) return null
    if (this.activeNode === resolvedNode) {
      return resolvedNode
    }
    this.activeNode = resolvedNode
    this.isFrozen = false
    this.lastPosition = null

    if (this.config.triggerType === 'hover') {
      this.scheduleOpen(event)
      return null
    }

    return resolvedNode
  }

  private leaferPointMove(event: any) {
    this.handleHoverNode(event.target, event)
  }

  private leaferPointClick(event: any) {
    const node = event.target
    const resolvedNode = this.shouldShowTooltip(node, event)
    if (!resolvedNode) return

    if (this.activeNode === resolvedNode) {
      this.hideTooltip()
      return
    }

    this.activeNode = resolvedNode

    if (this.app.view instanceof HTMLElement) {
      this.moveTooltip(event)
    }
  }

  private viewReadyEvent(): void {
    if (!(this.app.view instanceof HTMLElement)) return
    assert(this.app.view?.addEventListener === undefined, 'leafer.view 加载失败！')

    if (this.config.triggerType === 'hover') {
      this.app.view.addEventListener('mousemove', this._moveTooltip)
      this.app.view.addEventListener('mouseleave', this._hideTooltip)
    }
  }

  private initCssClass() {
    if (this.styleSheetElement) return
    const styleSheetElement = document.querySelector(`.${PLUGIN_NAME}`)
    if (styleSheetElement) {
      this.styleSheetElement = styleSheetElement as HTMLStyleElement
      return
    }
    this.styleSheetElement = createCssClass(`.${PLUGIN_NAME}`, {
      border: 'none',
      borderRadius: '6px',
      padding: '10px 14px',
      backgroundColor: 'rgba(255, 255, 255, 0.95)',
      color: '#333',
      fontSize: '13px',
      fontWeight: '400',
      boxShadow: '0 3px 14px rgba(0, 0, 0, 0.15)',
      backdropFilter: 'blur(8px)',
      transition: 'opacity 0.2s ease-in-out',
    })
  }

  private initCreateTooltip(): HTMLElement {
    let container: HTMLElement | null = getTooltip(this.domId)
    const isExists = container !== null

    if (!container) {
      container = document.createElement('div')
    }
    container.setAttribute(ATTRS_NAME, this.domId)
    container.style.display = 'none'
    if (this.config.className) {
      container.className = this.config.className
    } else if (!isExists) {
      container.className = PLUGIN_NAME
    }

    if (!isExists) {
      document.body.appendChild(container)
      this.bindContainerEvents(container)
    }

    return container
  }

  private hideTooltip(): void {
    this.activeNode = null
    this.isHoveringNode = false
    this.isHoveringTooltip = false
    this.isFrozen = false
    this.lastPosition = null
    this.pendingEvent = null
    this.clearOpenTimer()
    this.clearCloseTimer()

    const tooltipDOM = getTooltip(this.domId)
    if (tooltipDOM) {
      this.closeTooltipContent(tooltipDOM)
      tooltipDOM.classList.remove(INTERACTIVE_CLASS_NAME)
      tooltipDOM.style.display = 'none'
      tooltipDOM.style.pointerEvents = 'auto'
    }
  }

  private calculateTooltipPosition(event: any, tooltipElem: HTMLElement): { x: number; y: number } {
    const windowWidth = window.innerWidth
    const windowHeight = window.innerHeight
    const pageXOffset = window.scrollX
    const pageYOffset = window.scrollY

    let mouseX = 0
    let mouseY = 0

    const origin = event?.origin
    if (origin && typeof origin.x === 'number' && typeof origin.y === 'number') {
      mouseX = origin.x + pageXOffset
      mouseY = origin.y + pageYOffset
    } else if (typeof event?.clientX === 'number' && typeof event?.clientY === 'number') {
      mouseX = event.clientX + pageXOffset
      mouseY = event.clientY + pageYOffset
    } else {
      mouseX = (event as any).x + pageXOffset
      mouseY = (event as any).y + pageYOffset
    }

    const tooltipWidth = tooltipElem.offsetWidth
    const tooltipHeight = tooltipElem.offsetHeight

    const offset = this.getOffset()

    let x = mouseX + offset.x
    let y = mouseY + offset.y

    if (x + tooltipWidth > windowWidth + pageXOffset) {
      x = mouseX - tooltipWidth - offset.x
    }

    if (y + tooltipHeight > windowHeight + pageYOffset) {
      y = mouseY - tooltipHeight - offset.y
    }

    const minX = pageXOffset
    const minY = pageYOffset
    const maxX = Math.max(minX, windowWidth + pageXOffset - tooltipWidth)
    const maxY = Math.max(minY, windowHeight + pageYOffset - tooltipHeight)
    x = Math.min(Math.max(x, minX), maxX)
    y = Math.min(Math.max(y, minY), maxY)

    return { x, y }
  }

  private getTooltipContent(): string | null {
    const argumentType = typeof this.config.getContent
    assert(argumentType !== 'function', `getContent 为必传参数，且必须是一个函数，当前为：${argumentType} 类型`)
    if (!this.activeNode) return null
    const content = this.config.getContent(this.activeNode)
    if (content === undefined || content === null || content === '') return null
    return content
  }

  private closeTooltipContent(tooltipDOM: HTMLElement): void {
    if (!this.isOpen) return
    this.config.onClose?.(tooltipDOM, this.openedNode)
    this.isOpen = false
    this.openedNode = null
  }

  private syncActiveNodeFromDomEvent(event: MouseEvent | PointerEvent): boolean {
    if (!this.config.resolveNode || this.config.triggerType !== 'hover') return false
    const resolvedNode = this.shouldShowTooltip(null as never, event)
    if (!resolvedNode) return false
    if (this.activeNode === resolvedNode) return true
    this.activeNode = resolvedNode
    this.isFrozen = false
    this.lastPosition = null
    this.scheduleOpen(event)
    return false
  }

  private moveTooltip(event: MouseEvent | PointerEvent): void {
    if (!this.activeNode && !this.syncActiveNodeFromDomEvent(event)) return
    if (!this.activeNode) return
    this.lastEvent = event
    if (this.config.triggerType === 'hover' && this.openTimer) {
      this.pendingEvent = event
      return
    }
    const tooltipContainer = this.getTooltipElement() ?? this.initCreateTooltip()
    const shouldRefreshContent = this.updateTooltipContent(tooltipContainer)
    this.refreshFrozenState()

    let position: { x: number; y: number }
    if (this.lastPosition) {
      position = this.lastPosition
    } else {
      position = this.calculateTooltipPosition(event, tooltipContainer)
      this.lastPosition = position
    }

    this.applyTooltipPosition(tooltipContainer, position)
    this.openTooltipIfNeeded(tooltipContainer, shouldRefreshContent)
  }

  private scheduleOpen(event: MouseEvent | PointerEvent): void {
    this.clearOpenTimer()
    this.pendingEvent = event
    this.openTimer = window.setTimeout(() => {
      if (!this.activeNode) return
      this.openTimer = null
      const pendingEvent = this.pendingEvent ?? event
      this.moveTooltip(pendingEvent)
    }, TOOLTIP_OPEN_DELAY)
  }

  private clearOpenTimer(): void {
    if (this.openTimer === null) return
    window.clearTimeout(this.openTimer)
    this.openTimer = null
  }

  private getTooltipElement(): HTMLElement | null {
    return getTooltip(this.domId)
  }

  private isTooltipTarget(target: EventTarget | null): boolean {
    if (!(target instanceof Node)) return false
    const tooltipDOM = this.getTooltipElement()
    return !!tooltipDOM?.contains(target)
  }

  private shouldKeepTooltipOpen(): boolean {
    this.refreshFrozenState()
    return this.isFrozen || !!this.config.shouldKeepOpen?.()
  }

  private handleCanvasLeave(event?: MouseEvent): void {
    if (event && this.isTooltipTarget(event.relatedTarget)) return
    this.isHoveringNode = false
    this.isFrozen = false
    if (!this.isHoveringTooltip) {
      this.scheduleClose()
    }
  }

  private scheduleClose(): void {
    if (this.closeTimer !== null) return
    if (this.shouldKeepTooltipOpen()) return
    const closeDelay = this.config.closeDelay ?? TOOLTIP_CLOSE_DELAY
    this.closeTimer = window.setTimeout(() => {
      this.closeTimer = null
      if (this.shouldKeepTooltipOpen()) {
        this.scheduleClose()
        return
      }
      if (!this.isHoveringTooltip && !this.isHoveringNode) {
        this.hideTooltip()
      }
    }, closeDelay)
  }

  private clearCloseTimer(): void {
    if (this.closeTimer === null) return
    window.clearTimeout(this.closeTimer)
    this.closeTimer = null
  }

  public refreshVisibility(): void {
    if (this.shouldKeepTooltipOpen()) return
    if (!this.isHoveringTooltip && !this.isHoveringNode) {
      this.scheduleClose()
    }
  }

  public refreshPosition(): void {
    const tooltipDOM = this.getTooltipElement()
    const event = this.lastEvent ?? this.pendingEvent
    if (!tooltipDOM || !event || tooltipDOM.style.display === 'none') return
    this.lastPosition = null
    const position = this.calculateTooltipPosition(event, tooltipDOM)
    this.lastPosition = position
    this.applyTooltipPosition(tooltipDOM, position)
  }

  public getDomId() {
    return this.domId
  }

  public getOffset(): { x: number; y: number } {
    const offset = this.config.offset
    if (typeof offset === 'number') return { x: offset, y: offset }
    if (Array.isArray(offset)) {
      const [x, y] = offset
      return { x, y }
    }
    if (typeof offset === 'object') return offset
    return { x: DEFAULT_OFFSET, y: DEFAULT_OFFSET }
  }

  public createStyleRule(selector: string, useRules: string | Record<string, string>) {
    createCssClass(`${selector}[${ATTRS_NAME}=${this.domId}]`, useRules, this.styleSheetElement)
  }

  public removeStyleRule(selector: string) {
    const styleSheet = this.styleSheetElement.sheet
    if (!styleSheet) return
    const index = this.findStyleRuleIndex(selector)
    if (index === -1) return
    styleSheet.deleteRule(index)
  }

  public findStyleRuleIndex(selector: string): number {
    const styleSheet = this.styleSheetElement.sheet
    if (!styleSheet) return -1
    const rules = styleSheet.cssRules
    const fullSelector = `${selector}[${ATTRS_NAME}=${this.domId}]`
    for (let i = 0; i < rules.length; i++) {
      const rule = rules[i] as CSSStyleRule
      if (rule.selectorText === fullSelector) return i
    }
    return -1
  }

  public addClass(className: string | string[]) {
    const container = getTooltip(this.domId)
    if (container) {
      if (Array.isArray(className)) {
        className.forEach((item) => container.classList.add(item))
      } else {
        container.classList.add(className)
      }
    }
  }

  public removeClass(className: string | string[]) {
    const container = getTooltip(this.domId)
    if (container) {
      if (Array.isArray(className)) {
        className.forEach((item) => container.classList.remove(item))
      } else {
        container.classList.remove(className)
      }
    }
  }

  public destroy() {
    this.app.off_(this.bindEventIds)
    this.bindEventIds.length = 0
    if (this.app.view instanceof HTMLElement) {
      this.app.view.removeEventListener('mousemove', this._moveTooltip)
      this.app.view.removeEventListener('mouseleave', this._hideTooltip)
    }
    this.activeNode = null
    this.pendingEvent = null
    this.clearOpenTimer()
    this.clearCloseTimer()
    const tooltipDOM = getTooltip(this.domId)
    if (tooltipDOM && tooltipDOM.parentNode) {
      this.closeTooltipContent(tooltipDOM)
      tooltipDOM.parentNode.removeChild(tooltipDOM)
    }
  }

  public setTriggerType(triggerType: 'hover' | 'click'): void {
    if (this.config.triggerType === triggerType) return

    const prevTriggerType = this.config.triggerType

    this.app.off_(this.bindEventIds)
    this.bindEventIds.length = 0

    this.config.triggerType = triggerType

    this.initEvent()

    if (this.app.view instanceof HTMLElement) {
      if (triggerType === 'click' && prevTriggerType === 'hover') {
        this.app.view.removeEventListener('mousemove', this._moveTooltip)
        this.app.view.removeEventListener('mouseleave', this._hideTooltip)
      } else if (triggerType === 'hover' && prevTriggerType === 'click') {
        this.app.view.addEventListener('mousemove', this._moveTooltip)
        this.app.view.addEventListener('mouseleave', this._hideTooltip)
      }
    }

    this.hideTooltip()
  }
}
