// path/to/javascript/files/example.js

// Function to fetch a personalized greeting
async function fetchGreeting() {
    try {
        const response = await fetch("/greet?name=JavaScript");
        const text = await response.text();
        console.log("Greeting from server:", text);
    } catch (error) {
        console.error("Error fetching greeting:", error);
    }
}

// Function to trigger the inline JavaScript logging on the server
async function triggerInlineJS() {
    try {
        const response = await fetch("/inline-js");
        if (response.ok) {
            console.log("Inline JavaScript endpoint triggered.");
        }
    } catch (error) {
        console.error("Error triggering inline JavaScript:", error);
    }
}

// Run the functions
fetchGreeting();
triggerInlineJS();
