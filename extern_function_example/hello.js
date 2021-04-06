const fs = require('fs');
const buf = fs.readFileSync('./target/wasm32-unknown-unknown/debug/extern_function_example.wasm');

function console_log(x) { console.log(x);}

WebAssembly.instantiate(new Uint8Array(buf), {
    env: { "console_log": console_log }
}).then(function(result) {
    result.instance.exports.main();
});
