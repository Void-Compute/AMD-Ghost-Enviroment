#!/bin/bash
get_gpu_series() {
    local raw_gpu=$(lspci | grep -i "VGA" | grep -i "AMD")
    
    if [[ -z "$raw_gpu" ]]; then echo "UNKNOWN"; return; fi
    
    # Precise extraction logic
    if echo "$raw_gpu" | grep -q "RX 9"; then echo "9000"; return; fi
    if echo "$raw_gpu" | grep -q "RX 7"; then echo "7000"; return; fi
    if echo "$raw_gpu" | grep -q "RX 6"; then echo "6000"; return; fi
    if echo "$raw_gpu" | grep -q "RX 5"; then echo "5000"; return; fi
    
    echo "DEFAULT"
}
