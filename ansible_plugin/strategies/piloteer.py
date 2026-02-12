from ansible.plugins.strategy.linear import StrategyModule as LinearStrategyModule
from ansible.utils.display import Display
import socket
import json
import os
import time

display = Display()

class StrategyModule(LinearStrategyModule):
    def __init__(self, tqm):
        print("DEBUG: Piloteer Strategy Init", flush=True)
        super(StrategyModule, self).__init__(tqm)
        self.sock = None
        self._connect_to_piloteer()

    def _connect_to_piloteer(self):
        socket_path = os.environ.get("PILOTEER_SOCKET", "/tmp/piloteer.sock")
        print(f"DEBUG: Connecting to Piloteer at {socket_path}", flush=True)
        try:
            if ":" in socket_path:
                # Assume TCP: host:port
                host, port = socket_path.split(":", 1)
                self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                self.sock.connect((host, int(port)))
            else:
                # Assume Unix Socket
                self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                self.sock.connect(socket_path)
            
            secret = os.environ.get("PILOTEER_SECRET")
            self._send({"Handshake": {"token": secret}})
            self._wait_for_proceed()
        except Exception as e:
            display.warning(f"Could not connect to Piloteer at {socket_path}: {e}")
            self.sock = None

    def _send(self, data):
        if self.sock:
            try:
                msg = json.dumps(data) + "\n"
            except TypeError:
                 msg = json.dumps({"Error": "Serialization Failed"}) + "\n"
            self.sock.sendall(msg.encode('utf-8'))

    def _serialize_safe(self, obj, depth=0):
        if depth > 5:
            return "<MaxDepth>"
        if isinstance(obj, (str, int, float, bool, type(None))):
            return obj
        if isinstance(obj, dict):
            return {k: self._serialize_safe(v, depth+1) for k, v in obj.items()}
        if isinstance(obj, list):
            return [self._serialize_safe(v, depth+1) for v in obj]
        return str(obj)

    def _wait_for_proceed(self):
        if not self.sock:
            return
        
        buffer = ""
        while True:
            chunk = self.sock.recv(1024).decode('utf-8')
            if not chunk:
                break
            buffer += chunk
            if "\n" in buffer:
                line, buffer = buffer.split("\n", 1)
                try:
                    msg = json.loads(line)
                    if msg == "Proceed":
                        return
                except json.JSONDecodeError:
                    pass

    def run(self, iterator, play_context):
        self.play_context = play_context

        # Send Play Start
        play_name = iterator._play.get_name()
        # iterator._play.hosts could be a list or string, safer to str()
        host_pattern = str(iterator._play.hosts)
        
        print(f"DEBUG: Sending PlayStart: {play_name}", flush=True)
        self._send({
            "PlayStart": {
                "name": play_name,
                "host_pattern": host_pattern
            }
        })

        result = super(StrategyModule, self).run(iterator, play_context)
        print("DEBUG: Run finished, checking stats", flush=True)
        
        # Send Play Recap
        if self._tqm and self._tqm._stats:
            print("DEBUG: Sending PlayRecap", flush=True)
            stats = {}
            stats['ok'] = self._tqm._stats.ok
            stats['failures'] = self._tqm._stats.failures
            stats['changed'] = self._tqm._stats.changed
            stats['skipped'] = self._tqm._stats.skipped
            stats['dark'] = self._tqm._stats.dark
            stats['rescued'] = self._tqm._stats.rescued
            stats['ignored'] = self._tqm._stats.ignored
            
            self._send({"PlayRecap": {"stats": stats}})
            
        return result

    def _process_pending_results(self, iterator, max_passes=1, one_pass=False):
        # Capture previous states for rollback
        try:
            prev_host_states = iterator.host_states.copy()
        except AttributeError:
             prev_host_states = {}

        results = super(StrategyModule, self)._process_pending_results(iterator, max_passes, one_pass)
        print(f"DEBUG: Pending Results Count: {len(results)}", flush=True)
        
        cleaned_results = []
        for res in results:
            if res.is_unreachable():
                # Host Unreachable!
                host = res._host
                task = res._task.get_name()
                result_data = res._result
                
                # Extract connection error
                error_msg = result_data.get('msg', 'Host unreachable')
                
                # Notify Piloteer
                safe_result = self._serialize_safe(result_data)
                self._send({
                    "TaskUnreachable": {
                        "name": task,
                        "host": host.name,
                        "error": error_msg,
                        "result": safe_result
                    }
                })
                # Don't enter debug loop for unreachable - just log and continue
                cleaned_results.append(res)
                continue
                
            if res.is_failed():
                # Task Failed!
                host = res.host
                task = res.task_name
                result_data = res._return_data
                
                # Notify Piloteer
                safe_result = self._serialize_safe(result_data)
                self._send({"TaskFail": {"name": task, "result": safe_result}})
                
                # Enter "Debug Mode" Loop
                while True:
                    cmd_type, cmd_data = self._wait_for_command()
                    
                    if cmd_type == "Retry":
                        # Rollback Failure State
                        
                        # 1. Restore Iterator State
                        if host.name in prev_host_states:
                             iterator.set_state_for_host(host.name, prev_host_states[host.name])
                        
                        # 2. Un-fail host in TQM
                        if host.name in self._tqm._failed_hosts:
                            del self._tqm._failed_hosts[host.name]
                        
                        # 3. Put back in active hosts if it was removed
                        if host.name in iterator._play._removed_hosts:
                             iterator._play._removed_hosts.remove(host.name)
                             
                        # 4. Decrement Stats
                        self._tqm._stats.decrement('failures', host.name)
                        
                        # 5. Re-queue
                        self._blocked_hosts[host.name] = True
                        
                        # Retrieve original task vars and context from cache if possible
                        cached_args = self._queued_task_cache.get((host.name, res._task._uuid))
                        if cached_args:
                             task_vars = cached_args['task_vars']
                             play_context = cached_args['play_context']
                        else:
                             task_vars = self._variable_manager.get_vars(play=iterator._play, host=host, task=res._task)
                             play_context = self.play_context

                        self._queue_task(host, res._task, task_vars, play_context) 
                        break 
                        
                    elif cmd_type == "ModifyVar":
                        key = cmd_data.get("key")
                        val = cmd_data.get("value")
                        if key:
                            self._variable_manager.extra_vars[key] = val
                            display.display(f"[Piloteer] Modified {key} = {val} (Global/Extra Var)")
                            
                    elif cmd_type == "Continue":
                        self._send({
                            "TaskResult": {
                                "name": task,
                                "host": host.name,
                                "changed": False,
                                "failed": True,
                                "verbose_result": safe_result
                            }
                        })
                        cleaned_results.append(res)
                        break
                        
            else:
                # Task Succeeded
                is_changed = res.is_changed()
                safe_result = self._serialize_safe(res._return_data)
                self._send({
                    "TaskResult": {
                        "name": res.task_name,
                        "host": res.host.name,
                        "changed": is_changed,
                        "failed": False,
                        "verbose_result": safe_result
                    }
                })
                cleaned_results.append(res)
                
        return cleaned_results

    def _wait_for_command(self):
        if not self.sock:
            return "Continue", None
            
        buffer = ""
        while True:
            chunk = self.sock.recv(4096).decode('utf-8')
            if not chunk:
                return "Continue", None
            buffer += chunk
            if "\n" in buffer:
                line, buffer = buffer.split("\n", 1)
                try:
                    msg = json.loads(line)
                    if msg == "Retry":
                        return "Retry", None
                    elif isinstance(msg, dict) and "ModifyVar" in msg:
                        return "ModifyVar", msg["ModifyVar"]
                    elif msg == "Continue":
                        return "Continue", None
                    elif msg == "Proceed":
                         return "Continue", None
                except json.JSONDecodeError:
                    pass
    
    def get_hosts_left(self, iterator):
        return super(StrategyModule, self).get_hosts_left(iterator)

    def _get_next_task_lockstep(self, hosts, iterator):
        hosts_tasks = super(StrategyModule, self)._get_next_task_lockstep(hosts, iterator)
        if hosts_tasks:
            first_host, first_task = hosts_tasks[0]
            if first_task:
                 task_vars = self._tqm._variable_manager.get_vars(host=first_host, task=first_task)
                 serializable_vars = {}
                 for k, v in task_vars.items():
                    if k.startswith("ansible_"): continue 
                    try:
                        json.dumps(v)
                        serializable_vars[k] = v
                    except:
                        serializable_vars[k] = "<Non-Serializable>"

                 self._send({"TaskStart": {"name": first_task.get_name(), "task_vars": serializable_vars}})
                 self._wait_for_proceed()
        return hosts_tasks
