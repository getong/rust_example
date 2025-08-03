export function addToHostState(
    n: number,
): Promise<number> {
    return globalThis.opScriptingDemo(n);
}
