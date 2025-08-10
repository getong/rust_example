import * as http from "node:http";

export function default() {
    console.log("Starting Node.js HTTP server...");
    
    const server = http.createServer((req, res) => {
        console.log(req.rawHeaders);
        console.log(req.rawHeaders.toString());
        console.log(JSON.stringify(req.rawHeaders));
        console.log(typeof req.rawHeaders);
        console.log(Object.keys(req.rawHeaders));
        console.log(Object.getOwnPropertyNames(req.rawHeaders));
        console.log(Array.isArray(req.rawHeaders));
        console.log(Object.getPrototypeOf(req.rawHeaders));
        console.log("-------------");
        res.end();
    });
    
    server.listen(80, () => {
        console.log("Server listening on port 80");
    });
    
    return server;
}