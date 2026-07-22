import subprocess
import time

children = []

while True:
    children.append(subprocess.Popen(["sleep", "60"]))
    print(len(children))
    time.sleep(0.05)