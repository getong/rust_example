import jsonTest1 from "./json_test1.json" with { type: "json" };

console.log(JSON.stringify(jsonTest1));

const jsonModuleNamespace = await import("./json_test2.json", {
  with: { type: "json" },
});
console.log(JSON.stringify(jsonModuleNamespace));
