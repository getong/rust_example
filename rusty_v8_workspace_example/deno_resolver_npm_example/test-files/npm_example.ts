import express from "npm:express@4.18.2";
import chalk from "npm:chalk@5.3.0";
import dayjs from "npm:dayjs@1.11.10";

declare const ExampleExtension: {
  exampleCustomOp: (text: string) => string;
};

console.log(chalk.blue("Starting npm example..."));

const now = dayjs();
console.log(chalk.green(`Current time: ${now.format('YYYY-MM-DD HH:mm:ss')}`));

const app = express();

app.get("/", (_req, res) => {
  const message = ExampleExtension.exampleCustomOp("Hello from npm modules");
  res.send(`${message}\n\nCurrent time: ${now.format()}`);
});

app.get("/api/time", (_req, res) => {
  res.json({
    timestamp: Date.now(),
    formatted: dayjs().format('YYYY-MM-DD HH:mm:ss'),
    message: "Time from Deno with npm modules"
  });
});

const port = 3000;
app.listen(port, () => {
  console.log(chalk.yellow(`Express server running on http://localhost:${port}`));
  console.log(chalk.cyan("Press Ctrl+C to stop the server"));
});