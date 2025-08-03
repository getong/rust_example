// import * as cowsay from "./module.js";
import("https://esm.sh/cowsay@1.6.0").then((cowsay) => {
  console.log(cowsay.say({ text: "Hi! 😃" }));
});

// console.log("Hello world from JS!");

// console.log("Deno.build", Deno.build);
// Extension.hello("world 🚀");

// Deno.serve((req) => {
//   return new Response("Hello world from JS!");
// });
