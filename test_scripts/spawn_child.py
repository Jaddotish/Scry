import subprocess
import time

child = subprocess.Popen(
    ["python3", "-c", "import time; time.sleep(60)"]
)

print(f"spawned child pid: {child.pid}", flush=True)

time.sleep(60)