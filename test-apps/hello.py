import sys
import platform

print(f"Python version: {sys.version}")
print(f"Platform: {platform.platform()}")
print(f"Running on: {platform.node()}")

for i in range(5):
    print(f"Line {i}")
