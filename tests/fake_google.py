from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import socket
import sys

# Get port from args or default
PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 12352

class MockGoogleHandler(BaseHTTPRequestHandler):
    def do_POST(self):
        # Check URL
        if ":generateContent" not in self.path:
            self.send_error(404, "Not Found")
            return
            
        # Check Key
        if "key=" not in self.path:
            self.send_error(401, "Missing API Key")
            return

        # Check Body
        content_length = int(self.headers['Content-Length'])
        post_data = self.rfile.read(content_length)
        body = json.loads(post_data)
        
        if "contents" not in body:
             self.send_error(400, "Missing contents")
             return

        # Prepare Response
        # The AI Client expects the LLM to output a JSON string for Analysis.
        llm_output = json.dumps({
            "analysis": "Real Google Client Analysis",
            "fix": None
        })
        
        response_data = {
            "candidates": [
                {
                    "content": {
                        "parts": [
                            {
                                "text": f"```json\n{llm_output}\n```"
                            }
                        ]
                    }
                }
            ],
            "usageMetadata": {
                "totalTokenCount": 42
            }
        }
        
        self.send_response(200)
        self.send_header('Content-type', 'application/json')
        self.end_headers()
        self.send_message(json.dumps(response_data).encode('utf-8'))

    def send_message(self, message):
        self.wfile.write(message)

    def log_message(self, format, *args):
        # Suppress logging to keep test output clean
        pass

def run():
    server_address = ('', PORT)
    httpd = HTTPServer(server_address, MockGoogleHandler)
    print(f"Mock Google Server running on port {PORT}")
    httpd.serve_forever()

if __name__ == '__main__':
    run()
