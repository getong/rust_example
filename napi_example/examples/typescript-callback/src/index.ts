import { loadNativeBinding } from './native'

const native = loadNativeBinding<{
  runTypescriptCallback: (handler: (payload: string) => string, topic: string) => string
  emitTypescriptEvents: (handler: (payload: string) => void, values: string[]) => void
  callTypescriptTransformer: (
    handler: (value: string, multiplier: number) => string,
    input: string,
    multiplier: number,
  ) => string
  orchestrateTypescriptDecision: (
    ask: (topic: string) => boolean,
    onApproved: (message: string) => void,
    onDenied: (message: string) => void,
    topic: string,
  ) => boolean
}>()

const response = native.runTypescriptCallback(payload => {
  console.log(`[TypeScript] Rust says: ${payload}`)
  return payload.toUpperCase()
}, 'TypeScript callbacks')

console.log(`[Node] Rust received -> ${response}`)

native.emitTypescriptEvents(message => {
  console.log(`[TypeScript] Event from Rust: ${message}`)
}, ['alpha', 'beta', 'gamma'])

const transformed = native.callTypescriptTransformer((value, multiplier) => {
  console.log(`[TypeScript] transform request for "${value}" x${multiplier}`)
  return `${value.toUpperCase()} :: multiplied ${multiplier}`
}, 'Rust <-> TypeScript', 2)

console.log(`[Node] callTypescriptTransformer returned: ${transformed}`)

const decision = native.orchestrateTypescriptDecision(
  topic => {
    const approved = topic.length % 2 === 0
    console.log(`[TypeScript] deciding whether to approve "${topic}" -> ${approved}`)
    return approved
  },
  message => console.log(`[TypeScript] approval callback: ${message}`),
  message => console.warn(`[TypeScript] rejection callback: ${message}`),
  'Ship napi example',
)

console.log(`[Node] orchestrateTypescriptDecision result: ${decision}`)
