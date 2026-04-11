
import time
import sys
import os
import http.server
import socketserver

PORT = 7860

print("Initializing Fake-AI WebUI...")

zluda_active = "libcuda.so" in os.environ.get("LD_PRELOAD", "")

if zluda_active:
    print("\n🟢 [MOCK AI] ZLUDA INJECTION DETECTED! Running in CUDA-Translation mode...\n")
else:
    print("\n🔵 [MOCK AI] Running in Native ROCm mode...\n")

load_time = None
if "--time" in sys.argv:
    try:
        idx = sys.argv.index("--time")
        load_time = int(sys.argv[idx + 1])
    except (ValueError, IndexError):
        pass

if load_time is None:
    try:
        user_input = input("How long should the model take to load? (Enter seconds): ")
        load_time = int(user_input)
    except Exception:
        print("Auto-bypassed input. Defaulting to 15 seconds.")
        load_time = 15

print(f"\nLoading model weights into VRAM (this will take {load_time} seconds)...")
sleep_interval = load_time / 10.0

for i in range(1, 11):
    print(f"Loading tensors... {i * 10}%")
    time.sleep(sleep_interval)

if "--crash" in sys.argv and not zluda_active:
    sys.stderr.write("\nRuntimeError: No HIP backend available. Torch not compiled with ROCm.\n")
    sys.exit(1)

print(f"\nModel loaded successfully! Starting Web Server on port {PORT}...")
Handler = http.server.SimpleHTTPRequestHandler
with socketserver.TCPServer(("", PORT), Handler) as httpd:
    print("Server active! ZLUDA Failover is a 100% SUCCESS.")
    httpd.serve_forever()
EOF
