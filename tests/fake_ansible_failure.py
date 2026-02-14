import socket
import json
import time
import os
import sys

SOCKET_PATH = "/tmp/piloteer_test.sock"

def test_connection():
    print(f"Connecting to {SOCKET_PATH}...")
    # Retry loop
    for i in range(10):
        try:
            sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            sock.connect(SOCKET_PATH)
            print("Connected!")
            break
        except Exception:
            time.sleep(0.5)
    else:
        print("Failed to connect")
        return

    try:
        # Handshake
        handshake = {"Handshake": {"token": None}} 
        sock.sendall((json.dumps(handshake) + "\n").encode('utf-8'))
        
        # Wait for Proceed
        data = sock.recv(1024).decode('utf-8')
        if "Proceed" not in data:
            print("Handshake Failed")
            return

        # Send Task Start
        task_name = "Fail intentionally" # Must match the script!
        task = {
            "TaskStart": {
                "name": task_name,
                "task_vars": {"foo": "bar"},
                "facts": {}
            }
        }
        sock.sendall((json.dumps(task) + "\n").encode('utf-8'))
        
        time.sleep(1)
        
        # Send Task Fail
        fail = {
            "TaskFail": {
                "name": task_name,
                "result": {"msg": "Failed"},
                "facts": {}
            }
        }
        sock.sendall((json.dumps(fail) + "\n").encode('utf-8'))
        
        # Keep connection open for a bit to allow AI analysis to happen and be sent back
        time.sleep(5)
        
    except Exception as e:
        print(f"Error: {e}")
    finally:
        sock.close()

if __name__ == "__main__":
    test_connection()
