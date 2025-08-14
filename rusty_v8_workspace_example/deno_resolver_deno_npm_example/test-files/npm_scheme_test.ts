// Test npm: scheme imports
import chalk from "npm:chalk@5.3.0";
import lodash from "npm:lodash@4.17.21";

console.log("Testing npm: scheme imports...");

// Test chalk
const redText = chalk.red("This text should be red!");
console.log(redText);

// Test lodash
const numbers = [1, 2, 3, 4, 5];
const doubled = lodash.map(numbers, (n: number) => n * 2);
console.log("Original numbers:", numbers);
console.log("Doubled numbers:", doubled);

// Test dynamic import with npm: scheme
(async () => {
  const { default: dayjs } = await import("npm:dayjs@1.11.10");
  console.log(
    "Current date with dayjs:",
    dayjs().format("YYYY-MM-DD HH:mm:ss"),
  );
})();

console.log("npm: scheme test completed!");
