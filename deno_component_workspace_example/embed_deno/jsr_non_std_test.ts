import {
  camelCase,
  constantCase,
  kebabCase,
  pascalCase,
  snakeCase,
  splitPieces,
  titleCase,
} from "jsr:@luca/cases";

const input = "Stream API Token Generator";
const pieces = splitPieces(input);

const payload = {
  input,
  pieces,
  camelCase: camelCase(input),
  snakeCase: snakeCase(input),
  kebabCase: kebabCase(input),
  titleCase: titleCase(input),
  pascalCase: pascalCase(input),
  constantCase: constantCase(input),
};

console.log("[jsr_non_std_test] payload:", payload);
globalThis.embedDeno?.setResult(payload);
globalThis.embedDeno?.setExitData({ ok: true, kind: "jsr_non_std_test" });
