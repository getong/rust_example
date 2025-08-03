// tests/http_import.ts

import { assert } from "https://deno.land/std@0.195.0/assert/mod.ts";

function test_four(num1: number, num2: number): boolean {
    return (num1 + num2) === 4;
}

assert(test_four(2, 2));
console.log("Asserted that 2 + 2 = 4")