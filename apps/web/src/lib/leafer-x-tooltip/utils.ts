export const PLUGIN_NAME = 'leafer-x-tooltip'
export const ATTRS_NAME = 'data-lxt-id'

export function assert(condition: boolean, msg: string) {
  if (condition) {
    throw new Error(`[${PLUGIN_NAME}]: ${msg}`)
  }
}

export function addStyle(element: HTMLElement, cssStyles: Record<string, string>) {
  requestAnimationFrame(() => {
    Object.entries(cssStyles).forEach(([property, value]) => {
      element.style[property as any] = value
    })
  })
}

export function randomStr(length = 8) {
  return Math.random().toString(36).slice(2, length + 2)
}

export function allowNodeType(includeTypes: Array<string>, type: string): boolean {
  if (!Array.isArray(includeTypes)) return true
  if (includeTypes.length === 0) return true
  return includeTypes.includes(type)
}

export function denyNodeType(excludeTypes: Array<string>, type: string): boolean {
  if (!Array.isArray(excludeTypes)) return false
  if (excludeTypes.length === 0) return false
  return excludeTypes.includes(type)
}

export function getTooltip(dataId: string): HTMLElement | null {
  return document.querySelector(`[${ATTRS_NAME}=${dataId}]`)
}

export function camelCaseToDash(str: string) {
  return str.replace(/([A-Z])/g, '-$1').toLowerCase()
}

export function createCssClass(
  selector: string,
  useRules: string | Record<string, string>,
  userStyleElement?: HTMLStyleElement
) {
  let styleElement = userStyleElement
  if (!styleElement || !(userStyleElement instanceof HTMLStyleElement)) {
    styleElement = document.createElement('style')
    styleElement.setAttribute(PLUGIN_NAME, '')
    document.head.appendChild(styleElement)
  }

  let rules = typeof useRules === 'string' ? useRules : ''
  if (typeof useRules === 'object') {
    Object.keys(useRules).forEach((prop: string) => {
      rules += `${camelCaseToDash(prop)}: ${useRules[prop]};`
    })
  }

  if (styleElement.sheet) {
    styleElement.sheet.insertRule(`${selector} { ${rules} }`, 0)
  } else {
    styleElement.appendChild(document.createTextNode(rules))
  }

  return styleElement
}
