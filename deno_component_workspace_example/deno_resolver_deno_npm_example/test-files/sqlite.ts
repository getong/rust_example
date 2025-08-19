import { createClient } from "https://esm.sh/@libsql/client@0.14.0";

const client = createClient({
  url: "file:local.db",
});

await client.batch(
  [
    "CREATE TABLE IF NOT EXISTS users (email TEXT)",
    "INSERT INTO users VALUES ('first@example.com')",
    "INSERT INTO users VALUES ('second@example.com')",
    "INSERT INTO users VALUES ('third@example.com')",
  ],
  "write",
);

const result = await client.execute("SELECT * FROM users");

console.log("Users:", result.rows);
