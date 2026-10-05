// Wire format mirrored from gpui_topcoat_example/protocol/src/lib.rs.
// u64 values must remain within Number.MAX_SAFE_INTEGER in this browser client.
export type CounterAction = 'increment' | 'reset';
export interface CounterSnapshot { value: number; revision: number }
export interface Todo { id: number; title: string; done: boolean }
export type TodoCommand =
  | { action: 'create'; title: string }
  | { action: 'set_done'; id: number; done: boolean }
  | { action: 'delete'; id: number };
export interface Profile { username: string; age: number }
export type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };
export interface DemoSnapshot { todos: Todo[]; echoes: JsonValue[]; profiles: Profile[] }
