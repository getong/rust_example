import "http://localhost:8888/test1.js";
import dataUrlValue from "data:text/javascript,export default 42;";

console.log("basic_main.js", import.meta.url);
console.log("Data URL value:", dataUrlValue);
