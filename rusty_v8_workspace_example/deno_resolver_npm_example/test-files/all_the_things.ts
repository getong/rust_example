import * as cowsay from "https://esm.sh/cowsay@1.6.0";

declare const ExampleExtension: {
  exampleCustomOp: (text: string) => string;
};

const text = ExampleExtension.exampleCustomOp("Hello, World");

const textPlusCowsay = text + "\n\n" + cowsay.say({ text: "🤠 🚀" });

const encoder = new TextEncoder();
const data = encoder.encode(textPlusCowsay);

await Deno.writeFile("hello.txt", data);

console.log("File written successfully!");
