// TypeScript with local module imports

// Define a local module that would normally be an npm package
export function chunk<T>(array: T[], size: number = 1): T[][] {
  const result: T[][] = [];
  for (let i = 0; i < array.length; i += size) {
    result.push(array.slice(i, i + size));
  }
  return result;
}

export function sum(numbers: number[]): number {
  return numbers.reduce((a, b) => a + b, 0);
}

// Simple chalk-like color functions
export const colors = {
  red: (text: string) => `\x1b[31m${text}\x1b[0m`,
  green: (text: string) => `\x1b[32m${text}\x1b[0m`,
  blue: (text: string) => `\x1b[34m${text}\x1b[0m`,
  bold: (text: string) => `\x1b[1m${text}\x1b[0m`,
};

// Simple date formatting
export class SimpleDate {
  private date: Date;

  constructor(date?: Date | string) {
    this.date = date ? new Date(date) : new Date();
  }

  format(template: string): string {
    const year = this.date.getFullYear();
    const month = String(this.date.getMonth() + 1).padStart(2, '0');
    const day = String(this.date.getDate()).padStart(2, '0');
    const hours = String(this.date.getHours()).padStart(2, '0');
    const minutes = String(this.date.getMinutes()).padStart(2, '0');
    const seconds = String(this.date.getSeconds()).padStart(2, '0');

    return template
      .replace('YYYY', String(year))
      .replace('MM', month)
      .replace('DD', day)
      .replace('HH', hours)
      .replace('mm', minutes)
      .replace('ss', seconds);
  }

  add(value: number, unit: 'day' | 'month' | 'year'): SimpleDate {
    const newDate = new Date(this.date);
    switch(unit) {
      case 'day':
        newDate.setDate(newDate.getDate() + value);
        break;
      case 'month':
        newDate.setMonth(newDate.getMonth() + value);
        break;
      case 'year':
        newDate.setFullYear(newDate.getFullYear() + value);
        break;
    }
    return new SimpleDate(newDate);
  }
}

// Main execution
console.log("=== TypeScript with Local Modules ===\n");

// Test array operations
const numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
const chunked = chunk(numbers, 3);
console.log("Original array:", numbers);
console.log("Chunked by 3:", chunked);
console.log("Sum of numbers:", sum(numbers));

// Test colors
console.log("\nColor tests:");
console.log(colors.red("This is red text"));
console.log(colors.green("This is green text"));
console.log(colors.blue("This is blue text"));
console.log(colors.bold("This is bold text"));

// Test date operations
const now = new SimpleDate();
console.log("\nDate operations:");
console.log("Now:", now.format("YYYY-MM-DD HH:mm:ss"));
console.log("Tomorrow:", now.add(1, 'day').format("YYYY-MM-DD HH:mm:ss"));
console.log("Next month:", now.add(1, 'month').format("YYYY-MM-DD"));
console.log("Next year:", now.add(1, 'year').format("YYYY"));

// Advanced TypeScript features
type Operation = 'add' | 'multiply' | 'divide';

function calculate(a: number, b: number, op: Operation): number {
  switch(op) {
    case 'add': return a + b;
    case 'multiply': return a * b;
    case 'divide': return b !== 0 ? a / b : NaN;
  }
}

console.log("\nCalculations:");
console.log("10 + 5 =", calculate(10, 5, 'add'));
console.log("10 * 5 =", calculate(10, 5, 'multiply'));
console.log("10 / 5 =", calculate(10, 5, 'divide'));

// Async operations
async function processData(items: string[]): Promise<string[]> {
  console.log("\nProcessing items asynchronously...");

  const results = await Promise.all(
    items.map(async (item, index) => {
      await new Promise(resolve => setTimeout(resolve, 50 * index));
      return `Processed: ${item.toUpperCase()}`;
    })
  );

  return results;
}

(async () => {
  const items = ['apple', 'banana', 'cherry'];
  const processed = await processData(items);
  processed.forEach(item => console.log(`  ${item}`));

  console.log("\n=== TypeScript Execution Complete ===");
})();