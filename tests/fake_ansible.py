import socket
import json
import time
import os

SOCKET_PATH = "/tmp/piloteer.sock"

def test_connection():
    print(f"Connecting to {SOCKET_PATH}...")
    try:
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.connect(SOCKET_PATH)
        print("Connected!")
        
        # Handshake
        handshake = {"Handshake": {"token": None}} # No secret by default
        print(f"Sending: {handshake}")
        sock.sendall((json.dumps(handshake) + "\n").encode('utf-8'))
        
        # Wait for Proceed
        data = sock.recv(1024).decode('utf-8')
        print(f"Received: {data}")
        
        if "Proceed" in data:
            print("Handshake Successful!")
            
            # Send Task
            task = {
                "TaskStart": {
                    "name": "Fake Task",
                    "task_vars": {"foo": "bar"},
                    "facts": None
                }
            }
            print(f"Sending Task: {task}")
            sock.sendall((json.dumps(task) + "\n").encode('utf-8'))
            
            # Wait a bit
            time.sleep(2)
            
            # Send Result
            res = {
                "TaskResult": {
                    "name": "Fake Task",
                    "host": "localhost",
                    "changed": False,
                    "failed": False
                }
            }
            print(f"Sending Result: {res}")
            sock.sendall((json.dumps(res) + "\n").encode('utf-8'))
            
    except Exception as e:
        print(f"Connection Failed: {e}")

if __name__ == "__main__":
    if not os.path.exists(SOCKET_PATH):
        print(f"Socket file {SOCKET_PATH} does not exist!")
    else:
        test_connection()
