from flask import Flask, jsonify
import redis
import os
import socket

app = Flask(__name__)
redis_host = os.environ.get('REDIS_HOST', 'localhost')
redis_port = int(os.environ.get('REDIS_PORT', 6379))

try:
    r = redis.Redis(host=redis_host, port=redis_port, db=0, socket_connect_timeout=2)
except Exception as e:
    r = None

@app.route('/')
def hello():
    hostname = socket.gethostname()
    visits = "unknown"
    db_status = "disconnected"
    
    if r:
        try:
            visits = r.incr('counter')
            db_status = "connected"
        except Exception:
            pass

    return jsonify({
        "message": "Hello from Ansible Piloteer Demo App",
        "hostname": hostname,
        "visits": visits,
        "db_status": db_status,
        "backend": "python-flask"
    })

if __name__ == "__main__":
    app.run(host='0.0.0.0', port=5000)
